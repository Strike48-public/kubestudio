//! Custom Resource Definition (CRD) operations for dynamic resource discovery
//!
//! This module provides discovery and management of custom resources defined in the cluster.

use std::fmt::Debug;

use futures::StreamExt;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DeleteParams, DynamicObject, GroupVersionKind, ListParams};
use kube::discovery::ApiResource;
use kube::runtime::watcher::{self, Config as WatcherConfig};
use serde::{Deserialize, Serialize};

use super::KubeClient;
use super::types::{WatchEvent, WatchStream};

/// Scope of a custom resource (namespaced or cluster-wide)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrdScope {
    Namespaced,
    Cluster,
}

/// Column definition for displaying custom resources
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterColumn {
    pub name: String,
    pub json_path: String,
    pub column_type: String,
    pub priority: i32,
}

/// Information about a Custom Resource Definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrdInfo {
    /// Full CRD name (e.g., "certificates.cert-manager.io")
    pub name: String,
    /// API group (e.g., "cert-manager.io")
    pub group: String,
    /// Preferred/stored version (e.g., "v1")
    pub version: String,
    /// Kind name (e.g., "Certificate")
    pub kind: String,
    /// Plural name for API paths (e.g., "certificates")
    pub plural: String,
    /// Whether resources are namespaced or cluster-scoped
    pub scope: CrdScope,
    /// Short names for the resource (e.g., ["cert", "certs"])
    pub short_names: Vec<String>,
    /// Columns for displaying resources in tables
    pub printer_columns: Vec<PrinterColumn>,
}

impl CrdInfo {
    /// Create from a Kubernetes CRD resource
    pub fn from_crd(crd: &CustomResourceDefinition) -> Option<Self> {
        let spec = &crd.spec;
        let name = crd.metadata.name.clone()?;
        let group = spec.group.clone();
        let kind = spec.names.kind.clone();
        let plural = spec.names.plural.clone();

        // Find the preferred/stored version
        let version = spec
            .versions
            .iter()
            .find(|v| v.storage)
            .or_else(|| spec.versions.first())
            .map(|v| v.name.clone())?;

        let scope = match spec.scope.as_str() {
            "Namespaced" => CrdScope::Namespaced,
            _ => CrdScope::Cluster,
        };

        let short_names = spec.names.short_names.clone().unwrap_or_default();

        // Extract printer columns from the stored version
        let printer_columns = spec
            .versions
            .iter()
            .find(|v| v.storage)
            .or_else(|| spec.versions.first())
            .and_then(|v| v.additional_printer_columns.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|col| PrinterColumn {
                name: col.name,
                json_path: col.json_path,
                column_type: col.type_,
                priority: col.priority.unwrap_or(0),
            })
            .collect();

        Some(CrdInfo {
            name,
            group,
            version,
            kind,
            plural,
            scope,
            short_names,
            printer_columns,
        })
    }

    /// Get the GroupVersionKind for this CRD
    pub fn gvk(&self) -> GroupVersionKind {
        GroupVersionKind::gvk(&self.group, &self.version, &self.kind)
    }

    /// Get the ApiResource for dynamic API access
    pub fn api_resource(&self) -> ApiResource {
        ApiResource::from_gvk_with_plural(&self.gvk(), &self.plural)
    }
}

impl KubeClient {
    /// List all CRDs in the cluster
    pub async fn list_crds(&self) -> SkdResult<Vec<CrdInfo>> {
        let api: Api<CustomResourceDefinition> = Api::all(self.inner.clone());
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items.iter().filter_map(CrdInfo::from_crd).collect())
    }

    /// Watch CRDs for real-time updates
    pub fn watch_crds(&self) -> WatchStream<CustomResourceDefinition> {
        let api: Api<CustomResourceDefinition> = Api::all(self.inner.clone());
        let config = WatcherConfig::default();
        let stream = watcher::watcher(api, config);

        Box::pin(stream.map(|event| match event {
            Ok(watcher::Event::Apply(obj)) => WatchEvent::Applied(obj),
            Ok(watcher::Event::Delete(obj)) => WatchEvent::Deleted(obj),
            Ok(watcher::Event::Init) => WatchEvent::InitStarted,
            Ok(watcher::Event::InitApply(obj)) => WatchEvent::Applied(obj),
            Ok(watcher::Event::InitDone) => WatchEvent::InitDone,
            Err(e) => WatchEvent::Error(e.to_string()),
        }))
    }

    /// List instances of a custom resource
    pub async fn list_crd_instances(
        &self,
        crd: &CrdInfo,
        namespace: Option<&str>,
    ) -> SkdResult<Vec<DynamicObject>> {
        let ar = crd.api_resource();
        let api: Api<DynamicObject> = match (crd.scope, namespace) {
            (CrdScope::Namespaced, Some(ns)) => Api::namespaced_with(self.inner.clone(), ns, &ar),
            (CrdScope::Namespaced, None) => Api::all_with(self.inner.clone(), &ar),
            (CrdScope::Cluster, _) => Api::all_with(self.inner.clone(), &ar),
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

    /// Watch instances of a custom resource for real-time updates
    pub fn watch_crd_instances(
        &self,
        crd: &CrdInfo,
        namespace: Option<&str>,
    ) -> WatchStream<DynamicObject> {
        let ar = crd.api_resource();
        let api: Api<DynamicObject> = match (crd.scope, namespace) {
            (CrdScope::Namespaced, Some(ns)) => Api::namespaced_with(self.inner.clone(), ns, &ar),
            (CrdScope::Namespaced, None) => Api::all_with(self.inner.clone(), &ar),
            (CrdScope::Cluster, _) => Api::all_with(self.inner.clone(), &ar),
        };

        let config = WatcherConfig::default();
        let stream = watcher::watcher(api, config);

        Box::pin(stream.map(|event| match event {
            Ok(watcher::Event::Apply(obj)) => WatchEvent::Applied(obj),
            Ok(watcher::Event::Delete(obj)) => WatchEvent::Deleted(obj),
            Ok(watcher::Event::Init) => WatchEvent::InitStarted,
            Ok(watcher::Event::InitApply(obj)) => WatchEvent::Applied(obj),
            Ok(watcher::Event::InitDone) => WatchEvent::InitDone,
            Err(e) => WatchEvent::Error(e.to_string()),
        }))
    }

    /// Delete an instance of a custom resource
    pub async fn delete_crd_instance(
        &self,
        crd: &CrdInfo,
        name: &str,
        namespace: Option<&str>,
    ) -> SkdResult<()> {
        let ar = crd.api_resource();
        let api: Api<DynamicObject> = match (crd.scope, namespace) {
            (CrdScope::Namespaced, Some(ns)) => Api::namespaced_with(self.inner.clone(), ns, &ar),
            (CrdScope::Namespaced, None) => {
                return Err(SkdError::ResourceNotFound {
                    kind: crd.kind.clone(),
                    name: name.to_string(),
                    namespace: None,
                });
            }
            (CrdScope::Cluster, _) => Api::all_with(self.inner.clone(), &ar),
        };

        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Get a single CRD instance as a DynamicObject
    pub async fn get_crd_instance(
        &self,
        crd: &CrdInfo,
        name: &str,
        namespace: Option<&str>,
    ) -> SkdResult<DynamicObject> {
        let ar = crd.api_resource();
        let api: Api<DynamicObject> = match (crd.scope, namespace) {
            (CrdScope::Namespaced, Some(ns)) => Api::namespaced_with(self.inner.clone(), ns, &ar),
            (CrdScope::Namespaced, None) => {
                return Err(SkdError::ResourceNotFound {
                    kind: crd.kind.clone(),
                    name: name.to_string(),
                    namespace: None,
                });
            }
            (CrdScope::Cluster, _) => Api::all_with(self.inner.clone(), &ar),
        };

        api.get(name).await.map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: e.to_string(),
        })
    }
}
