//! Kubernetes client implementation with modular operations
//!
//! This module provides a high-level client for interacting with Kubernetes clusters.
//! Operations are organized by resource category:
//! - `workloads`: Pod, Deployment, StatefulSet, DaemonSet, Job, CronJob
//! - `config`: ConfigMap, Secret
//! - `networking`: Service, Endpoints, Ingress
//! - `storage`: PersistentVolume, PersistentVolumeClaim, StorageClass
//! - `rbac`: Role, ClusterRole, RoleBinding, ClusterRoleBinding
//! - `cluster`: Namespace, Node, Event
//! - `watch`: Real-time watch streams for all resource types
//! - `drilldown`: Methods to fetch related resources
//! - `apply`: YAML apply operations
//! - `exec`: Pod exec and port-forward
//! - `yaml`: YAML serialization

mod apply;
mod cluster;
mod config;
pub mod crd;
mod drilldown;
mod exec;
mod metrics;
mod networking;
mod rbac;
mod storage;
pub mod types;
mod watch;
mod workloads;
mod yaml;

pub use crd::{CrdInfo, CrdScope, PrinterColumn};
pub use metrics::{ContainerMetrics, NodeMetrics, PodMetrics};
pub use types::{
    ApplyResult, ExecHandle, MultiApplyResult, PortForwardHandle, PortForwardInfo, WatchEvent,
    WatchStream,
};

use ks_core::{SkdError, SkdResult};
use kube::{Client, Config};

/// High-level Kubernetes client
pub struct KubeClient {
    pub(crate) inner: Client,
    cluster_name: String,
}

impl KubeClient {
    /// Create a new KubeClient from a kubeconfig context.
    /// Falls back to in-cluster config when kubeconfig-based connection fails
    /// and the context matches the synthetic in-cluster context.
    pub async fn from_context(context_name: &str) -> SkdResult<Self> {
        let kubeconfig_result = Config::from_kubeconfig(&kube::config::KubeConfigOptions {
            context: Some(context_name.to_string()),
            ..Default::default()
        })
        .await;

        let config = match kubeconfig_result {
            Ok(c) => c,
            Err(_) if Config::incluster().is_ok() => {
                tracing::info!("Using in-cluster config for context '{}'", context_name);
                Config::incluster().map_err(|e| SkdError::Kubeconfig(e.to_string()))?
            }
            Err(e) => return Err(SkdError::Kubeconfig(e.to_string())),
        };

        let client = Client::try_from(config).map_err(|e| SkdError::ClusterConnection {
            cluster_name: context_name.to_string(),
            message: e.to_string(),
        })?;

        Ok(Self {
            inner: client,
            cluster_name: context_name.to_string(),
        })
    }

    /// Get a reference to the underlying kube Client
    pub fn client(&self) -> &Client {
        &self.inner
    }

    /// Get the cluster name (context name used to create this client)
    pub fn cluster_name(&self) -> &str {
        &self.cluster_name
    }
}
