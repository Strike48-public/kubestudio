//! Cluster-level resource operations: Namespace, Node, Event

use k8s_openapi::api::core::v1::{Event, Namespace, Node};
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, ListParams};

use super::KubeClient;

impl KubeClient {
    /// List all namespaces in the cluster
    pub async fn list_namespaces(&self) -> SkdResult<Vec<String>> {
        let api: Api<Namespace> = Api::all(self.inner.clone());
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list
            .items
            .into_iter()
            .filter_map(|ns| ns.metadata.name)
            .collect())
    }

    /// List all nodes in the cluster
    pub async fn list_nodes(&self) -> SkdResult<Vec<Node>> {
        let api: Api<Node> = Api::all(self.inner.clone());
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// List events in a namespace or across all namespaces
    pub async fn list_events(&self, namespace: Option<&str>) -> SkdResult<Vec<Event>> {
        let api: Api<Event> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };

        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }
}
