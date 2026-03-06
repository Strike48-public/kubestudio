//! Toolbox pod management for AI agent command execution
//!
//! This module provides a persistent "toolbox" pod pattern for executing
//! commands inside a Kubernetes cluster. The toolbox pod runs with kubectl
//! and other CLI tools pre-installed, allowing AI agents to perform
//! cluster management operations.
//!
//! ## Permission Modes
//!
//! The toolbox supports two permission modes:
//! - `ReadOnly`: Uses the `view` ClusterRole, only allows read commands
//! - `ReadWrite`: Uses the `cluster-admin` ClusterRole, allows all commands

use k8s_openapi::api::core::v1::{Container, Namespace, Pod, PodSpec, ServiceAccount};
use k8s_openapi::api::rbac::v1::{ClusterRoleBinding, RoleRef, Subject};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use ks_core::{SkdError, SkdResult};
use kube::Client;
use kube::api::{Api, AttachParams, PostParams};
use std::collections::BTreeMap;
use tokio::io::AsyncReadExt;

/// Default namespace for the toolbox pod
pub const TOOLBOX_NAMESPACE: &str = "kubestudio-system";

/// Default name for the toolbox pod
pub const TOOLBOX_POD_NAME: &str = "kubestudio-toolbox";

/// Default name for the toolbox service account
pub const TOOLBOX_SERVICE_ACCOUNT: &str = "kubestudio-toolbox";

/// Default image for the toolbox pod
pub const TOOLBOX_IMAGE: &str = "bitnami/kubectl:latest";

/// Permission mode for the toolbox
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    /// Read-only access - uses `view` ClusterRole
    /// Only allows read commands (get, list, describe, logs)
    ReadOnly,

    /// Read-write access - uses `cluster-admin` ClusterRole
    /// Allows all commands including create, apply, delete
    #[default]
    ReadWrite,
}

impl PermissionMode {
    /// Parse from string (e.g., from environment variable)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "read" | "readonly" | "read-only" | "view" => PermissionMode::ReadOnly,
            "write" | "readwrite" | "read-write" | "admin" | "cluster-admin" => {
                PermissionMode::ReadWrite
            }
            _ => PermissionMode::ReadWrite, // Default to read-write
        }
    }

    /// Get the ClusterRole name for this permission mode
    pub fn cluster_role(&self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "view",
            PermissionMode::ReadWrite => "cluster-admin",
        }
    }

    /// Check if a command is allowed in this permission mode
    pub fn is_command_allowed(&self, command: &str) -> bool {
        match self {
            PermissionMode::ReadWrite => true,
            PermissionMode::ReadOnly => !is_write_command(command),
        }
    }
}

/// Check if a command is a write operation
fn is_write_command(command: &str) -> bool {
    // Normalize the command for checking
    let cmd_lower = command.to_lowercase();

    // kubectl write operations
    let write_patterns = [
        "kubectl apply",
        "kubectl create",
        "kubectl delete",
        "kubectl patch",
        "kubectl replace",
        "kubectl edit",
        "kubectl scale",
        "kubectl rollout",
        "kubectl set",
        "kubectl label",
        "kubectl annotate",
        "kubectl taint",
        "kubectl cordon",
        "kubectl uncordon",
        "kubectl drain",
        "kubectl cp",
        "kubectl exec",
        "kubectl run",
        "kubectl expose",
        "kubectl autoscale",
        // helm write operations
        "helm install",
        "helm upgrade",
        "helm uninstall",
        "helm delete",
        "helm rollback",
        // Other dangerous operations
        "rm ",
        "rm\t",
        "rmdir",
        "mv ",
        "mv\t",
        "> ",
        ">>",
        "kubectl proxy",
        "kubectl port-forward",
    ];

    for pattern in write_patterns {
        if cmd_lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Result of a toolbox command execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Status of the toolbox pod
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolboxStatus {
    pub exists: bool,
    pub ready: bool,
    pub phase: Option<String>,
    pub namespace: String,
    pub pod_name: String,
    pub image: Option<String>,
    pub message: Option<String>,
    pub permission_mode: PermissionMode,
}

/// Toolbox manager for creating and managing the toolbox pod
pub struct Toolbox {
    client: Client,
    namespace: String,
    pod_name: String,
    image: String,
    permission_mode: PermissionMode,
}

impl Toolbox {
    /// Create a new Toolbox manager with default settings (read-write mode)
    pub fn new(client: Client) -> Self {
        Self {
            client,
            namespace: TOOLBOX_NAMESPACE.to_string(),
            pod_name: TOOLBOX_POD_NAME.to_string(),
            image: TOOLBOX_IMAGE.to_string(),
            permission_mode: PermissionMode::default(),
        }
    }

    /// Create a new Toolbox manager with specified permission mode
    pub fn with_permission_mode(client: Client, permission_mode: PermissionMode) -> Self {
        Self {
            client,
            namespace: TOOLBOX_NAMESPACE.to_string(),
            pod_name: TOOLBOX_POD_NAME.to_string(),
            image: TOOLBOX_IMAGE.to_string(),
            permission_mode,
        }
    }

    /// Create a new Toolbox manager with custom settings
    pub fn with_config(
        client: Client,
        namespace: impl Into<String>,
        pod_name: impl Into<String>,
        image: impl Into<String>,
        permission_mode: PermissionMode,
    ) -> Self {
        Self {
            client,
            namespace: namespace.into(),
            pod_name: pod_name.into(),
            image: image.into(),
            permission_mode,
        }
    }

    /// Get the current permission mode
    pub fn permission_mode(&self) -> PermissionMode {
        self.permission_mode
    }

    /// Get the toolbox status
    pub async fn status(&self) -> SkdResult<ToolboxStatus> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        match pods.get_opt(&self.pod_name).await {
            Ok(Some(pod)) => {
                let phase = pod.status.as_ref().and_then(|s| s.phase.clone());

                let ready = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.conditions.as_ref())
                    .map(|conditions| {
                        conditions
                            .iter()
                            .any(|c| c.type_ == "Ready" && c.status == "True")
                    })
                    .unwrap_or(false);

                let image = pod
                    .spec
                    .as_ref()
                    .and_then(|s| s.containers.first())
                    .map(|c| c.image.clone().unwrap_or_default());

                let message = pod.status.as_ref().and_then(|s| s.message.clone());

                Ok(ToolboxStatus {
                    exists: true,
                    ready,
                    phase,
                    namespace: self.namespace.clone(),
                    pod_name: self.pod_name.clone(),
                    image,
                    message,
                    permission_mode: self.permission_mode,
                })
            }
            Ok(None) => Ok(ToolboxStatus {
                exists: false,
                ready: false,
                phase: None,
                namespace: self.namespace.clone(),
                pod_name: self.pod_name.clone(),
                image: None,
                message: None,
                permission_mode: self.permission_mode,
            }),
            Err(e) => Err(SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to get toolbox status: {}", e),
            }),
        }
    }

    /// Ensure the toolbox namespace exists
    async fn ensure_namespace(&self) -> SkdResult<()> {
        let namespaces: Api<Namespace> = Api::all(self.client.clone());

        if namespaces
            .get_opt(&self.namespace)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to check namespace: {}", e),
            })?
            .is_none()
        {
            let ns = Namespace {
                metadata: ObjectMeta {
                    name: Some(self.namespace.clone()),
                    labels: Some(BTreeMap::from([(
                        "app.kubernetes.io/managed-by".to_string(),
                        "kubestudio".to_string(),
                    )])),
                    ..Default::default()
                },
                ..Default::default()
            };

            namespaces
                .create(&PostParams::default(), &ns)
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: format!("Failed to create namespace: {}", e),
                })?;

            tracing::info!("Created namespace: {}", self.namespace);
        }

        Ok(())
    }

    /// Ensure the service account exists with appropriate permissions
    async fn ensure_service_account(&self) -> SkdResult<()> {
        let sa_api: Api<ServiceAccount> = Api::namespaced(self.client.clone(), &self.namespace);

        if sa_api
            .get_opt(TOOLBOX_SERVICE_ACCOUNT)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to check service account: {}", e),
            })?
            .is_none()
        {
            let sa = ServiceAccount {
                metadata: ObjectMeta {
                    name: Some(TOOLBOX_SERVICE_ACCOUNT.to_string()),
                    namespace: Some(self.namespace.clone()),
                    labels: Some(BTreeMap::from([(
                        "app.kubernetes.io/managed-by".to_string(),
                        "kubestudio".to_string(),
                    )])),
                    ..Default::default()
                },
                ..Default::default()
            };

            sa_api
                .create(&PostParams::default(), &sa)
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: format!("Failed to create service account: {}", e),
                })?;

            tracing::info!("Created service account: {}", TOOLBOX_SERVICE_ACCOUNT);
        }

        // Ensure ClusterRoleBinding with appropriate role based on permission mode
        let crb_api: Api<ClusterRoleBinding> = Api::all(self.client.clone());
        let role_name = self.permission_mode.cluster_role();
        let crb_name = format!("{}-{}", TOOLBOX_SERVICE_ACCOUNT, role_name);

        // Check if correct binding exists
        if crb_api
            .get_opt(&crb_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to check cluster role binding: {}", e),
            })?
            .is_none()
        {
            // Delete any existing bindings with different roles
            let other_role = match self.permission_mode {
                PermissionMode::ReadOnly => "cluster-admin",
                PermissionMode::ReadWrite => "view",
            };
            let other_crb_name = format!("{}-{}", TOOLBOX_SERVICE_ACCOUNT, other_role);

            if crb_api
                .get_opt(&other_crb_name)
                .await
                .ok()
                .flatten()
                .is_some()
            {
                let _ = crb_api.delete(&other_crb_name, &Default::default()).await;
                tracing::info!("Deleted old cluster role binding: {}", other_crb_name);
            }

            // Create the correct binding
            let crb = ClusterRoleBinding {
                metadata: ObjectMeta {
                    name: Some(crb_name.clone()),
                    labels: Some(BTreeMap::from([
                        (
                            "app.kubernetes.io/managed-by".to_string(),
                            "kubestudio".to_string(),
                        ),
                        (
                            "kubestudio.io/permission-mode".to_string(),
                            format!("{:?}", self.permission_mode).to_lowercase(),
                        ),
                    ])),
                    ..Default::default()
                },
                role_ref: RoleRef {
                    api_group: "rbac.authorization.k8s.io".to_string(),
                    kind: "ClusterRole".to_string(),
                    name: role_name.to_string(),
                },
                subjects: Some(vec![Subject {
                    kind: "ServiceAccount".to_string(),
                    name: TOOLBOX_SERVICE_ACCOUNT.to_string(),
                    namespace: Some(self.namespace.clone()),
                    ..Default::default()
                }]),
            };

            crb_api
                .create(&PostParams::default(), &crb)
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: format!("Failed to create cluster role binding: {}", e),
                })?;

            tracing::info!(
                "Created cluster role binding: {} (role: {})",
                crb_name,
                role_name
            );
        }

        Ok(())
    }

    /// Ensure the toolbox pod is running
    pub async fn ensure_running(&self) -> SkdResult<ToolboxStatus> {
        // Ensure namespace exists
        self.ensure_namespace().await?;

        // Ensure service account and RBAC
        self.ensure_service_account().await?;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        // Check if pod already exists
        if let Some(existing) =
            pods.get_opt(&self.pod_name)
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: format!("Failed to check pod: {}", e),
                })?
        {
            let phase = existing
                .status
                .as_ref()
                .and_then(|s| s.phase.as_deref())
                .unwrap_or("Unknown");

            // If pod is running, we're good
            if phase == "Running" {
                return self.status().await;
            }

            // If pod is in a terminal state, delete and recreate
            if phase == "Failed" || phase == "Succeeded" {
                tracing::info!("Toolbox pod in terminal state ({}), recreating", phase);
                pods.delete(&self.pod_name, &Default::default())
                    .await
                    .map_err(|e| SkdError::KubeApi {
                        status_code: 500,
                        message: format!("Failed to delete pod: {}", e),
                    })?;

                // Wait for pod to be deleted
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            } else {
                // Pod is pending or in another state, wait for it
                return self.wait_for_ready().await;
            }
        }

        // Create the toolbox pod
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some(self.pod_name.clone()),
                namespace: Some(self.namespace.clone()),
                labels: Some(BTreeMap::from([
                    ("app".to_string(), "kubestudio-toolbox".to_string()),
                    (
                        "app.kubernetes.io/managed-by".to_string(),
                        "kubestudio".to_string(),
                    ),
                    (
                        "kubestudio.io/permission-mode".to_string(),
                        format!("{:?}", self.permission_mode).to_lowercase(),
                    ),
                ])),
                ..Default::default()
            },
            spec: Some(PodSpec {
                service_account_name: Some(TOOLBOX_SERVICE_ACCOUNT.to_string()),
                containers: vec![Container {
                    name: "toolbox".to_string(),
                    image: Some(self.image.clone()),
                    command: Some(vec!["sleep".to_string(), "infinity".to_string()]),
                    ..Default::default()
                }],
                restart_policy: Some("Always".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        pods.create(&PostParams::default(), &pod)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to create toolbox pod: {}", e),
            })?;

        tracing::info!(
            "Created toolbox pod: {}/{} (mode: {:?})",
            self.namespace,
            self.pod_name,
            self.permission_mode
        );

        // Wait for pod to be ready
        self.wait_for_ready().await
    }

    /// Wait for the toolbox pod to be ready
    async fn wait_for_ready(&self) -> SkdResult<ToolboxStatus> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        for _ in 0..60 {
            if let Some(pod) =
                pods.get_opt(&self.pod_name)
                    .await
                    .map_err(|e| SkdError::KubeApi {
                        status_code: 500,
                        message: format!("Failed to get pod: {}", e),
                    })?
            {
                let phase = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.phase.as_deref())
                    .unwrap_or("Unknown");

                if phase == "Running" {
                    let ready = pod
                        .status
                        .as_ref()
                        .and_then(|s| s.conditions.as_ref())
                        .map(|conditions| {
                            conditions
                                .iter()
                                .any(|c| c.type_ == "Ready" && c.status == "True")
                        })
                        .unwrap_or(false);

                    if ready {
                        return self.status().await;
                    }
                }

                if phase == "Failed" {
                    return Err(SkdError::KubeApi {
                        status_code: 500,
                        message: "Toolbox pod failed to start".to_string(),
                    });
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Err(SkdError::KubeApi {
            status_code: 500,
            message: "Timeout waiting for toolbox pod to be ready".to_string(),
        })
    }

    /// Execute a command in the toolbox pod
    ///
    /// In read-only mode, write commands will be rejected before execution.
    pub async fn exec(&self, command: &str) -> SkdResult<ExecResult> {
        // Check if command is allowed in current permission mode
        if !self.permission_mode.is_command_allowed(command) {
            return Err(SkdError::KubeApi {
                status_code: 403,
                message: format!(
                    "Command not allowed in read-only mode. Blocked write operation: {}",
                    command.chars().take(50).collect::<String>()
                ),
            });
        }

        // Ensure toolbox is running
        let status = self.status().await?;
        if !status.ready {
            return Err(SkdError::KubeApi {
                status_code: 400,
                message: "Toolbox pod is not ready. Call ensure_running() first.".to_string(),
            });
        }

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        // Use sh -c to execute the command
        let cmd = vec!["sh", "-c", command];

        let attach_params = AttachParams {
            stdin: false,
            stdout: true,
            stderr: true,
            tty: false,
            ..Default::default()
        };

        let mut attached = pods
            .exec(&self.pod_name, cmd, &attach_params)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to exec command: {}", e),
            })?;

        // Read stdout
        let mut stdout_buf = Vec::new();
        if let Some(mut stdout) = attached.stdout() {
            stdout.read_to_end(&mut stdout_buf).await.ok();
        }

        // Read stderr
        let mut stderr_buf = Vec::new();
        if let Some(mut stderr) = attached.stderr() {
            stderr.read_to_end(&mut stderr_buf).await.ok();
        }

        // Get exit status
        let exit_status = attached.take_status();
        let exit_code = if let Some(status) = exit_status {
            match status.await {
                Some(status) => {
                    if status.status == Some("Success".to_string()) {
                        Some(0)
                    } else {
                        // Try to parse exit code from reason
                        status
                            .reason
                            .as_ref()
                            .and_then(|r| r.strip_prefix("ExitCode:"))
                            .and_then(|c| c.trim().parse().ok())
                            .or(Some(1))
                    }
                }
                None => None,
            }
        } else {
            None
        };

        let stdout = String::from_utf8_lossy(&stdout_buf).to_string();
        let stderr = String::from_utf8_lossy(&stderr_buf).to_string();
        let success = exit_code == Some(0);

        Ok(ExecResult {
            success,
            exit_code,
            stdout,
            stderr,
        })
    }

    /// Delete the toolbox pod only
    pub async fn delete(&self) -> SkdResult<()> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        if pods
            .get_opt(&self.pod_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to check pod: {}", e),
            })?
            .is_some()
        {
            pods.delete(&self.pod_name, &Default::default())
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: format!("Failed to delete toolbox pod: {}", e),
                })?;

            tracing::info!("Deleted toolbox pod: {}/{}", self.namespace, self.pod_name);
        }

        Ok(())
    }

    /// Clean up all toolbox resources (pod, service account, ClusterRoleBinding)
    ///
    /// This should be called on application shutdown to avoid leaving orphaned resources.
    /// Use `cleanup_all_with_timeout` for graceful shutdown scenarios.
    pub async fn cleanup_all(&self) -> SkdResult<()> {
        let mut errors = Vec::new();

        // 1. Delete the pod
        if let Err(e) = self.delete().await {
            tracing::warn!("Failed to delete toolbox pod: {}", e);
            errors.push(format!("pod: {}", e));
        }

        // 2. Delete the ClusterRoleBindings (both possible roles)
        let crb_api: Api<ClusterRoleBinding> = Api::all(self.client.clone());
        for role in ["view", "cluster-admin"] {
            let crb_name = format!("{}-{}", TOOLBOX_SERVICE_ACCOUNT, role);
            if crb_api.get_opt(&crb_name).await.unwrap_or(None).is_some() {
                if let Err(e) = crb_api.delete(&crb_name, &Default::default()).await {
                    tracing::warn!("Failed to delete ClusterRoleBinding {}: {}", crb_name, e);
                    errors.push(format!("clusterrolebinding/{}: {}", crb_name, e));
                } else {
                    tracing::info!("Deleted ClusterRoleBinding: {}", crb_name);
                }
            }
        }

        // 3. Delete the service account
        let sa_api: Api<ServiceAccount> = Api::namespaced(self.client.clone(), &self.namespace);
        if sa_api
            .get_opt(TOOLBOX_SERVICE_ACCOUNT)
            .await
            .unwrap_or(None)
            .is_some()
        {
            if let Err(e) = sa_api
                .delete(TOOLBOX_SERVICE_ACCOUNT, &Default::default())
                .await
            {
                tracing::warn!("Failed to delete service account: {}", e);
                errors.push(format!("serviceaccount: {}", e));
            } else {
                tracing::info!(
                    "Deleted service account: {}/{}",
                    self.namespace,
                    TOOLBOX_SERVICE_ACCOUNT
                );
            }
        }

        if errors.is_empty() {
            tracing::info!("Toolbox cleanup completed successfully");
            Ok(())
        } else {
            Err(SkdError::KubeApi {
                status_code: 500,
                message: format!("Partial cleanup failure: {}", errors.join(", ")),
            })
        }
    }

    /// Clean up all toolbox resources with a timeout
    ///
    /// Returns Ok(true) if cleanup succeeded, Ok(false) if timed out, Err on failure.
    pub async fn cleanup_all_with_timeout(&self, timeout_secs: u64) -> SkdResult<bool> {
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(timeout_secs),
            self.cleanup_all(),
        )
        .await
        {
            Ok(result) => {
                result?;
                Ok(true)
            }
            Err(_) => {
                tracing::warn!(
                    "Toolbox cleanup timed out after {}s, exiting anyway",
                    timeout_secs
                );
                Ok(false)
            }
        }
    }
}

/// Clean up any orphaned toolbox resources from a previous session
///
/// This is useful to call on startup to clean up resources from crashed sessions.
pub async fn cleanup_orphaned_toolbox(client: Client) -> SkdResult<()> {
    let toolbox = Toolbox::new(client);
    let status = toolbox.status().await?;

    if status.exists {
        tracing::info!("Found orphaned toolbox from previous session, cleaning up...");
        toolbox.cleanup_all().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_from_str() {
        assert_eq!(PermissionMode::from_str("read"), PermissionMode::ReadOnly);
        assert_eq!(
            PermissionMode::from_str("readonly"),
            PermissionMode::ReadOnly
        );
        assert_eq!(PermissionMode::from_str("view"), PermissionMode::ReadOnly);
        assert_eq!(PermissionMode::from_str("write"), PermissionMode::ReadWrite);
        assert_eq!(PermissionMode::from_str("admin"), PermissionMode::ReadWrite);
        assert_eq!(
            PermissionMode::from_str("unknown"),
            PermissionMode::ReadWrite
        );
    }

    #[test]
    fn test_is_write_command() {
        // Write commands
        assert!(is_write_command("kubectl apply -f deployment.yaml"));
        assert!(is_write_command("kubectl create namespace test"));
        assert!(is_write_command("kubectl delete pod nginx"));
        assert!(is_write_command(
            "kubectl scale deployment nginx --replicas=3"
        ));
        assert!(is_write_command("helm install myapp ./chart"));
        assert!(is_write_command("kubectl exec -it nginx -- bash"));

        // Read commands
        assert!(!is_write_command("kubectl get pods"));
        assert!(!is_write_command("kubectl describe pod nginx"));
        assert!(!is_write_command("kubectl logs nginx"));
        assert!(!is_write_command("kubectl get nodes -o wide"));
        assert!(!is_write_command("helm list"));
        assert!(!is_write_command("helm status myapp"));
    }

    #[test]
    fn test_permission_mode_command_allowed() {
        let read_only = PermissionMode::ReadOnly;
        let read_write = PermissionMode::ReadWrite;

        // Read-only mode
        assert!(read_only.is_command_allowed("kubectl get pods"));
        assert!(!read_only.is_command_allowed("kubectl apply -f foo.yaml"));
        assert!(!read_only.is_command_allowed("kubectl delete pod nginx"));

        // Read-write mode
        assert!(read_write.is_command_allowed("kubectl get pods"));
        assert!(read_write.is_command_allowed("kubectl apply -f foo.yaml"));
        assert!(read_write.is_command_allowed("kubectl delete pod nginx"));
    }
}
