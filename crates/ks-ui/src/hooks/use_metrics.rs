//! Hook for polling resource metrics from metrics-server

use dioxus::prelude::*;
use ks_kube::NodeMetrics;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::use_cluster::ClusterContext;

/// Polling interval for metrics in milliseconds
const METRICS_POLL_INTERVAL_MS: u64 = 30_000;

/// State for node metrics
#[derive(Clone, Default)]
pub struct MetricsState {
    /// Node name -> NodeMetrics mapping
    pub nodes: HashMap<String, NodeMetrics>,
    /// Whether metrics-server is available
    pub available: bool,
    /// Last error message, if any
    pub error: Option<String>,
}

/// Hook that polls node metrics from metrics-server
/// Returns a signal with the current metrics state
pub fn use_node_metrics(cluster: Signal<Option<ClusterContext>>) -> Signal<MetricsState> {
    let mut metrics_state = use_signal(MetricsState::default);

    // Monotonic generation counter — each new effect invocation bumps this so
    // the previous polling task knows to stop. Avoids overlapping tasks that
    // leak file descriptors.
    let poll_gen = use_hook(|| Arc::new(std::sync::atomic::AtomicU64::new(0)));

    use_effect(move || {
        let cluster_ctx = cluster.read().clone();

        // Bump generation to cancel any previous polling task
        let my_gen = poll_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let poll_gen = poll_gen.clone();

        if let Some(ctx) = cluster_ctx {
            let client = ctx.client.clone();

            spawn(async move {
                // Check if metrics-server is available first
                if poll_gen.load(Ordering::SeqCst) != my_gen {
                    return;
                }
                let available = client.has_metrics_server().await;

                if !available {
                    metrics_state.set(MetricsState {
                        nodes: HashMap::new(),
                        available: false,
                        error: None,
                    });
                    return;
                }

                loop {
                    if poll_gen.load(Ordering::SeqCst) != my_gen {
                        tracing::debug!("Metrics polling stopped: superseded");
                        break;
                    }

                    match client.get_node_metrics().await {
                        Ok(node_metrics) => {
                            let nodes: HashMap<String, NodeMetrics> = node_metrics
                                .into_iter()
                                .map(|m| (m.name.clone(), m))
                                .collect();

                            metrics_state.set(MetricsState {
                                nodes,
                                available: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            tracing::debug!("Failed to fetch node metrics: {}", e);
                            metrics_state.set(MetricsState {
                                nodes: HashMap::new(),
                                available: false,
                                error: Some(e.to_string()),
                            });
                        }
                    }

                    // Sleep in small increments so we can respond to cancellation quickly
                    for _ in 0..(METRICS_POLL_INTERVAL_MS / 500) {
                        if poll_gen.load(Ordering::SeqCst) != my_gen {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            });
        } else {
            metrics_state.set(MetricsState::default());
        }
    });

    metrics_state
}
