use ks_core::{SkdError, SkdResult};
use kube::config::Kubeconfig;
use std::path::PathBuf;

/// Load kubeconfig from default location or specified path.
/// Falls back to a synthetic in-cluster kubeconfig when no file is found.
pub async fn load_kubeconfig(path: Option<PathBuf>) -> SkdResult<Kubeconfig> {
    let result = match path {
        Some(p) => Kubeconfig::read_from(&p).map_err(|e| SkdError::Kubeconfig(e.to_string())),
        None => Kubeconfig::read().map_err(|e| SkdError::Kubeconfig(e.to_string())),
    };

    match result {
        Ok(config) => Ok(config),
        Err(file_err) => {
            // Fall back to in-cluster config if available
            if kube::Config::incluster().is_ok() {
                tracing::info!("No kubeconfig file found, using in-cluster service account");
                Ok(incluster_kubeconfig().await)
            } else {
                Err(file_err)
            }
        }
    }
}

/// Build a synthetic Kubeconfig that represents the in-cluster environment.
/// Detects the cluster name by querying node labels/names.
async fn incluster_kubeconfig() -> Kubeconfig {
    use kube::config::{Cluster, Context, NamedCluster, NamedContext};

    let cluster_name = detect_cluster_name().await;
    tracing::info!("Detected in-cluster name: {}", cluster_name);

    Kubeconfig {
        current_context: Some(cluster_name.clone()),
        clusters: vec![NamedCluster {
            name: cluster_name.clone(),
            cluster: Some(Cluster {
                server: Some(
                    std::env::var("KUBERNETES_SERVICE_HOST")
                        .map(|h| {
                            let port = std::env::var("KUBERNETES_SERVICE_PORT")
                                .unwrap_or_else(|_| "443".to_string());
                            format!("https://{}:{}", h, port)
                        })
                        .unwrap_or_else(|_| "https://kubernetes.default.svc".to_string()),
                ),
                ..Default::default()
            }),
        }],
        contexts: vec![NamedContext {
            name: cluster_name.clone(),
            context: Some(Context {
                cluster: cluster_name.clone(),
                ..Default::default()
            }),
        }],
        ..Default::default()
    }
}

/// Detect the cluster name from the environment.
/// Priority: CLUSTER_NAME env → well-known node labels (EKS/GKE/AKS) → fallback.
async fn detect_cluster_name() -> String {
    // Explicit override — set via Helm values or pod env
    if let Ok(name) = std::env::var("CLUSTER_NAME")
        && !name.is_empty()
    {
        return name;
    }

    // Try querying node labels for cloud-provider cluster identity
    if let Ok(config) = kube::Config::incluster()
        && let Ok(client) = kube::Client::try_from(config)
    {
        use kube::api::{Api, ListParams};
        let nodes: Api<k8s_openapi::api::core::v1::Node> = Api::all(client);
        if let Ok(node_list) = nodes.list(&ListParams::default().limit(1)).await
            && let Some(node) = node_list.items.first()
        {
            let labels = node.metadata.labels.as_ref();

            // EKS
            if let Some(name) = labels
                .and_then(|l| l.get("alpha.eksctl.io/cluster-name"))
                .or_else(|| labels.and_then(|l| l.get("eks.amazonaws.com/cluster")))
            {
                return name.clone();
            }

            // GKE
            if let Some(name) = labels.and_then(|l| l.get("cloud.google.com/gke-cluster-name")) {
                return name.clone();
            }

            // AKS
            if let Some(name) = labels.and_then(|l| l.get("kubernetes.azure.com/cluster")) {
                return name.clone();
            }
        }
    }

    "in-cluster".to_string()
}

/// List available context names from kubeconfig
pub fn list_contexts(config: &Kubeconfig) -> Vec<String> {
    config.contexts.iter().map(|c| c.name.clone()).collect()
}

/// Get the current context name
pub fn current_context(config: &Kubeconfig) -> Option<String> {
    config.current_context.clone()
}
