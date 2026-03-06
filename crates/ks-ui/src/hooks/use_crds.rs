//! Hooks for watching Custom Resource Definitions and their instances

use crate::hooks::use_cluster::ClusterContext;
use crate::hooks::use_watch::{WatchConnectionState, WatchedResourceState};
use dioxus::prelude::*;
use futures::StreamExt;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use ks_kube::{CrdInfo, WatchEvent};
use kube::ResourceExt;
use kube::api::DynamicObject;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// State for watching CRDs with parsed CrdInfo
#[derive(Clone, Default)]
pub struct CrdState {
    /// Parsed CRD info list
    pub crds: Vec<CrdInfo>,
    /// Connection state
    pub connection_state: WatchConnectionState,
    /// Last error if any
    #[allow(dead_code)]
    pub error: Option<String>,
}

/// Max backoff between retries when a watch stream errors
const MAX_RETRY_BACKOFF_SECS: u64 = 30;

/// Max consecutive errors before giving up (prevents FD exhaustion).
const MAX_CONSECUTIVE_ERRORS: u32 = 7;

/// Hook to watch all CRDs in the cluster
pub fn use_watch_crds(cluster: Signal<Option<ClusterContext>>) -> Signal<CrdState> {
    let mut state = use_signal(CrdState::default);
    let cancel_token = use_hook(|| Arc::new(std::sync::Mutex::new(CancellationToken::new())));

    use_effect(move || {
        let cluster_value = cluster.read().clone();

        let new_token = CancellationToken::new();
        {
            let mut guard = cancel_token.lock().unwrap();
            guard.cancel();
            *guard = new_token.clone();
        }
        let token = new_token;

        state.set(CrdState::default());

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

                    state.set(CrdState {
                        crds: vec![],
                        connection_state: WatchConnectionState::Syncing,
                        error: None,
                    });

                    let mut stream = ctx.client.watch_crds();
                    let mut crd_map: HashMap<String, CustomResourceDefinition> = HashMap::new();
                    let mut errored = false;

                    loop {
                        let event = tokio::select! {
                            _ = token.cancelled() => {
                                tracing::debug!("CRD watcher cancelled");
                                return;
                            }
                            ev = stream.next() => match ev {
                                Some(e) => e,
                                None => break,
                            }
                        };

                        match event {
                            WatchEvent::Applied(crd) => {
                                let name = crd.name_any();
                                crd_map.insert(name, crd);
                                let crds: Vec<CrdInfo> =
                                    crd_map.values().filter_map(CrdInfo::from_crd).collect();
                                let current_state = state.read().connection_state;
                                state.set(CrdState {
                                    crds,
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::Deleted(crd) => {
                                let name = crd.name_any();
                                crd_map.remove(&name);
                                let crds: Vec<CrdInfo> =
                                    crd_map.values().filter_map(CrdInfo::from_crd).collect();
                                let current_state = state.read().connection_state;
                                state.set(CrdState {
                                    crds,
                                    connection_state: current_state,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::InitStarted => {
                                crd_map.clear();
                                state.set(CrdState {
                                    crds: vec![],
                                    connection_state: WatchConnectionState::Syncing,
                                    error: None,
                                });
                            }
                            WatchEvent::InitDone => {
                                let crds: Vec<CrdInfo> =
                                    crd_map.values().filter_map(CrdInfo::from_crd).collect();
                                state.set(CrdState {
                                    crds,
                                    connection_state: WatchConnectionState::Live,
                                    error: None,
                                });
                                backoff_secs = 1;
                                consecutive_errors = 0;
                            }
                            WatchEvent::Restarted => {
                                crd_map.clear();
                                state.set(CrdState {
                                    crds: vec![],
                                    connection_state: WatchConnectionState::Reconnecting,
                                    error: None,
                                });
                            }
                            WatchEvent::Error(e) => {
                                consecutive_errors += 1;
                                if !token.is_cancelled() {
                                    tracing::error!("CRD watch error: {}", e);
                                    let crds = state.read().crds.clone();
                                    state.set(CrdState {
                                        crds,
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
                            let crds = state.read().crds.clone();
                            state.set(CrdState {
                                crds,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("CRD watch stream ended".to_string()),
                            });
                        }
                        return;
                    }

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        tracing::warn!(
                            "CRD watch gave up after {} consecutive errors",
                            consecutive_errors
                        );
                        if !token.is_cancelled() {
                            let crds = state.read().crds.clone();
                            state.set(CrdState {
                                crds,
                                connection_state: WatchConnectionState::Disconnected,
                                error: Some("Connection failed after retries".to_string()),
                            });
                        }
                        return;
                    }

                    tracing::debug!(
                        "CRD watch retry in {}s ({}/{})",
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

/// Hook to watch instances of a specific CRD
/// The crd parameter is optional - when None, no watch is started
pub fn use_watch_crd_instances(
    cluster: Signal<Option<ClusterContext>>,
    namespace: Signal<Option<String>>,
    crd: Signal<Option<CrdInfo>>,
) -> Signal<WatchedResourceState<DynamicObject>> {
    let mut state = use_signal(WatchedResourceState::default);
    let cancel_token = use_hook(|| Arc::new(std::sync::Mutex::new(CancellationToken::new())));

    use_effect(move || {
        let cluster_value = cluster.read().clone();
        let namespace_value = namespace.read().clone();
        let crd_value = crd.read().clone();

        let new_token = CancellationToken::new();
        {
            let mut guard = cancel_token.lock().unwrap();
            guard.cancel();
            *guard = new_token.clone();
        }
        let token = new_token;

        state.set(WatchedResourceState::default());

        spawn(async move {
            let (ctx, crd_info) = match (cluster_value, crd_value) {
                (Some(ctx), Some(crd)) => (ctx, crd),
                _ => return,
            };

            if token.is_cancelled() {
                return;
            }

            let ns_ref = namespace_value.as_deref().filter(|s| !s.is_empty());
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

                let mut stream = ctx.client.watch_crd_instances(&crd_info, ns_ref);
                let mut resources: HashMap<String, DynamicObject> = HashMap::new();
                let mut errored = false;

                loop {
                    let event = tokio::select! {
                        _ = token.cancelled() => {
                            tracing::debug!("CRD instance watcher cancelled");
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
                                tracing::error!("CRD instance watch error: {}", e);
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

                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    tracing::warn!(
                        "CRD instance watch gave up after {} consecutive errors",
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
                    "CRD instance watch retry in {}s ({}/{})",
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
        });
    });

    state
}
