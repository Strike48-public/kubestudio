//! Storage resource operations: PersistentVolume, PersistentVolumeClaim, StorageClass

use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim};
use k8s_openapi::api::storage::v1::StorageClass;
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DeleteParams, ListParams};

use super::KubeClient;

impl KubeClient {
    // === PersistentVolume Operations ===

    /// List persistent volumes (cluster-scoped)
    pub async fn list_persistentvolumes(&self) -> SkdResult<Vec<PersistentVolume>> {
        let api: Api<PersistentVolume> = Api::all(self.inner.clone());

        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// Delete a persistent volume by name (cluster-scoped)
    pub async fn delete_persistentvolume(&self, name: &str) -> SkdResult<()> {
        let api: Api<PersistentVolume> = Api::all(self.inner.clone());
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === PersistentVolumeClaim Operations ===

    /// List persistent volume claims in a namespace or across all namespaces
    pub async fn list_persistentvolumeclaims(
        &self,
        namespace: Option<&str>,
    ) -> SkdResult<Vec<PersistentVolumeClaim>> {
        let api: Api<PersistentVolumeClaim> = match namespace {
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

    /// Delete a persistent volume claim by name and namespace
    pub async fn delete_persistentvolumeclaim(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<PersistentVolumeClaim> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === StorageClass Operations ===

    /// List storageclasses (cluster-scoped)
    pub async fn list_storageclasses(&self) -> SkdResult<Vec<StorageClass>> {
        let api: Api<StorageClass> = Api::all(self.inner.clone());

        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// Delete a storageclass by name
    pub async fn delete_storageclass(&self, name: &str) -> SkdResult<()> {
        let api: Api<StorageClass> = Api::all(self.inner.clone());
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }
}
