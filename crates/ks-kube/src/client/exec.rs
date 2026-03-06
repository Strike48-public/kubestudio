//! Pod exec and port-forward operations

use k8s_openapi::api::core::v1::Pod;
use ks_core::{SkdError, SkdResult};
use kube::api::Api;
use tokio::net::TcpListener;
use tokio::sync::watch;

use super::KubeClient;
use super::types::{ExecHandle, PortForwardHandle};

impl KubeClient {
    /// Start a port-forward to a pod
    /// Returns a handle that can be used to stop the forward
    pub async fn port_forward(
        &self,
        pod_name: &str,
        namespace: &str,
        local_port: u16,
        remote_port: u16,
    ) -> SkdResult<PortForwardHandle> {
        let api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to bind to port {}: {}", local_port, e),
            })?;

        let pod_name_owned = pod_name.to_string();
        let api_clone = api.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!("Port-forward to {}:{} stopped", pod_name_owned, remote_port);
                            break;
                        }
                    }
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((mut client_stream, _)) => {
                                let api = api_clone.clone();
                                let pod_name = pod_name_owned.clone();
                                let port = remote_port;

                                tokio::spawn(async move {
                                    match api.portforward(&pod_name, &[port]).await {
                                        Ok(mut pf) => {
                                            if let Some(mut upstream) = pf.take_stream(port) {
                                                let (mut client_read, mut client_write) = client_stream.split();
                                                let (mut upstream_read, mut upstream_write) = tokio::io::split(&mut upstream);

                                                let client_to_upstream = tokio::io::copy(&mut client_read, &mut upstream_write);
                                                let upstream_to_client = tokio::io::copy(&mut upstream_read, &mut client_write);

                                                tokio::select! {
                                                    _ = client_to_upstream => {}
                                                    _ = upstream_to_client => {}
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Port-forward connection error: {}", e);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("Failed to accept connection: {}", e);
                            }
                        }
                    }
                }
            }
        });

        Ok(PortForwardHandle {
            pod_name: pod_name.to_string(),
            namespace: namespace.to_string(),
            local_port,
            remote_port,
            shutdown_tx,
        })
    }

    /// Start an exec session in a pod
    /// Returns a handle for stdin/stdout communication
    pub async fn exec(
        &self,
        pod_name: &str,
        namespace: &str,
        container: Option<&str>,
        command: Vec<&str>,
    ) -> SkdResult<ExecHandle> {
        use kube::api::AttachParams;
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncWriteExt;

        let api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);

        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (stdout_tx, stdout_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (_stderr_tx, stderr_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let attach_params = AttachParams {
            container: container.map(|s| s.to_string()),
            stdin: true,
            stdout: true,
            stderr: false, // Cannot be true when tty is true
            tty: true,
            ..Default::default()
        };

        let pod_name_owned = pod_name.to_string();
        let command: Vec<String> = command.into_iter().map(|s| s.to_string()).collect();

        let mut attached = api
            .exec(&pod_name_owned, command, &attach_params)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to exec into pod: {}", e),
            })?;

        tokio::spawn(async move {
            let stdin_stream = attached.stdin();
            let stdout_stream = attached.stdout();

            if let Some(mut stdin) = stdin_stream {
                tokio::spawn(async move {
                    while let Some(data) = stdin_rx.recv().await {
                        if stdin.write_all(&data).await.is_err() {
                            break;
                        }
                        let _ = stdin.flush().await;
                    }
                });
            }

            if let Some(mut stdout) = stdout_stream {
                let stdout_tx_clone = stdout_tx.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    loop {
                        match stdout.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                if stdout_tx_clone.send(buf[..n].to_vec()).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
            }

            let mut shutdown_rx_inner = shutdown_rx;
            let _ = shutdown_rx_inner.changed().await;
        });

        Ok(ExecHandle {
            stdin_tx,
            stdout_rx,
            stderr_rx,
            pod_name: pod_name.to_string(),
            namespace: namespace.to_string(),
            container: container.map(|s| s.to_string()),
            shutdown_tx,
        })
    }
}
