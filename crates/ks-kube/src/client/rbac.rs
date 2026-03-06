//! RBAC resource operations: Role, ClusterRole, RoleBinding, ClusterRoleBinding

use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, ListParams};

use super::KubeClient;

impl KubeClient {
    /// List Roles in a namespace or across all namespaces
    pub async fn list_roles(&self, namespace: Option<&str>) -> SkdResult<Vec<Role>> {
        let api: Api<Role> = match namespace {
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

    /// List ClusterRoles (cluster-scoped)
    pub async fn list_clusterroles(&self) -> SkdResult<Vec<ClusterRole>> {
        let api: Api<ClusterRole> = Api::all(self.inner.clone());
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(list.items)
    }

    /// List RoleBindings in a namespace or across all namespaces
    pub async fn list_rolebindings(&self, namespace: Option<&str>) -> SkdResult<Vec<RoleBinding>> {
        let api: Api<RoleBinding> = match namespace {
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

    /// List ClusterRoleBindings (cluster-scoped)
    pub async fn list_clusterrolebindings(&self) -> SkdResult<Vec<ClusterRoleBinding>> {
        let api: Api<ClusterRoleBinding> = Api::all(self.inner.clone());
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
