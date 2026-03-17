// Node list component for displaying cluster nodes

use std::cmp::Ordering;
use std::collections::HashMap;

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Node;
use ks_kube::NodeMetrics;
use lucide_dioxus::{ChevronDown, ChevronUp, Server};

use super::resource_list::SortDirection;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NodeSortColumn {
    Name,
    Roles,
    Cpu,
    Memory,
    Version,
    Status,
    Age,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct NodeSortState {
    pub column: Option<NodeSortColumn>,
    pub direction: Option<SortDirection>,
}

impl NodeSortState {
    fn cycle(self, column: NodeSortColumn) -> Self {
        if self.column == Some(column) {
            match self.direction {
                Some(SortDirection::Ascending) => NodeSortState {
                    column: Some(column),
                    direction: Some(SortDirection::Descending),
                },
                Some(SortDirection::Descending) => NodeSortState::default(),
                _ => NodeSortState {
                    column: Some(column),
                    direction: Some(SortDirection::Ascending),
                },
            }
        } else {
            NodeSortState {
                column: Some(column),
                direction: Some(SortDirection::Ascending),
            }
        }
    }

    fn is_asc(&self, column: NodeSortColumn) -> bool {
        self.column == Some(column) && self.direction == Some(SortDirection::Ascending)
    }

    fn is_desc(&self, column: NodeSortColumn) -> bool {
        self.column == Some(column) && self.direction == Some(SortDirection::Descending)
    }
}

fn parse_cpu_to_millicores(cpu: &str) -> Option<i64> {
    let cpu = cpu.trim();
    if cpu == "-" || cpu.is_empty() {
        return None;
    }
    if cpu.ends_with('m') {
        cpu.trim_end_matches('m').parse::<i64>().ok()
    } else if cpu.ends_with('n') {
        cpu.trim_end_matches('n')
            .parse::<i64>()
            .ok()
            .map(|n| n / 1_000_000)
    } else {
        cpu.parse::<f64>().ok().map(|v| (v * 1000.0) as i64)
    }
}

fn parse_memory_to_ki(mem: &str) -> Option<i64> {
    let mem = mem.trim();
    if mem == "-" || mem.is_empty() {
        return None;
    }
    if mem.ends_with("Ti") {
        mem.trim_end_matches("Ti")
            .parse::<f64>()
            .ok()
            .map(|v| (v * 1024.0 * 1024.0 * 1024.0) as i64)
    } else if mem.ends_with("Gi") {
        mem.trim_end_matches("Gi")
            .parse::<f64>()
            .ok()
            .map(|v| (v * 1024.0 * 1024.0) as i64)
    } else if mem.ends_with("Mi") {
        mem.trim_end_matches("Mi")
            .parse::<f64>()
            .ok()
            .map(|v| (v * 1024.0) as i64)
    } else if mem.ends_with("Ki") {
        mem.trim_end_matches("Ki").parse::<i64>().ok()
    } else {
        mem.parse::<f64>().ok().map(|v| (v / 1024.0) as i64)
    }
}

fn parse_age_to_seconds(age: &str) -> Option<i64> {
    let age = age.trim();
    if age.is_empty() || age == "-" {
        return None;
    }
    let mut total: i64 = 0;
    let mut num_buf = String::new();
    for ch in age.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let n: i64 = num_buf.parse().ok()?;
            num_buf.clear();
            match ch {
                'd' => total += n * 86400,
                'h' => total += n * 3600,
                'm' => total += n * 60,
                's' => total += n,
                _ => return None,
            }
        }
    }
    if total > 0 { Some(total) } else { None }
}

fn node_status_severity(status: &str) -> u8 {
    match status {
        "Ready" => 0,
        "NotReady" => 1,
        _ => 2,
    }
}

/// Pre-computed sort keys for a node row
struct NodeSortKeys {
    name: String,
    roles: String,
    cpu: String,
    memory: String,
    version: String,
    status: String,
    age: String,
}

fn sort_nodes(nodes: &mut [(Node, NodeSortKeys)], state: &NodeSortState) {
    let (Some(column), Some(direction)) = (state.column, state.direction) else {
        return;
    };
    nodes.sort_by(|a, b| {
        let (_, ka) = a;
        let (_, kb) = b;
        let ord = match column {
            NodeSortColumn::Name => ka.name.to_lowercase().cmp(&kb.name.to_lowercase()),
            NodeSortColumn::Roles => ka.roles.to_lowercase().cmp(&kb.roles.to_lowercase()),
            NodeSortColumn::Cpu => {
                match (
                    parse_cpu_to_millicores(&ka.cpu),
                    parse_cpu_to_millicores(&kb.cpu),
                ) {
                    (Some(av), Some(bv)) => av.cmp(&bv),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            NodeSortColumn::Memory => {
                match (
                    parse_memory_to_ki(&ka.memory),
                    parse_memory_to_ki(&kb.memory),
                ) {
                    (Some(av), Some(bv)) => av.cmp(&bv),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            NodeSortColumn::Version => ka.version.cmp(&kb.version),
            NodeSortColumn::Status => {
                node_status_severity(&ka.status).cmp(&node_status_severity(&kb.status))
            }
            NodeSortColumn::Age => {
                match (parse_age_to_seconds(&ka.age), parse_age_to_seconds(&kb.age)) {
                    (Some(a_s), Some(b_s)) => a_s.cmp(&b_s),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
        };
        match direction {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        }
    });
}

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
    /// Sort state (managed by parent for persistence across data refreshes)
    #[props(default = None)]
    pub sort_state: Option<Signal<NodeSortState>>,
}

#[component]
pub fn NodeList(props: NodeListProps) -> Element {
    let local_sort = use_signal(NodeSortState::default);
    let mut sort = props.sort_state.unwrap_or(local_sort);
    let current_sort = *sort.read();

    let node_count = props.nodes.len();
    let is_empty = node_count == 0;

    // Pre-compute sort keys and sort
    let mut keyed_nodes: Vec<(Node, NodeSortKeys)> = props
        .nodes
        .iter()
        .map(|node| {
            let name = node.metadata.name.clone().unwrap_or_default();
            let (status, _) = get_node_status(node);
            let roles = get_node_roles(node);
            let (cpu, memory) = if let Some(ref metrics_map) = props.metrics
                && let Some(node_metrics) = metrics_map.get(&name)
            {
                (
                    node_metrics.cpu_usage.clone(),
                    node_metrics.memory_usage.clone(),
                )
            } else {
                (get_node_cpu(node), get_node_memory(node))
            };
            let version = get_node_version(node);
            let age = get_node_age(node);
            let keys = NodeSortKeys {
                name,
                roles,
                cpu,
                memory,
                version,
                status,
                age,
            };
            (node.clone(), keys)
        })
        .collect();
    sort_nodes(&mut keyed_nodes, &current_sort);

    let cpu_label = if props.metrics_available {
        "CPU Usage"
    } else {
        "CPU"
    };
    let mem_label = if props.metrics_available {
        "Mem Usage"
    } else {
        "Memory"
    };

    rsx! {
        div { class: "container-drilldown",
            // Header - matches drilldown style
            div { class: "drilldown-header",
                h3 { "Nodes" }
                span { class: "drilldown-count", "{node_count}" }
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
                                th {
                                    class: "col-name sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Name)); },
                                    span { class: "sortable-header-content",
                                        "Name"
                                        if current_sort.is_asc(NodeSortColumn::Name) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Name) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-roles sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Roles)); },
                                    span { class: "sortable-header-content",
                                        "Roles"
                                        if current_sort.is_asc(NodeSortColumn::Roles) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Roles) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-cpu sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Cpu)); },
                                    span { class: "sortable-header-content",
                                        "{cpu_label}"
                                        if current_sort.is_asc(NodeSortColumn::Cpu) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Cpu) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-memory sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Memory)); },
                                    span { class: "sortable-header-content",
                                        "{mem_label}"
                                        if current_sort.is_asc(NodeSortColumn::Memory) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Memory) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-version sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Version)); },
                                    span { class: "sortable-header-content",
                                        "Version"
                                        if current_sort.is_asc(NodeSortColumn::Version) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Version) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-status-text sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Status)); },
                                    span { class: "sortable-header-content",
                                        "Status"
                                        if current_sort.is_asc(NodeSortColumn::Status) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Status) { ChevronDown { size: 14 } }
                                    }
                                }
                                th {
                                    class: "col-age sortable-header",
                                    onclick: move |_| { sort.set(current_sort.cycle(NodeSortColumn::Age)); },
                                    span { class: "sortable-header-content",
                                        "Age"
                                        if current_sort.is_asc(NodeSortColumn::Age) { ChevronUp { size: 14 } }
                                        if current_sort.is_desc(NodeSortColumn::Age) { ChevronDown { size: 14 } }
                                    }
                                }
                            }
                        }
                        tbody {
                            for (idx, (node, keys)) in keyed_nodes.iter().enumerate() {
                                {
                                    let is_selected = props.selected_index == Some(idx) && props.is_focused;
                                    let node_clone = node.clone();
                                    let on_select = props.on_select;
                                    let (_, status_class) = get_node_status(node);

                                    let row_class = if is_selected {
                                        "container-row selected"
                                    } else {
                                        "container-row"
                                    };

                                    rsx! {
                                        tr {
                                            key: "{keys.name}",
                                            class: "{row_class}",
                                            onclick: move |_| on_select.call(node_clone.clone()),
                                            td { class: "col-status",
                                                span { class: "status-dot {status_class}" }
                                            }
                                            td { class: "col-name", "{keys.name}" }
                                            td { class: "col-roles", "{keys.roles}" }
                                            td { class: "col-cpu", "{keys.cpu}" }
                                            td { class: "col-memory", "{keys.memory}" }
                                            td { class: "col-version", "{keys.version}" }
                                            td { class: "col-status-text",
                                                span { class: "status-badge {status_class}", "{keys.status}" }
                                            }
                                            td { class: "col-age", "{keys.age}" }
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
