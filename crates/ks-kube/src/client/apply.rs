//! YAML apply operations for creating/updating resources

use ks_core::{SkdError, SkdResult};
use kube::ResourceExt;
use kube::api::{Api, Patch, PatchParams};
use kube::core::DynamicObject;
use kube::discovery::ApiResource;

use super::KubeClient;
use super::types::{ApplyResult, MultiApplyResult, pluralize_kind};

impl KubeClient {
    /// Apply YAML manifest to the cluster using server-side apply
    /// Returns the name of the applied resource on success
    pub async fn apply_yaml(&self, yaml: &str) -> SkdResult<ApplyResult> {
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(yaml).map_err(|e| SkdError::KubeApi {
                status_code: 400,
                message: format!("Invalid YAML: {}", e),
            })?;

        let api_version = parsed
            .get("apiVersion")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkdError::KubeApi {
                status_code: 400,
                message: "Missing apiVersion in YAML".to_string(),
            })?;

        let kind =
            parsed
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SkdError::KubeApi {
                    status_code: 400,
                    message: "Missing kind in YAML".to_string(),
                })?;

        let name = parsed
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkdError::KubeApi {
                status_code: 400,
                message: "Missing metadata.name in YAML".to_string(),
            })?;

        let namespace = parsed
            .get("metadata")
            .and_then(|m| m.get("namespace"))
            .and_then(|v| v.as_str());

        let (group, version) = if api_version.contains('/') {
            let parts: Vec<&str> = api_version.splitn(2, '/').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), api_version.to_string())
        };

        let api_resource = ApiResource {
            group: group.clone(),
            version: version.clone(),
            api_version: api_version.to_string(),
            kind: kind.to_string(),
            plural: pluralize_kind(kind),
        };

        let obj: DynamicObject = serde_yaml::from_str(yaml).map_err(|e| SkdError::KubeApi {
            status_code: 400,
            message: format!("Failed to parse YAML as Kubernetes object: {}", e),
        })?;

        let api: Api<DynamicObject> = if let Some(ns) = namespace {
            Api::namespaced_with(self.inner.clone(), ns, &api_resource)
        } else {
            Api::all_with(self.inner.clone(), &api_resource)
        };

        let patch_params = PatchParams::apply("kubestudio").force();
        let result = api
            .patch(name, &patch_params, &Patch::Apply(&obj))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Failed to apply resource: {}", e),
            })?;

        Ok(ApplyResult {
            name: result.name_any(),
            kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        })
    }

    /// Apply multiple YAML documents (separated by ---)
    /// Returns results for each document, continuing even if some fail
    pub async fn apply_multi_yaml(&self, yaml: &str) -> SkdResult<MultiApplyResult> {
        let mut results = Vec::new();
        let mut errors = Vec::new();

        let documents: Vec<&str> = yaml
            .split("\n---")
            .map(|s| s.trim_start_matches("---").trim())
            .filter(|s| !s.is_empty())
            .collect();

        for (idx, doc) in documents.iter().enumerate() {
            match self.apply_yaml(doc).await {
                Ok(result) => results.push(result),
                Err(e) => errors.push(format!("Document {}: {}", idx + 1, e)),
            }
        }

        Ok(MultiApplyResult { results, errors })
    }

    /// Dry-run apply to validate YAML without making changes
    pub async fn dry_run_apply(&self, yaml: &str) -> SkdResult<ApplyResult> {
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(yaml).map_err(|e| SkdError::KubeApi {
                status_code: 400,
                message: format!("Invalid YAML: {}", e),
            })?;

        let api_version = parsed
            .get("apiVersion")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkdError::KubeApi {
                status_code: 400,
                message: "Missing apiVersion in YAML".to_string(),
            })?;

        let kind =
            parsed
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SkdError::KubeApi {
                    status_code: 400,
                    message: "Missing kind in YAML".to_string(),
                })?;

        let name = parsed
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkdError::KubeApi {
                status_code: 400,
                message: "Missing metadata.name in YAML".to_string(),
            })?;

        let namespace = parsed
            .get("metadata")
            .and_then(|m| m.get("namespace"))
            .and_then(|v| v.as_str());

        let (group, version) = if api_version.contains('/') {
            let parts: Vec<&str> = api_version.splitn(2, '/').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (String::new(), api_version.to_string())
        };

        let api_resource = ApiResource {
            group: group.clone(),
            version: version.clone(),
            api_version: api_version.to_string(),
            kind: kind.to_string(),
            plural: pluralize_kind(kind),
        };

        let obj: DynamicObject = serde_yaml::from_str(yaml).map_err(|e| SkdError::KubeApi {
            status_code: 400,
            message: format!("Failed to parse YAML as Kubernetes object: {}", e),
        })?;

        let api: Api<DynamicObject> = if let Some(ns) = namespace {
            Api::namespaced_with(self.inner.clone(), ns, &api_resource)
        } else {
            Api::all_with(self.inner.clone(), &api_resource)
        };

        let patch_params = PatchParams::apply("kubestudio").force().dry_run();
        let result = api
            .patch(name, &patch_params, &Patch::Apply(&obj))
            .await
            .map_err(|e| SkdError::KubeApi {
                status_code: 500,
                message: format!("Dry-run validation failed: {}", e),
            })?;

        Ok(ApplyResult {
            name: result.name_any(),
            kind: kind.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        })
    }
}
