//! Workload resource operations: Pod, Deployment, StatefulSet, DaemonSet, Job, CronJob

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::Pod;
use ks_core::{SkdError, SkdResult};
use kube::api::{Api, DeleteParams, ListParams, LogParams, Patch, PatchParams};
use tokio_util::compat::FuturesAsyncReadCompatExt;

use super::KubeClient;

impl KubeClient {
    // === Pod Operations ===

    /// List pods in a namespace or across all namespaces
    pub async fn list_pods(&self, namespace: Option<&str>) -> SkdResult<Vec<Pod>> {
        let api: Api<Pod> = match namespace {
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

    /// Get pod logs as a string
    pub async fn get_pod_logs(
        &self,
        name: &str,
        namespace: &str,
        container: Option<&str>,
    ) -> SkdResult<String> {
        let api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);

        let mut params = LogParams {
            follow: false,
            ..Default::default()
        };
        if let Some(c) = container {
            params.container = Some(c.to_string());
        }

        let logs = api
            .logs(name, &params)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(logs)
    }

    /// Stream pod logs in real-time
    pub async fn stream_pod_logs(
        &self,
        name: &str,
        namespace: &str,
        container: Option<&str>,
        tail_lines: Option<i64>,
        timestamps: bool,
    ) -> SkdResult<impl tokio::io::AsyncBufRead + Unpin> {
        let api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);

        let mut params = LogParams {
            follow: true,
            timestamps,
            tail_lines,
            ..Default::default()
        };
        if let Some(c) = container {
            params.container = Some(c.to_string());
        }

        let stream = api
            .log_stream(name, &params)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;

        Ok(tokio::io::BufReader::new(stream.compat()))
    }

    /// Delete a pod by name and namespace
    pub async fn delete_pod(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Pod> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === Deployment Operations ===

    /// List deployments in a namespace or across all namespaces
    pub async fn list_deployments(&self, namespace: Option<&str>) -> SkdResult<Vec<Deployment>> {
        let api: Api<Deployment> = match namespace {
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

    /// Delete a deployment by name and namespace
    pub async fn delete_deployment(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Deployment> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Scale a deployment to a specific number of replicas
    pub async fn scale_deployment(
        &self,
        name: &str,
        namespace: &str,
        replicas: i32,
    ) -> SkdResult<i32> {
        let api: Api<Deployment> = Api::namespaced(self.inner.clone(), namespace);

        let patch = serde_json::json!({
            "spec": {
                "replicas": replicas
            }
        });

        let patch_params = PatchParams::default();
        let result = api
            .patch(name, &patch_params, &Patch::Merge(&patch))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to scale deployment: {}", e),
            })?;

        Ok(result.spec.and_then(|s| s.replicas).unwrap_or(replicas))
    }

    /// Get current replica count for a deployment
    pub async fn get_deployment_replicas(&self, name: &str, namespace: &str) -> SkdResult<i32> {
        let api: Api<Deployment> = Api::namespaced(self.inner.clone(), namespace);
        let deployment = api.get(name).await.map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: e.to_string(),
        })?;

        Ok(deployment.spec.and_then(|s| s.replicas).unwrap_or(1))
    }

    /// Restart a deployment (triggers rolling update)
    pub async fn restart_deployment(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Deployment> = Api::namespaced(self.inner.clone(), namespace);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        let patch = serde_json::json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": {
                            "kubectl.kubernetes.io/restartedAt": now
                        }
                    }
                }
            }
        });

        let patch_params = PatchParams::default();
        api.patch(name, &patch_params, &Patch::Merge(&patch))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to restart deployment: {}", e),
            })?;

        Ok(())
    }

    // === StatefulSet Operations ===

    /// List statefulsets in a namespace or across all namespaces
    pub async fn list_statefulsets(&self, namespace: Option<&str>) -> SkdResult<Vec<StatefulSet>> {
        let api: Api<StatefulSet> = match namespace {
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

    /// Delete a statefulset by name and namespace
    pub async fn delete_statefulset(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<StatefulSet> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Restart a statefulset (triggers rolling update)
    pub async fn restart_statefulset(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<StatefulSet> = Api::namespaced(self.inner.clone(), namespace);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        let patch = serde_json::json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": {
                            "kubectl.kubernetes.io/restartedAt": now
                        }
                    }
                }
            }
        });

        let patch_params = PatchParams::default();
        api.patch(name, &patch_params, &Patch::Merge(&patch))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to restart statefulset: {}", e),
            })?;

        Ok(())
    }

    // === DaemonSet Operations ===

    /// List daemonsets in a namespace or across all namespaces
    pub async fn list_daemonsets(&self, namespace: Option<&str>) -> SkdResult<Vec<DaemonSet>> {
        let api: Api<DaemonSet> = match namespace {
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

    /// Delete a daemonset by name and namespace
    pub async fn delete_daemonset(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<DaemonSet> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Restart a daemonset (triggers rolling update)
    pub async fn restart_daemonset(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<DaemonSet> = Api::namespaced(self.inner.clone(), namespace);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        let patch = serde_json::json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": {
                            "kubectl.kubernetes.io/restartedAt": now
                        }
                    }
                }
            }
        });

        let patch_params = PatchParams::default();
        api.patch(name, &patch_params, &Patch::Merge(&patch))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to restart daemonset: {}", e),
            })?;

        Ok(())
    }

    // === Job Operations ===

    /// List jobs in a namespace or across all namespaces
    pub async fn list_jobs(&self, namespace: Option<&str>) -> SkdResult<Vec<Job>> {
        let api: Api<Job> = match namespace {
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

    /// Delete a job by name and namespace
    pub async fn delete_job(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<Job> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    // === CronJob Operations ===

    /// List cronjobs in a namespace or across all namespaces
    pub async fn list_cronjobs(&self, namespace: Option<&str>) -> SkdResult<Vec<CronJob>> {
        let api: Api<CronJob> = match namespace {
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

    /// Delete a cronjob by name and namespace
    pub async fn delete_cronjob(&self, name: &str, namespace: &str) -> SkdResult<()> {
        let api: Api<CronJob> = Api::namespaced(self.inner.clone(), namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: e.to_string(),
            })?;
        Ok(())
    }

    /// Trigger a manual job run from a CronJob
    pub async fn trigger_cronjob(&self, name: &str, namespace: &str) -> SkdResult<String> {
        let cronjob_api: Api<CronJob> = Api::namespaced(self.inner.clone(), namespace);
        let job_api: Api<Job> = Api::namespaced(self.inner.clone(), namespace);

        let cronjob = cronjob_api.get(name).await.map_err(|e| SkdError::KubeApi {
            status_code: 500,
            message: format!("Failed to get cronjob: {}", e),
        })?;

        let job_template = cronjob
            .spec
            .as_ref()
            .and_then(|s| s.job_template.spec.clone())
            .ok_or_else(|| SkdError::KubeApi {
                status_code: 400,
                message: "CronJob has no job template".to_string(),
            })?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let job_name = format!("{}-manual-{}", name, timestamp);

        let job = Job {
            metadata: kube::api::ObjectMeta {
                name: Some(job_name.clone()),
                namespace: Some(namespace.to_string()),
                labels: Some(
                    [("triggered-by".to_string(), "kubestudio".to_string())]
                        .into_iter()
                        .collect(),
                ),
                ..Default::default()
            },
            spec: Some(job_template),
            status: None,
        };

        job_api
            .create(&kube::api::PostParams::default(), &job)
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to create job: {}", e),
            })?;

        Ok(job_name)
    }
}
