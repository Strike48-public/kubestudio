// Node list component for displaying cluster nodes

use std::collections::HashMap;

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Node;
use ks_kube::NodeMetrics;
use lucide_dioxus::Server;

#[derive(Props, Clone, PartialEq)]
pub struct NodeListProps {
    /// List of nodes to display
    pub nodes: Vec<Node>,
    /// Currently selected index
    pub selected_index: Option<usize>,
    /// Whether this list is focused
    pub is_focused: bool,
    /// Callback when a node is selected
    pub on_select: EventHandler<Node>,
    /// Node metrics from metrics-server (optional)
    #[props(default)]
    pub metrics: Option<HashMap<String, NodeMetrics>>,
    /// Whether metrics are available
    #[props(default = false)]
    pub metrics_available: bool,
}

#[component]
pub fn NodeList(props: NodeListProps) -> Element {
    let nodes = &props.nodes;
    let is_empty = nodes.is_empty();

    rsx! {
        div { class: "container-drilldown",
            // Header - matches drilldown style
            div { class: "drilldown-header",
                h3 { "Nodes" }
                span { class: "drilldown-count", "{nodes.len()}" }
            }

            // Hint text
            div { class: "drilldown-hint",
                "Use ↑↓ to navigate • Enter to view details • d to describe"
            }

            if is_empty {
                div { class: "empty-state",
                    div { class: "empty-icon", Server { size: 20 } }
                    div { class: "empty-title", "No Nodes Found" }
                    div { class: "empty-hint", "Unable to fetch cluster nodes" }
                }
            } else {
                div { class: "container-table-wrapper",
                    table { class: "container-table",
                        thead {
                            tr {
                                th { class: "col-status", "" }
                                th { class: "col-name", "Name" }
                                th { class: "col-roles", "Roles" }
                                th { class: "col-cpu",
                                    if props.metrics_available { "CPU Usage" } else { "CPU" }
                                }
                                th { class: "col-memory",
                                    if props.metrics_available { "Mem Usage" } else { "Memory" }
                                }
                                th { class: "col-version", "Version" }
                                th { class: "col-status-text", "Status" }
                                th { class: "col-age", "Age" }
                            }
                        }
                        tbody {
                            for (idx, node) in nodes.iter().enumerate() {
                                {
                                    let is_selected = props.selected_index == Some(idx) && props.is_focused;
                                    let node_clone = node.clone();
                                    let on_select = props.on_select;

                                    let name = node.metadata.name.clone().unwrap_or_default();
                                    let (status, status_class) = get_node_status(node);
                                    let roles = get_node_roles(node);

                                    // If metrics available, show usage; otherwise show allocatable capacity
                                    let (cpu, memory) = if let Some(ref metrics_map) = props.metrics
                                        && let Some(node_metrics) = metrics_map.get(&name)
                                    {
                                        (node_metrics.cpu_usage.clone(), node_metrics.memory_usage.clone())
                                    } else {
                                        (get_node_cpu(node), get_node_memory(node))
                                    };

                                    let version = get_node_version(node);
                                    let age = get_node_age(node);

                                    let row_class = if is_selected {
                                        "container-row selected"
                                    } else {
                                        "container-row"
                                    };

                                    rsx! {
                                        tr {
                                            key: "{name}",
                                            class: "{row_class}",
                                            onclick: move |_| on_select.call(node_clone.clone()),
                                            td { class: "col-status",
                                                span { class: "status-dot {status_class}" }
                                            }
                                            td { class: "col-name", "{name}" }
                                            td { class: "col-roles", "{roles}" }
                                            td { class: "col-cpu", "{cpu}" }
                                            td { class: "col-memory", "{memory}" }
                                            td { class: "col-version", "{version}" }
                                            td { class: "col-status-text",
                                                span { class: "status-badge {status_class}", "{status}" }
                                            }
                                            td { class: "col-age", "{age}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Get node ready status and CSS class
fn get_node_status(node: &Node) -> (String, &'static str) {
    if let Some(status) = &node.status
        && let Some(conditions) = &status.conditions
    {
        for condition in conditions {
            if condition.type_ == "Ready" {
                return if condition.status == "True" {
                    ("Ready".to_string(), "status-success")
                } else {
                    ("NotReady".to_string(), "status-error")
                };
            }
        }
    }
    ("Unknown".to_string(), "status-unknown")
}

/// Get node roles from labels
fn get_node_roles(node: &Node) -> String {
    let mut roles = Vec::new();

    if let Some(labels) = &node.metadata.labels {
        for key in labels.keys() {
            if key.starts_with("node-role.kubernetes.io/") {
                let role = key.trim_start_matches("node-role.kubernetes.io/");
                if !role.is_empty() {
                    roles.push(role.to_string());
                }
            }
        }
    }

    if roles.is_empty() {
        "<none>".to_string()
    } else {
        roles.join(", ")
    }
}

/// Get kubelet version from node status
fn get_node_version(node: &Node) -> String {
    node.status
        .as_ref()
        .and_then(|s| s.node_info.as_ref())
        .map(|info| info.kubelet_version.clone())
        .unwrap_or_else(|| "-".to_string())
}

/// Get node age from creation timestamp
fn get_node_age(node: &Node) -> String {
    if let Some(ts) = &node.metadata.creation_timestamp {
        let created = ts.0;
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(created);

        if duration.num_days() > 0 {
            format!("{}d", duration.num_days())
        } else if duration.num_hours() > 0 {
            format!("{}h", duration.num_hours())
        } else if duration.num_minutes() > 0 {
            format!("{}m", duration.num_minutes())
        } else {
            format!("{}s", duration.num_seconds())
        }
    } else {
        "-".to_string()
    }
}

/// Get node CPU capacity/allocatable
fn get_node_cpu(node: &Node) -> String {
    node.status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .and_then(|a| a.get("cpu"))
        .map(|q| format_cpu(&q.0))
        .unwrap_or_else(|| "-".to_string())
}

/// Get node memory capacity/allocatable
fn get_node_memory(node: &Node) -> String {
    node.status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .and_then(|a| a.get("memory"))
        .map(|q| format_memory(&q.0))
        .unwrap_or_else(|| "-".to_string())
}

/// Format CPU quantity (e.g., "4" cores, "500m" = 0.5 cores)
fn format_cpu(cpu: &str) -> String {
    if cpu.ends_with('m') {
        // Millicores - convert to cores
        if let Ok(millis) = cpu.trim_end_matches('m').parse::<u64>() {
            if millis >= 1000 {
                format!("{}", millis / 1000)
            } else {
                format!("{}m", millis)
            }
        } else {
            cpu.to_string()
        }
    } else if cpu.ends_with('n') {
        // Nanocores
        if let Ok(nanos) = cpu.trim_end_matches('n').parse::<u64>() {
            let millis = nanos / 1_000_000;
            if millis >= 1000 {
                format!("{}", millis / 1000)
            } else {
                format!("{}m", millis)
            }
        } else {
            cpu.to_string()
        }
    } else {
        // Already in cores
        cpu.to_string()
    }
}

/// Format memory quantity to human-readable (Ki, Mi, Gi)
fn format_memory(mem: &str) -> String {
    // Parse Kubernetes memory format (Ki, Mi, Gi, Ti, or raw bytes)
    let (value, unit) = if mem.ends_with("Ki") {
        (mem.trim_end_matches("Ki").parse::<u64>().ok(), "Ki")
    } else if mem.ends_with("Mi") {
        (mem.trim_end_matches("Mi").parse::<u64>().ok(), "Mi")
    } else if mem.ends_with("Gi") {
        (mem.trim_end_matches("Gi").parse::<u64>().ok(), "Gi")
    } else if mem.ends_with("Ti") {
        (mem.trim_end_matches("Ti").parse::<u64>().ok(), "Ti")
    } else if mem.ends_with('K') {
        (mem.trim_end_matches('K').parse::<u64>().ok(), "K")
    } else if mem.ends_with('M') {
        (mem.trim_end_matches('M').parse::<u64>().ok(), "M")
    } else if mem.ends_with('G') {
        (mem.trim_end_matches('G').parse::<u64>().ok(), "G")
    } else if mem.ends_with('T') {
        (mem.trim_end_matches('T').parse::<u64>().ok(), "T")
    } else {
        // Raw bytes
        (mem.parse::<u64>().ok(), "B")
    };

    match (value, unit) {
        (Some(v), "Ki") => {
            // Convert KiB to more readable format
            if v >= 1024 * 1024 {
                format!("{:.1}Gi", v as f64 / (1024.0 * 1024.0))
            } else if v >= 1024 {
                format!("{:.0}Mi", v as f64 / 1024.0)
            } else {
                format!("{}Ki", v)
            }
        }
        (Some(v), "Mi") => {
            if v >= 1024 {
                format!("{:.1}Gi", v as f64 / 1024.0)
            } else {
                format!("{}Mi", v)
            }
        }
        (Some(v), "Gi") => format!("{}Gi", v),
        (Some(v), "Ti") => format!("{}Ti", v),
        (Some(v), "B") => {
            // Raw bytes - convert to GiB
            let gib = v as f64 / (1024.0 * 1024.0 * 1024.0);
            if gib >= 1.0 {
                format!("{:.1}Gi", gib)
            } else {
                let mib = v as f64 / (1024.0 * 1024.0);
                format!("{:.0}Mi", mib)
            }
        }
        _ => mem.to_string(),
    }
}
