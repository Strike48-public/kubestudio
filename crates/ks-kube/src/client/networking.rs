//! Networking resource operations: Service, Endpoints, Ingress

use k8s_openapi::api::core::v1::{Endpoints, Service};
use k8s_openapi::api::networking::v1::Ingress;
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DeleteParams, ListParams};

use super::KubeClient;

impl KubeClient {
    // === Service Operations ===

    /// List services in a namespace or across all namespaces
    pub async fn list_services(&self, namespace: Option<&str>) -> SkdResult<Vec<Service>> {
        let api: Api<Service> = match namespace {
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

    /// Delete a service by name and namespace
    pub async fn delete_service(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Service> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === Endpoints Operations ===

    /// List endpoints in a namespace or across all namespaces
    pub async fn list_endpoints(&self, namespace: Option<&str>) -> SkdResult<Vec<Endpoints>> {
        let api: Api<Endpoints> = match namespace {
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

    /// Delete an endpoint by name and namespace
    pub async fn delete_endpoint(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Endpoints> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === Ingress Operations ===

    /// List ingresses in a namespace or across all namespaces
    pub async fn list_ingresses(&self, namespace: Option<&str>) -> SkdResult<Vec<Ingress>> {
        let api: Api<Ingress> = match namespace {
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

    /// Delete an ingress by name and namespace
    pub async fn delete_ingress(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Ingress> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }
}
