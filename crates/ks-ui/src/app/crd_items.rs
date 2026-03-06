//! Convert DynamicObjects to ResourceItems for display

use super::helpers::format_age;
use crate::components::ResourceItem;
use ks_kube::{CrdInfo, PrinterColumn};
use kube::ResourceExt;
use kube::api::DynamicObject;
use serde_json::Value;

/// Convert a list of DynamicObjects to ResourceItems for display
/// Uses printer columns from the CRD spec when available
pub fn dynamic_objects_to_items(objects: &[DynamicObject], crd: &CrdInfo) -> Vec<ResourceItem> {
    objects
        .iter()
        .map(|obj| {
            let metadata = &obj.metadata;
            let name = obj.name_any();
            let namespace = metadata.namespace.clone();
            let age = format_age(metadata.creation_timestamp.as_ref());

            // Try to extract status from printer columns or fall back to generic extraction
            let status = extract_status(obj, &crd.printer_columns);
            let ready = extract_ready(obj, &crd.printer_columns);

            ResourceItem {
                name,
                namespace,
                status,
                age,
                ready,
                restarts: None,
            }
        })
        .collect()
}

/// Extract status from DynamicObject using printer columns or common patterns
fn extract_status(obj: &DynamicObject, printer_columns: &[PrinterColumn]) -> String {
    // First try to find a Status column in printer columns
    if let Some(status_col) = printer_columns
        .iter()
        .find(|c| c.name.to_lowercase() == "status" || c.name.to_lowercase() == "phase")
        && let Some(value) = extract_json_path(&obj.data, &status_col.json_path)
    {
        return value;
    }

    // Try common status paths
    let common_paths = [
        ".status.phase",
        ".status.state",
        ".status.conditions[0].type",
        ".status.health.status",
    ];

    for path in common_paths {
        if let Some(value) = extract_json_path(&obj.data, path) {
            return value;
        }
    }

    // Check if there are any conditions and return the first one's type
    if let Some(conditions) = obj.data.get("status").and_then(|s| s.get("conditions"))
        && let Some(conditions_array) = conditions.as_array()
        && let Some(first_cond) = conditions_array.first()
        && let Some(cond_type) = first_cond.get("type").and_then(|t| t.as_str())
    {
        let status = first_cond
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("Unknown");
        if status == "True" {
            return cond_type.to_string();
        }
    }

    "Unknown".to_string()
}

/// Extract ready status from DynamicObject using printer columns or common patterns
fn extract_ready(obj: &DynamicObject, printer_columns: &[PrinterColumn]) -> Option<String> {
    // First try to find a Ready column in printer columns
    if let Some(ready_col) = printer_columns
        .iter()
        .find(|c| c.name.to_lowercase() == "ready" || c.name.to_lowercase() == "replicas")
        && let Some(value) = extract_json_path(&obj.data, &ready_col.json_path)
    {
        return Some(value);
    }

    // Try common ready patterns
    let ready_paths = [
        (".status.readyReplicas", ".status.replicas"),
        (".status.ready", ".status.total"),
    ];

    for (ready_path, total_path) in ready_paths {
        if let (Some(ready), Some(total)) = (
            extract_json_path(&obj.data, ready_path),
            extract_json_path(&obj.data, total_path),
        ) {
            return Some(format!("{}/{}", ready, total));
        }
    }

    None
}

/// Extract a value from a JSON object using a simplified JSONPath
/// Supports paths like ".status.phase", ".spec.replicas", ".status.conditions[0].type"
fn extract_json_path(data: &Value, path: &str) -> Option<String> {
    let path = path.trim_start_matches('.');
    let mut current = data;

    for part in path.split('.') {
        if part.is_empty() {
            continue;
        }

        // Handle array indexing like "conditions[0]"
        if let Some(bracket_pos) = part.find('[') {
            let field_name = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];

            // Navigate to the field
            current = current.get(field_name)?;

            // Parse and apply the index
            if let Ok(index) = index_str.parse::<usize>() {
                current = current.get(index)?;
            } else {
                return None;
            }
        } else {
            current = current.get(part)?;
        }
    }

    // Convert the final value to a string
    match current {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        _ => Some(current.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_json_path_simple() {
        let data = json!({
            "status": {
                "phase": "Running"
            }
        });
        assert_eq!(
            extract_json_path(&data, ".status.phase"),
            Some("Running".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_array() {
        let data = json!({
            "status": {
                "conditions": [
                    {"type": "Ready", "status": "True"},
                    {"type": "Available", "status": "False"}
                ]
            }
        });
        assert_eq!(
            extract_json_path(&data, ".status.conditions[0].type"),
            Some("Ready".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_number() {
        let data = json!({
            "status": {
                "replicas": 3
            }
        });
        assert_eq!(
            extract_json_path(&data, ".status.replicas"),
            Some("3".to_string())
        );
    }
}
