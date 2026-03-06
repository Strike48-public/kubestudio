//! Watch stream operations for real-time resource updates

use std::fmt::Debug;

use futures::StreamExt;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Event, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret,
    Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::Resource;
use kube::api::Api;
use kube::runtime::watcher::{self, Config as WatcherConfig};

use super::KubeClient;
use super::types::{WatchEvent, WatchStream};

impl KubeClient {
    /// Watch pods in a namespace or across all namespaces
    pub fn watch_pods(&self, namespace: Option<&str>) -> WatchStream<Pod> {
        let api: Api<Pod> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch deployments in a namespace or across all namespaces
    pub fn watch_deployments(&self, namespace: Option<&str>) -> WatchStream<Deployment> {
        let api: Api<Deployment> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch statefulsets in a namespace or across all namespaces
    pub fn watch_statefulsets(&self, namespace: Option<&str>) -> WatchStream<StatefulSet> {
        let api: Api<StatefulSet> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch daemonsets in a namespace or across all namespaces
    pub fn watch_daemonsets(&self, namespace: Option<&str>) -> WatchStream<DaemonSet> {
        let api: Api<DaemonSet> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch cronjobs in a namespace or across all namespaces
    pub fn watch_cronjobs(&self, namespace: Option<&str>) -> WatchStream<CronJob> {
        let api: Api<CronJob> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch jobs in a namespace or across all namespaces
    pub fn watch_jobs(&self, namespace: Option<&str>) -> WatchStream<Job> {
        let api: Api<Job> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch configmaps in a namespace or across all namespaces
    pub fn watch_configmaps(&self, namespace: Option<&str>) -> WatchStream<ConfigMap> {
        let api: Api<ConfigMap> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch secrets in a namespace or across all namespaces
    pub fn watch_secrets(&self, namespace: Option<&str>) -> WatchStream<Secret> {
        let api: Api<Secret> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch services in a namespace or across all namespaces
    pub fn watch_services(&self, namespace: Option<&str>) -> WatchStream<Service> {
        let api: Api<Service> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch endpoints in a namespace or across all namespaces
    pub fn watch_endpoints(&self, namespace: Option<&str>) -> WatchStream<Endpoints> {
        let api: Api<Endpoints> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch ingresses in a namespace or across all namespaces
    pub fn watch_ingresses(&self, namespace: Option<&str>) -> WatchStream<Ingress> {
        let api: Api<Ingress> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch persistent volume claims in a namespace or across all namespaces
    pub fn watch_persistentvolumeclaims(
        &self,
        namespace: Option<&str>,
    ) -> WatchStream<PersistentVolumeClaim> {
        let api: Api<PersistentVolumeClaim> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch events in a namespace or across all namespaces
    pub fn watch_events(&self, namespace: Option<&str>) -> WatchStream<Event> {
        let api: Api<Event> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch persistent volumes (cluster-scoped)
    pub fn watch_persistentvolumes(&self) -> WatchStream<PersistentVolume> {
        let api: Api<PersistentVolume> = Api::all(self.inner.clone());
        Self::create_watch_stream(api)
    }

    /// Watch storage classes (cluster-scoped)
    pub fn watch_storageclasses(&self) -> WatchStream<StorageClass> {
        let api: Api<StorageClass> = Api::all(self.inner.clone());
        Self::create_watch_stream(api)
    }

    /// Watch nodes (cluster-scoped)
    pub fn watch_nodes(&self) -> WatchStream<Node> {
        let api: Api<Node> = Api::all(self.inner.clone());
        Self::create_watch_stream(api)
    }

    /// Watch Roles in a namespace or across all namespaces
    pub fn watch_roles(&self, namespace: Option<&str>) -> WatchStream<Role> {
        let api: Api<Role> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch ClusterRoles (cluster-scoped)
    pub fn watch_clusterroles(&self) -> WatchStream<ClusterRole> {
        let api: Api<ClusterRole> = Api::all(self.inner.clone());
        Self::create_watch_stream(api)
    }

    /// Watch RoleBindings in a namespace or across all namespaces
    pub fn watch_rolebindings(&self, namespace: Option<&str>) -> WatchStream<RoleBinding> {
        let api: Api<RoleBinding> = match namespace {
            Some(ns) => Api::namespaced(self.inner.clone(), ns),
            None => Api::all(self.inner.clone()),
        };
        Self::create_watch_stream(api)
    }

    /// Watch ClusterRoleBindings (cluster-scoped)
    pub fn watch_clusterrolebindings(&self) -> WatchStream<ClusterRoleBinding> {
        let api: Api<ClusterRoleBinding> = Api::all(self.inner.clone());
        Self::create_watch_stream(api)
    }

    /// Create a watch stream for any resource type
    /// Handles conversion of kube-rs watcher events to our WatchEvent type
    pub(crate) fn create_watch_stream<K>(api: Api<K>) -> WatchStream<K>
    where
        K: Resource + Clone + Debug + Send + Sync + 'static,
        K: serde::de::DeserializeOwned,
        K::DynamicType: Default,
    {
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
}
