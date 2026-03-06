// Watch API hook for real-time resource updates

use crate::hooks::use_cluster::ClusterContext;
use dioxus::prelude::*;
use futures::StreamExt;
use ks_kube::WatchEvent;
use kube::ResourceExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Current state of the watch connection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WatchConnectionState {
    /// Not connected / no watch active
    #[default]
    Disconnected,
    /// Initial sync in progress
    Syncing,
    /// Connected and receiving live updates
    Live,
    /// Connection error, will retry
    Reconnecting,
}

/// State for watching resources of a specific type
#[derive(Clone)]
pub struct WatchedResourceState<T: Clone> {
    /// Current list of resources
    pub items: Vec<T>,
    /// Connection state
    pub connection_state: WatchConnectionState,
    /// Last error message if any
    #[allow(dead_code)]
    pub error: Option<String>,
}

impl<T: Clone> Default for WatchedResourceState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            connection_state: WatchConnectionState::default(),
            error: None,
        }
    }
}

/// Internal resource map for tracking resources by name
type ResourceMap<T> = HashMap<String, T>;

/// Hook to watch pods with real-time updates
pub fn use_watch_pods(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Pod>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_pods(ns))
}

/// Hook to watch deployments with real-time updates
pub fn use_watch_deployments(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::apps::v1::Deployment>> {
    use_watched_resource(cluster, namespace, |client, ns| {
        client.watch_deployments(ns)
    })
}

/// Hook to watch statefulsets with real-time updates
pub fn use_watch_statefulsets(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::apps::v1::StatefulSet>> {
    use_watched_resource(cluster, namespace, |client, ns| {
        client.watch_statefulsets(ns)
    })
}

/// Hook to watch daemonsets with real-time updates
pub fn use_watch_daemonsets(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::apps::v1::DaemonSet>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_daemonsets(ns))
}

/// Hook to watch cronjobs with real-time updates
pub fn use_watch_cronjobs(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::batch::v1::CronJob>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_cronjobs(ns))
}

/// Hook to watch jobs with real-time updates
pub fn use_watch_jobs(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::batch::v1::Job>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_jobs(ns))
}

/// Hook to watch configmaps with real-time updates
pub fn use_watch_configmaps(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::ConfigMap>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_configmaps(ns))
}

/// Hook to watch secrets with real-time updates
pub fn use_watch_secrets(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Secret>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_secrets(ns))
}

/// Hook to watch services with real-time updates
pub fn use_watch_services(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Service>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_services(ns))
}

/// Hook to watch endpoints with real-time updates
pub fn use_watch_endpoints(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Endpoints>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_endpoints(ns))
}

/// Hook to watch ingresses with real-time updates
pub fn use_watch_ingresses(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::networking::v1::Ingress>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_ingresses(ns))
}

/// Hook to watch persistent volume claims with real-time updates
pub fn use_watch_persistentvolumeclaims(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::PersistentVolumeClaim>> {
    use_watched_resource(cluster, namespace, |client, ns| {
        client.watch_persistentvolumeclaims(ns)
    })
}

/// Hook to watch events with real-time updates
pub fn use_watch_events(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Event>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_events(ns))
}

/// Hook to watch persistent volumes with real-time updates (cluster-scoped)
pub fn use_watch_persistentvolumes(
    cluster: Signal<Option<ClusterContext>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::PersistentVolume>> {
    use_watched_cluster_resource(cluster, |client| client.watch_persistentvolumes())
}

/// Hook to watch storage classes with real-time updates (cluster-scoped)
pub fn use_watch_storageclasses(
    cluster: Signal<Option<ClusterContext>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::storage::v1::StorageClass>> {
    use_watched_cluster_resource(cluster, |client| client.watch_storageclasses())
}

/// Hook to watch nodes with real-time updates (cluster-scoped)
pub fn use_watch_nodes(
    cluster: Signal<Option<ClusterContext>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::core::v1::Node>> {
    use_watched_cluster_resource(cluster, |client| client.watch_nodes())
}

// === RBAC Watch Hooks ===

/// Hook to watch Roles with real-time updates (namespaced)
pub fn use_watch_roles(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::rbac::v1::Role>> {
    use_watched_resource(cluster, namespace, |client, ns| client.watch_roles(ns))
}

/// Hook to watch ClusterRoles with real-time updates (cluster-scoped)
pub fn use_watch_clusterroles(
    cluster: Signal<Option<ClusterContext>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::rbac::v1::ClusterRole>> {
    use_watched_cluster_resource(cluster, |client| client.watch_clusterroles())
}

/// Hook to watch RoleBindings with real-time updates (namespaced)
pub fn use_watch_rolebindings(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::rbac::v1::RoleBinding>> {
    use_watched_resource(cluster, namespace, |client, ns| {
        client.watch_rolebindings(ns)
    })
}

/// Hook to watch ClusterRoleBindings with real-time updates (cluster-scoped)
pub fn use_watch_clusterrolebindings(
    cluster: Signal<Option<ClusterContext>>,
) -> Signal<WatchedResourceState<k8s_openapi::api::rbac::v1::ClusterRoleBinding>> {
    use_watched_cluster_resource(cluster, |client| client.watch_clusterrolebindings())
}

/// Max backoff between retries when a watch stream errors
const MAX_RETRY_BACKOFF_SECS: u64 = 30;

/// Max consecutive errors before giving up (prevents FD exhaustion).
/// At exponential backoff 1,2,4,8,16,30,30... this is ~90s of retrying.
const MAX_CONSECUTIVE_ERRORS: u32 = 7;

/// Generic hook for watching namespaced resources
fn use_watched_resource<T, F>(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
    watch_fn: F,
) -> Signal<WatchedResourceState<T>>
where
    T: Clone + Send + Sync + 'static,
    T: kube::Resource,
    F: Fn(&ks_kube::KubeClient, Option<&str>) -> ks_kube::WatchStream<T>
        + Send
        + Sync
        + Clone
        + 'static,
{
    let mut state = use_signal(WatchedResourceState::default);
    let cancel_token = use_hook(|| Arc::new(std::sync::Mutex::new(CancellationToken::new())));

    use_effect(move || {
        let cluster_value = cluster.read().clone();
        let namespace_value = namespace.read().clone();
        let watch_fn = watch_fn.clone();

        // Cancel the previous watch task immediately — this drops the stream
        // even if it's stuck in kube-rs internal retry backoff.
        let new_token = CancellationToken::new();
        {
            let mut guard = cancel_token.lock().unwrap();
            guard.cancel();
            *guard = new_token.clone();
        }
        let token = new_token;

        // Reset state when cluster changes (including to None)
        state.set(WatchedResourceState::default());

        spawn(async move {
            if let Some(ctx) = cluster_value {
                if token.is_cancelled() {
                    return;
                }

                let ns_ref = namespace_value.as_deref().filter(|s| !s.is_empty());
                let mut backoff_secs: u64 = 1;
                let mut consecutive_errors: u32 = 0;

                // Outer retry loop: on error we drop the stream (releasing its
                // connections) and create a fresh one after a backoff delay.
                loop {
                    if token.is_cancelled() {
                        return;
                    }

                    state.set(WatchedResourceState {
                        items: vec![],
                        connection_state: WatchConnectionState::Syncing,
                        error: None,
                    });

                    let mut stream = watch_fn(&ctx.client, ns_ref);
                    let mut resources: ResourceMap<T> = HashMap::new();
                    let mut errored = false;

                    // Inner event loop — runs until stream ends, errors, or is cancelled.
                    loop {
                        let event = tokio::select! {
                            _ = token.cancelled() => {
                                tracing::debug!("Watcher cancelled");
                                return;
                            }
                            ev = stream.next() => match ev {
                                Some(e) => e,
                                None => break,
                            }
                        };

                        match event {
                            WatchEvent::Applied(obj) => {
                                let name = obj.name_any();
                                resources.insert(name, obj);
                                let current_state = state.read().connection_state;
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1; // reset on success
                                consecutive_errors = 0;
                            }
                            WatchEvent::Deleted(obj) => {
                                let name = obj.name_any();
                                resources.remove(&name);
                                let current_state = state.read().connection_state;
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::InitStarted => {
                                resources.clear();
                                state.set(WatchedResourceState {
                                    items: vec![],
                                    connection_state: WatchConnectionState::Syncing,
                                    error: None,
                                });
                            }
                            WatchEvent::InitDone => {
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: WatchConnectionState::Live,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::Restarted => {
                                resources.clear();
                                state.set(WatchedResourceState {
                                    items: vec![],
                                    connection_state: WatchConnectionState::Reconnecting,
                                    error: None,
                                });
                            }
                            WatchEvent::Error(e) => {
                                consecutive_errors += 1;
                                if !token.is_cancelled() {
                                    tracing::error!("Watch error: {}", e);
                                    state.set(WatchedResourceState {
                                        items: resources.values().cloned().collect(),
                                        connection_state: WatchConnectionState::Reconnecting,
                                        error: Some(e),
                                    });
                                }
                                // Break inner loop to drop the stream and its
                                // connections, then retry after backoff.
                                errored = true;
                                break;
                            }
                        }
                    }
                    // Stream is dropped here, releasing all kube-rs internal
                    // retry connections and associated file descriptors.

                    if !errored {
                        // Stream ended naturally (not an error) — stop retrying.
                        if !token.is_cancelled() {
                            let current_items = state.read().items.clone();
                            state.set(WatchedResourceState {
                                items: current_items,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("Watch stream ended".to_string()),
                            });
                        }
                        return;
                    }

                    // Give up after too many consecutive errors to prevent FD exhaustion.
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        tracing::warn!(
                            "Watch gave up after {} consecutive errors",
                            consecutive_errors
                        );
                        if !token.is_cancelled() {
                            let current_items = state.read().items.clone();
                            state.set(WatchedResourceState {
                                items: current_items,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("Connection failed after retries".to_string()),
                            });
                        }
                        return;
                    }

                    // Backoff before creating a fresh stream
                    tracing::debug!(
                        "Watch retry in {}s ({}/{})",
                        backoff_secs,
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS
                    );
                    tokio::select! {
                        _ = token.cancelled() => return,
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)) => {}
                    }
                    backoff_secs = (backoff_secs * 2).min(MAX_RETRY_BACKOFF_SECS);
                }
            }
        });
    });

    state
}

/// Generic hook for watching cluster-scoped resources (no namespace)
fn use_watched_cluster_resource<T, F>(
    cluster: Signal<Option<ClusterContext>>,
    watch_fn: F,
) -> Signal<WatchedResourceState<T>>
where
    T: Clone + Send + Sync + 'static,
    T: kube::Resource,
    F: Fn(&ks_kube::KubeClient) -> ks_kube::WatchStream<T> + Send + Sync + Clone + 'static,
{
    let mut state = use_signal(WatchedResourceState::default);
    let cancel_token = use_hook(|| Arc::new(std::sync::Mutex::new(CancellationToken::new())));

    use_effect(move || {
        let cluster_value = cluster.read().clone();
        let watch_fn = watch_fn.clone();

        // Cancel the previous watch task immediately
        let new_token = CancellationToken::new();
        {
            let mut guard = cancel_token.lock().unwrap();
            guard.cancel();
            *guard = new_token.clone();
        }
        let token = new_token;

        state.set(WatchedResourceState::default());

        spawn(async move {
            if let Some(ctx) = cluster_value {
                if token.is_cancelled() {
                    return;
                }

                let mut backoff_secs: u64 = 1;
                let mut consecutive_errors: u32 = 0;

                loop {
                    if token.is_cancelled() {
                        return;
                    }

                    state.set(WatchedResourceState {
                        items: vec![],
                        connection_state: WatchConnectionState::Syncing,
                        error: None,
                    });

                    let mut stream = watch_fn(&ctx.client);
                    let mut resources: ResourceMap<T> = HashMap::new();
                    let mut errored = false;

                    loop {
                        let event = tokio::select! {
                            _ = token.cancelled() => {
                                tracing::debug!("Watcher cancelled");
                                return;
                            }
                            ev = stream.next() => match ev {
                                Some(e) => e,
                                None => break,
                            }
                        };

                        match event {
                            WatchEvent::Applied(obj) => {
                                let name = obj.name_any();
                                resources.insert(name, obj);
                                let current_state = state.read().connection_state;
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::Deleted(obj) => {
                                let name = obj.name_any();
                                resources.remove(&name);
                                let current_state = state.read().connection_state;
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::InitStarted => {
                                resources.clear();
                                state.set(WatchedResourceState {
                                    items: vec![],
                                    connection_state: WatchConnectionState::Syncing,
                                    error: None,
                                });
                            }
                            WatchEvent::InitDone => {
                                state.set(WatchedResourceState {
                                    items: resources.values().cloned().collect(),
                                    connection_state: WatchConnectionState::Live,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::Restarted => {
                                resources.clear();
                                state.set(WatchedResourceState {
                                    items: vec![],
                                    connection_state: WatchConnectionState::Reconnecting,
                                    error: None,
                                });
                            }
                            WatchEvent::Error(e) => {
                                consecutive_errors += 1;
                                if !token.is_cancelled() {
                                    tracing::error!("Watch error: {}", e);
                                    state.set(WatchedResourceState {
                                        items: resources.values().cloned().collect(),
                                        connection_state: WatchConnectionState::Reconnecting,
                                        error: Some(e),
                                    });
                                }
                                errored = true;
                                break;
                            }
                        }
                    }

                    if !errored {
                        if !token.is_cancelled() {
                            let current_items = state.read().items.clone();
                            state.set(WatchedResourceState {
                                items: current_items,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("Watch stream ended".to_string()),
                            });
                        }
                        return;
                    }

                    // Give up after too many consecutive errors to prevent FD exhaustion.
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        tracing::warn!(
                            "Watch gave up after {} consecutive errors",
                            consecutive_errors
                        );
                        if !token.is_cancelled() {
                            let current_items = state.read().items.clone();
                            state.set(WatchedResourceState {
                                items: current_items,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("Connection failed after retries".to_string()),
                            });
                        }
                        return;
                    }

                    tracing::debug!(
                        "Watch retry in {}s ({}/{})",
                        backoff_secs,
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS
                    );
                    tokio::select! {
                        _ = token.cancelled() => return,
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)) => {}
                    }
                    backoff_secs = (backoff_secs * 2).min(MAX_RETRY_BACKOFF_SECS);
                }
            }
        });
    });

    state
}
