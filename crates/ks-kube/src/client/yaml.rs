//! YAML serialization operations for resources

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Event, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret,
    Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use ks_core::{SkdError, SkdResult};
use kube::api::Api;
use serde::Serialize;

use super::KubeClient;
use super::crd::CrdInfo;

impl KubeClient {
    /// Get a resource as YAML string
    /// Supports all resource types with generic handling
    pub async fn get_resource_yaml(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> SkdResult<String> {
        async fn fetch_and_serialize<T>(api: Api<T>, name: &str) -> SkdResult<String>
        where
            T: k8s_openapi::Resource + Serialize + Clone + std::fmt::Debug,
            T: serde::de::DeserializeOwned,
        {
            let resource = api.get(name).await.map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

            serde_yaml::to_string(&resource).map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to serialize to YAML: {}", e),
            })
        }

        match kind {
            "Pod" => {
                let api: Api<Pod> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Deployment" => {
                let api: Api<Deployment> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "StatefulSet" => {
                let api: Api<StatefulSet> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "DaemonSet" => {
                let api: Api<DaemonSet> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Job" => {
                let api: Api<Job> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "CronJob" => {
                let api: Api<CronJob> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "ConfigMap" => {
                let api: Api<ConfigMap> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Secret" => {
                let api: Api<Secret> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Service" => {
                let api: Api<Service> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Endpoints" => {
                let api: Api<Endpoints> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Ingress" => {
                let api: Api<Ingress> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "PersistentVolume" => {
                let api: Api<PersistentVolume> = Api::all(self.inner.clone());
                fetch_and_serialize(api, name).await
            }
            "PersistentVolumeClaim" => {
                let api: Api<PersistentVolumeClaim> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "StorageClass" => {
                let api: Api<StorageClass> = Api::all(self.inner.clone());
                fetch_and_serialize(api, name).await
            }
            "Event" => {
                let api: Api<Event> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "Node" => {
                let api: Api<Node> = Api::all(self.inner.clone());
                fetch_and_serialize(api, name).await
            }
            "Role" => {
                let api: Api<Role> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "ClusterRole" => {
                let api: Api<ClusterRole> = Api::all(self.inner.clone());
                fetch_and_serialize(api, name).await
            }
            "RoleBinding" => {
                let api: Api<RoleBinding> = if let Some(ns) = namespace {
                    Api::namespaced(self.inner.clone(), ns)
                } else {
                    return Err(SkdError::ResourceNotFound {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        namespace: None,
                    });
                };
                fetch_and_serialize(api, name).await
            }
            "ClusterRoleBinding" => {
                let api: Api<ClusterRoleBinding> = Api::all(self.inner.clone());
                fetch_and_serialize(api, name).await
            }
            _ => Err(SkdError::KubeApi {
                status_code: 400,
                message: format!("Unsupported resource kind: {}", kind),
            }),
        }
    }

    /// Get a CRD instance as YAML string using CrdInfo
    pub async fn get_crd_instance_yaml(
        &self,
        crd: &CrdInfo,
        name: &str,
        namespace: Option<&str>,
    ) -> SkdResult<String> {
        let obj = self.get_crd_instance(crd, name, namespace).await?;
        serde_yaml::to_string(&obj).map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: format!("Failed to serialize to YAML: {}", e),
        })
    }

    /// Get a resource as YAML string, with CRD support
    /// If crd_info is provided, it will be used for unknown kinds
    pub async fn get_resource_yaml_with_crd(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
        crd_info: Option<&CrdInfo>,
    ) -> SkdResult<String> {
        // First try built-in types
        match self.get_resource_yaml(kind, name, namespace).await {
            Ok(yaml) => Ok(yaml),
            Err(SkdError::KubeApi {
                status_code: 400, ..
            }) => {
                // Unknown kind - try as CRD if info is provided
                if let Some(crd) = crd_info {
                    self.get_crd_instance_yaml(crd, name, namespace).await
                } else {
                    Err(SkdError::KubeApi {
                        status_code: 400,
                        message: format!("Unsupported resource kind: {}", kind),
                    })
                }
            }
            Err(e) => Err(e),
        }
    }
}
