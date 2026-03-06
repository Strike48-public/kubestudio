use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum SkdError {
    #[error("Cluster connection failed: {cluster_name} - {message}")]
    ClusterConnection {
        cluster_name: String,
        message: String,
    },

    #[error("Resource not found: {kind}/{name} in namespace {namespace:?}")]
    ResourceNotFound {
        kind: String,
        name: String,
        namespace: Option<String>,
    },

    #[error("Kubernetes API error: {status_code} - {message}")]
    KubeApi { status_code: u16, message: String },

    #[error("Kubeconfig error: {0}")]
    Kubeconfig(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Watch stream closed unexpectedly")]
    WatchClosed,
}

pub type SkdResult<T> = Result<T, SkdError>;
