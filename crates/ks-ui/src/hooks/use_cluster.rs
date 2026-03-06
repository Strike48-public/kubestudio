use dioxus::prelude::*;
use ks_kube::KubeClient;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global connection generation counter - incremented on each cluster switch
/// Used to invalidate stale watcher tasks
static CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Get the current connection generation
fn current_generation() -> u64 {
    CONNECTION_GENERATION.load(Ordering::SeqCst)
}

#[derive(Clone)]
pub struct ClusterContext {
    pub context_name: String,
    pub client: Arc<KubeClient>,
    /// Generation ID - watchers should ignore updates if this doesn't match current
    pub generation: u64,
    #[allow(dead_code)]
    pub connected: bool,
}

/// Hook for managing cluster connection state
/// Returns current cluster context and a function to trigger connection
pub fn use_cluster() -> Signal<Option<ClusterContext>> {
    use_signal(|| None::<ClusterContext>)
}

/// Hook to connect to a cluster by context name
pub fn use_connect_cluster(
    cluster_state: Signal<Option<ClusterContext>>,
) -> impl Fn(String) + Copy {
    move |context_name: String| {
        let mut cluster_state = cluster_state;

        // Increment generation to invalidate all existing watchers
        let new_generation = CONNECTION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        tracing::info!("Cluster switch initiated, generation: {}", new_generation);

        // IMPORTANT: Clear cluster state IMMEDIATELY when switching
        // This ensures all watchers reset before the new connection attempt
        cluster_state.set(None);

        spawn(async move {
            tracing::info!("Connecting to cluster context: {}", context_name);
            match KubeClient::from_context(&context_name).await {
                Ok(client) => {
                    // Only set if this is still the current generation
                    if current_generation() == new_generation {
                        cluster_state.set(Some(ClusterContext {
                            context_name,
                            client: Arc::new(client),
                            generation: new_generation,
                            connected: true,
                        }));
                        tracing::info!(
                            "Successfully connected to cluster (gen {})",
                            new_generation
                        );
                    } else {
                        tracing::info!(
                            "Ignoring stale connection (gen {} vs current {})",
                            new_generation,
                            current_generation()
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to connect to cluster: {}", e);
                    // Already None, but log the failure
                }
            }
        });
    }
}
