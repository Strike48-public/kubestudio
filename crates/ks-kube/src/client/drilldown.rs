//! Drill-down operations for fetching related resources

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{Endpoints, Pod, Service};
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, ListParams};

use super::KubeClient;

impl KubeClient {
    /// List pods belonging to a Deployment using its label selector
    pub async fn list_pods_for_deployment(
        &self,
        deployment_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Pod>> {
        let deploy_api: Api<Deployment> = Api::namespaced(self.inner.clone(), namespace);
        let deployment = deploy_api
            .get(deployment_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        let selector = deployment
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .map(|labels| {
                labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default().labels(&selector))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// List pods belonging to a StatefulSet using its label selector
    pub async fn list_pods_for_statefulset(
        &self,
        statefulset_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Pod>> {
        let sts_api: Api<StatefulSet> = Api::namespaced(self.inner.clone(), namespace);
        let statefulset = sts_api
            .get(statefulset_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        let selector = statefulset
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .map(|labels| {
                labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default().labels(&selector))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// List pods belonging to a DaemonSet using its label selector
    pub async fn list_pods_for_daemonset(
        &self,
        daemonset_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Pod>> {
        let ds_api: Api<DaemonSet> = Api::namespaced(self.inner.clone(), namespace);
        let daemonset = ds_api
            .get(daemonset_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        let selector = daemonset
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref())
            .map(|labels| {
                labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default().labels(&selector))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// List pods belonging to a Job using its label selector
    pub async fn list_pods_for_job(&self, job_name: &str, namespace: &str) -> SkdResult<Vec<Pod>> {
        let job_api: Api<Job> = Api::namespaced(self.inner.clone(), namespace);
        let job = job_api.get(job_name).await.map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: e.to_string(),
        })?;

        let selector = job
            .spec
            .as_ref()
            .and_then(|s| s.selector.as_ref())
            .and_then(|s| s.match_labels.as_ref())
            .map(|labels| {
                labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default().labels(&selector))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// List Jobs triggered by a CronJob
    pub async fn list_jobs_for_cronjob(
        &self,
        cronjob_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Job>> {
        let job_api: Api<Job> = Api::namespaced(self.inner.clone(), namespace);
        let list = job_api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        // Filter jobs that have this CronJob as owner
        let jobs: Vec<Job> = list
            .items
            .into_iter()
            .filter(|job| {
                job.metadata
                    .owner_references
                    .as_ref()
                    .map(|refs| {
                        refs.iter()
                            .any(|r| r.kind == "CronJob" && r.name == cronjob_name)
                    })
                    .unwrap_or(false)
            })
            .collect();

        Ok(jobs)
    }

    /// Get a single Service by name
    pub async fn get_service(&self, name: &str, namespace: &str) -> SkdResult<Service> {
        let api: Api<Service> = Api::namespaced(self.inner.clone(), namespace);
        api.get(name).await.map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: e.to_string(),
        })
    }

    /// List pods that are endpoints for a Service
    pub async fn list_pods_for_service(
        &self,
        service_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Pod>> {
        let svc_api: Api<Service> = Api::namespaced(self.inner.clone(), namespace);
        let service = svc_api
            .get(service_name)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        let selector = service
            .spec
            .as_ref()
            .and_then(|s| s.selector.as_ref())
            .map(|labels| {
                labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        if selector.is_empty() {
            // No selector — try the Endpoints object for pod references
            return self
                .list_pods_from_endpoints(service_name, namespace)
                .await;
        }

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default().labels(&selector))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(list.items)
    }

    /// Fetch pods referenced by an Endpoints object (for services without selectors)
    async fn list_pods_from_endpoints(
        &self,
        endpoints_name: &str,
        namespace: &str,
    ) -> SkdResult<Vec<Pod>> {
        let ep_api: Api<Endpoints> = Api::namespaced(self.inner.clone(), namespace);
        let ep = match ep_api.get(endpoints_name).await {
            Ok(ep) => ep,
            Err(_) => return Ok(vec![]),
        };

        let subsets = ep.subsets.unwrap_or_default();

        // First try: resolve pods via targetRef (direct pod references)
        let mut pod_names: Vec<String> = Vec::new();
        let mut endpoint_ips: Vec<String> = Vec::new();

        for subset in &subsets {
            for addr in subset.addresses.as_deref().unwrap_or_default() {
                if let Some(tr) = &addr.target_ref {
                    if tr.kind.as_deref() == Some("Pod") {
                        if let Some(name) = &tr.name {
                            pod_names.push(name.clone());
                        }
                    }
                }
                // Collect IPs as fallback
                endpoint_ips.push(addr.ip.clone());
            }
        }

        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);

        // If we have direct pod names, fetch them
        if !pod_names.is_empty() {
            let mut pods = Vec::new();
            for name in &pod_names {
                if let Ok(pod) = pod_api.get(name).await {
                    pods.push(pod);
                }
            }
            return Ok(pods);
        }

        // Fallback: match endpoint IPs against pod IPs in the namespace
        if !endpoint_ips.is_empty() {
            let all_pods = pod_api
                .list(&ListParams::default())
                .await
                .map_err(|e| SkdError::KubeApi {
                    status_code: 500,
                    message: e.to_string(),
                })?;

            let pods: Vec<Pod> = all_pods
                .items
                .into_iter()
                .filter(|pod| {
                    pod.status
                        .as_ref()
                        .and_then(|s| s.pod_ips.as_ref())
                        .map(|ips| {
                            ips.iter().any(|pip| endpoint_ips.contains(&pip.ip))
                        })
                        .unwrap_or(false)
                })
                .collect();

            return Ok(pods);
        }

        Ok(vec![])
    }

    /// List pods using a PersistentVolumeClaim
    pub async fn list_pods_for_pvc(&self, pvc_name: &str, namespace: &str) -> SkdResult<Vec<Pod>> {
        let pod_api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        let list = pod_api
            .list(&ListParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        let pods: Vec<Pod> = list
            .items
            .into_iter()
            .filter(|pod| {
                pod.spec
                    .as_ref()
                    .and_then(|s| s.volumes.as_ref())
                    .map(|volumes| {
                        volumes.iter().any(|v| {
                            v.persistent_volume_claim
                                .as_ref()
                                .map(|pvc| pvc.claim_name == pvc_name)
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
            .collect();

        Ok(pods)
    }
}
