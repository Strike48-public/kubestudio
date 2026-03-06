use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::sync::watch;

/// Event type for resource changes from watch streams
#[derive(Debug, Clone)]
pub enum WatchEvent<T> {
    /// Resource was added or updated
    Applied(T),
    /// Resource was deleted
    Deleted(T),
    /// Initial sync started
    InitStarted,
    /// Initial sync completed, stream now receiving live updates
    InitDone,
    /// Stream restarted (e.g., after reconnection)
    Restarted,
    /// Error occurred in the watch stream
    Error(String),
}

/// A boxed stream of watch events
pub type WatchStream<T> = Pin<Box<dyn futures::Stream<Item = WatchEvent<T>> + Send + 'static>>;

/// Handle to a running port-forward
#[derive(Clone)]
pub struct PortForwardHandle {
    /// Pod name
    pub pod_name: String,
    /// Namespace
    pub namespace: String,
    /// Local port
    pub local_port: u16,
    /// Remote port
    pub remote_port: u16,
    /// Sender to signal shutdown
    pub(crate) shutdown_tx: watch::Sender<bool>,
}

impl PortForwardHandle {
    /// Stop the port-forward
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Info about an active port-forward (for display)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortForwardInfo {
    pub pod_name: String,
    pub namespace: String,
    pub local_port: u16,
    pub remote_port: u16,
}

/// Handle to an exec session
pub struct ExecHandle {
    /// Sender for stdin
    pub stdin_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    /// Receiver for stdout
    pub stdout_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    /// Receiver for stderr
    pub stderr_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    /// Pod name
    pub pod_name: String,
    /// Namespace
    pub namespace: String,
    /// Container name
    pub container: Option<String>,
    /// Shutdown sender
    pub(crate) shutdown_tx: watch::Sender<bool>,
}

impl ExecHandle {
    /// Stop the exec session
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Send input to the session
    pub fn send(&self, data: &[u8]) -> bool {
        self.stdin_tx.send(data.to_vec()).is_ok()
    }
}

/// Result of applying a YAML manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub name: String,
    pub kind: String,
    pub namespace: Option<String>,
}

/// Result of applying multiple YAML documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiApplyResult {
    pub results: Vec<ApplyResult>,
    pub errors: Vec<String>,
}

/// Convert a Kubernetes kind to its plural form for API paths
pub fn pluralize_kind(kind: &str) -> String {
    let lower = kind.to_lowercase();
    match lower.as_str() {
        "ingress" => "ingresses".to_string(),
        "endpoints" => "endpoints".to_string(),
        "storageclass" => "storageclasses".to_string(),
        _ if lower.ends_with("s") => format!("{}es", lower),
        _ if lower.ends_with("y") => format!("{}ies", &lower[..lower.len() - 1]),
        _ => format!("{}s", lower),
    }
}
