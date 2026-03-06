//! Resource metrics from metrics-server
//!
//! Provides access to live CPU and memory usage metrics for nodes and pods.

use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DynamicObject, ListParams};
use kube::discovery::ApiResource;
use serde::Deserialize;

use super::KubeClient;

/// Node metrics from metrics-server
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeMetrics {
    pub name: String,
    pub cpu_usage: String,
    pub memory_usage: String,
}

/// Pod metrics from metrics-server
#[derive(Debug, Clone, PartialEq)]
pub struct PodMetrics {
    pub name: String,
    pub namespace: String,
    pub containers: Vec<ContainerMetrics>,
}

/// Container metrics from metrics-server
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerMetrics {
    pub name: String,
    pub cpu_usage: String,
    pub memory_usage: String,
}

/// Raw structures for parsing dynamic objects
#[derive(Debug, Deserialize)]
struct ResourceUsage {
    cpu: String,
    memory: String,
}

#[derive(Debug, Deserialize)]
struct ContainerMetricItem {
    name: String,
    usage: ResourceUsage,
}

/// ApiResource for NodeMetrics
fn node_metrics_api_resource() -> ApiResource {
    ApiResource {
        group: "metrics.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "metrics.k8s.io/v1beta1".to_string(),
        kind: "NodeMetrics".to_string(),
        plural: "nodes".to_string(),
    }
}

/// ApiResource for PodMetrics
fn pod_metrics_api_resource() -> ApiResource {
    ApiResource {
        group: "metrics.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "metrics.k8s.io/v1beta1".to_string(),
        kind: "PodMetrics".to_string(),
        plural: "pods".to_string(),
    }
}

impl KubeClient {
    /// Check if metrics-server is available in the cluster
    pub async fn has_metrics_server(&self) -> bool {
        // Try to list node metrics - if it works, metrics-server is available
        let api: Api<DynamicObject> =
            Api::all_with(self.inner.clone(), &node_metrics_api_resource());
        api.list(&ListParams::default().limit(1)).await.is_ok()
    }

    /// Get metrics for all nodes
    pub async fn get_node_metrics(&self) -> SkdResult<Vec<NodeMetrics>> {
        let api: Api<DynamicObject> =
            Api::all_with(self.inner.clone(), &node_metrics_api_resource());

        let list = api.list(&ListParams::default()).await.map_err(|e| {
            if e.to_string().contains("404") || e.to_string().contains("not found") {
                SkdError::KubeApi {
                    status_code: 404,
                    message: "Metrics server not available".to_string(),
                }
            } else {
                SkdError::KubeApi {
                    status_code: 500,
                    message: e.to_string(),
                }
            }
        })?;

        Ok(list
            .items
            .into_iter()
            .filter_map(|obj| {
                let name = obj.metadata.name?;
                let usage: ResourceUsage =
                    serde_json::from_value(obj.data.get("usage")?.clone()).ok()?;
                Some(NodeMetrics {
                    name,
                    cpu_usage: format_cpu(&usage.cpu),
                    memory_usage: format_memory(&usage.memory),
                })
            })
            .collect())
    }

    /// Get metrics for pods in a namespace, or all namespaces if None
    pub async fn get_pod_metrics(&self, namespace: Option<&str>) -> SkdResult<Vec<PodMetrics>> {
        let api: Api<DynamicObject> = match namespace {
            Some(ns) => Api::namespaced_with(self.inner.clone(), ns, &pod_metrics_api_resource()),
            None => Api::all_with(self.inner.clone(), &pod_metrics_api_resource()),
        };

        let list = api.list(&ListParams::default()).await.map_err(|e| {
            if e.to_string().contains("404") || e.to_string().contains("not found") {
                SkdError::KubeApi {
                    status_code: 404,
                    message: "Metrics server not available".to_string(),
                }
            } else {
                SkdError::KubeApi {
                    status_code: 500,
                    message: e.to_string(),
                }
            }
        })?;

        Ok(list
            .items
            .into_iter()
            .filter_map(|obj| {
                let name = obj.metadata.name?;
                let namespace = obj.metadata.namespace.unwrap_or_default();
                let containers_value = obj.data.get("containers")?;
                let container_items: Vec<ContainerMetricItem> =
                    serde_json::from_value(containers_value.clone()).ok()?;

                Some(PodMetrics {
                    name,
                    namespace,
                    containers: container_items
                        .into_iter()
                        .map(|c| ContainerMetrics {
                            name: c.name,
                            cpu_usage: format_cpu(&c.usage.cpu),
                            memory_usage: format_memory(&c.usage.memory),
                        })
                        .collect(),
                })
            })
            .collect())
    }
}

/// Format CPU usage (e.g., "250000000n" -> "250m")
fn format_cpu(cpu: &str) -> String {
    // CPU is typically in nanocores (e.g., "123456789n")
    if let Some(nano_str) = cpu.strip_suffix('n')
        && let Ok(nano) = nano_str.parse::<u64>()
    {
        let milli = nano / 1_000_000;
        return format!("{}m", milli);
    }
    // Already in milli format or other
    cpu.to_string()
}

/// Format memory usage (e.g., "1234567890" -> "1.2Gi")
fn format_memory(memory: &str) -> String {
    // Memory can be in various formats: bytes, Ki, Mi, Gi
    // Try to parse as raw bytes first
    if let Ok(bytes) = memory.parse::<u64>() {
        if bytes >= 1024 * 1024 * 1024 {
            return format!("{:.1}Gi", bytes as f64 / (1024.0 * 1024.0 * 1024.0));
        } else if bytes >= 1024 * 1024 {
            return format!("{:.0}Mi", bytes as f64 / (1024.0 * 1024.0));
        } else if bytes >= 1024 {
            return format!("{:.0}Ki", bytes as f64 / 1024.0);
        } else {
            return format!("{}", bytes);
        }
    }

    // Handle Ki suffix
    if let Some(ki_str) = memory.strip_suffix("Ki")
        && let Ok(ki) = ki_str.parse::<u64>()
    {
        if ki >= 1024 * 1024 {
            return format!("{:.1}Gi", ki as f64 / (1024.0 * 1024.0));
        } else if ki >= 1024 {
            return format!("{:.0}Mi", ki as f64 / 1024.0);
        }
        return format!("{}Ki", ki);
    }

    // Already formatted or other format
    memory.to_string()
}
