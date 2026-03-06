//! Configuration resource operations: ConfigMap, Secret

use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DeleteParams, ListParams};

use super::KubeClient;

impl KubeClient {
    // === ConfigMap Operations ===

    /// List configmaps in a namespace or across all namespaces
    pub async fn list_configmaps(&self, namespace: Option<&str>) -> SkdResult<Vec<ConfigMap>> {
        let api: Api<ConfigMap> = match namespace {
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

    /// Delete a configmap by name and namespace
    pub async fn delete_configmap(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<ConfigMap> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === Secret Operations ===

    /// List secrets in a namespace or across all namespaces
    pub async fn list_secrets(&self, namespace: Option<&str>) -> SkdResult<Vec<Secret>> {
        let api: Api<Secret> = match namespace {
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

    /// Delete a secret by name and namespace
    pub async fn delete_secret(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Secret> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }
}
