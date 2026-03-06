// Enhanced cluster overview dashboard component

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Event, Node, Pod};
use lucide_dioxus::{Check, Globe, Package, Rocket, Server, TriangleAlert};

#[derive(Props, Clone, PartialEq)]
pub struct ClusterOverviewProps {
    /// Connected cluster name
    pub cluster_name: Option<String>,
    /// All nodes
    pub nodes: Vec<Node>,
    /// All pods (from current namespace filter)
    pub pods: Vec<Pod>,
    /// Deployment count
    pub deployment_count: usize,
    /// StatefulSet count
    pub statefulset_count: usize,
    /// DaemonSet count
    pub daemonset_count: usize,
    /// Service count
    pub service_count: usize,
    /// Recent events (warnings)
    pub events: Vec<Event>,
    /// Callback for quick navigation
    pub on_navigate: EventHandler<String>,
    /// Callback for "Ask Agent" on a warning event
    #[props(default)]
    pub on_ask_agent: EventHandler<String>,
    /// Whether Matrix API is configured (show AI buttons)
    #[props(default = false)]
    pub has_matrix: bool,
    /// Whether the user is authenticated (has a valid auth token for AI features)
    #[props(default = false)]
    pub ai_authenticated: bool,
    /// Currently selected card index (for keyboard navigation)
    #[props(default = None)]
    pub selected_index: Option<usize>,
    /// Whether the overview is focused
    #[props(default = false)]
    pub is_focused: bool,
    /// Whether keyboard navigation is active (hides mouse hover)
    #[props(default = false)]
    pub keyboard_nav_active: bool,
}

/// Card navigation targets (in order)
pub const CARD_TARGETS: [&str; 4] = ["nodes", "pods", "deployments", "services"];

/// Get the number of navigable cards
pub fn get_overview_card_count() -> usize {
    CARD_TARGETS.len()
}

/// Get the navigation target for a card index
pub fn get_overview_card_target(index: usize) -> Option<&'static str> {
    CARD_TARGETS.get(index).copied()
}

#[component]
pub fn ClusterOverview(props: ClusterOverviewProps) -> Element {
    // Calculate node stats
    let (ready_nodes, not_ready_nodes) = count_node_status(&props.nodes);
    let total_nodes = props.nodes.len();

    // Calculate pod stats
    let (running_pods, pending_pods, failed_pods) = count_pod_status(&props.pods);
    let total_pods = props.pods.len();

    // Get recent warning events (last 10)
    let warning_events: Vec<_> = props
        .events
        .iter()
        .filter(|e| e.type_.as_deref() == Some("Warning"))
        .take(10)
        .collect();

    // Determine container class based on keyboard navigation state
    let container_class = if props.keyboard_nav_active {
        "cluster-overview keyboard-nav"
    } else {
        "cluster-overview"
    };

    let cluster_name_display = props.cluster_name.clone().unwrap_or_default();

    rsx! {
        div { class: "{container_class}",
            // Header
            div { class: "overview-header",
                h1 { "Cluster Overview" }
            }

            // Stats Grid
            div { class: "overview-stats-grid",
                // Nodes Card (index 0)
                div {
                    class: if props.is_focused && props.selected_index == Some(0) {
                        "overview-card nodes-card selected"
                    } else {
                        "overview-card nodes-card"
                    },
                    onclick: {
                        let on_navigate = props.on_navigate;
                        move |_| on_navigate.call("nodes".to_string())
                    },
                    div { class: "card-header",
                        span { class: "card-icon", Server { size: 16 } }
                        h3 { "Nodes" }
                    }
                    div { class: "card-stats",
                        div { class: "stat-row",
                            span { class: "stat-label", "Ready" }
                            span { class: "stat-value success", "{ready_nodes}" }
                        }
                        if not_ready_nodes > 0 {
                            div { class: "stat-row",
                                span { class: "stat-label", "Not Ready" }
                                span { class: "stat-value error", "{not_ready_nodes}" }
                            }
                        }
                        div { class: "stat-row total",
                            span { class: "stat-label", "Total" }
                            span { class: "stat-value", "{total_nodes}" }
                        }
                    }
                }

                // Pods Card (index 1)
                div {
                    class: if props.is_focused && props.selected_index == Some(1) {
                        "overview-card pods-card selected"
                    } else {
                        "overview-card pods-card"
                    },
                    onclick: {
                        let on_navigate = props.on_navigate;
                        move |_| on_navigate.call("pods".to_string())
                    },
                    div { class: "card-header",
                        span { class: "card-icon", Package { size: 16 } }
                        h3 { "Pods" }
                    }
                    div { class: "card-stats",
                        div { class: "stat-row",
                            span { class: "stat-label", "Running" }
                            span { class: "stat-value success", "{running_pods}" }
                        }
                        if pending_pods > 0 {
                            div { class: "stat-row",
                                span { class: "stat-label", "Pending" }
                                span { class: "stat-value warning", "{pending_pods}" }
                            }
                        }
                        if failed_pods > 0 {
                            div { class: "stat-row",
                                span { class: "stat-label", "Failed" }
                                span { class: "stat-value error", "{failed_pods}" }
                            }
                        }
                        div { class: "stat-row total",
                            span { class: "stat-label", "Total" }
                            span { class: "stat-value", "{total_pods}" }
                        }
                    }
                }

                // Workloads Card (index 2)
                div {
                    class: if props.is_focused && props.selected_index == Some(2) {
                        "overview-card workloads-card selected"
                    } else {
                        "overview-card workloads-card"
                    },
                    onclick: {
                        let on_navigate = props.on_navigate;
                        move |_| on_navigate.call("deployments".to_string())
                    },
                    div { class: "card-header",
                        span { class: "card-icon", Rocket { size: 16 } }
                        h3 { "Workloads" }
                    }
                    div { class: "card-stats",
                        div { class: "stat-row",
                            span { class: "stat-label", "Deployments" }
                            span { class: "stat-value", "{props.deployment_count}" }
                        }
                        div { class: "stat-row",
                            span { class: "stat-label", "StatefulSets" }
                            span { class: "stat-value", "{props.statefulset_count}" }
                        }
                        div { class: "stat-row",
                            span { class: "stat-label", "DaemonSets" }
                            span { class: "stat-value", "{props.daemonset_count}" }
                        }
                    }
                }

                // Services Card (index 3)
                div {
                    class: if props.is_focused && props.selected_index == Some(3) {
                        "overview-card services-card selected"
                    } else {
                        "overview-card services-card"
                    },
                    onclick: {
                        let on_navigate = props.on_navigate;
                        move |_| on_navigate.call("services".to_string())
                    },
                    div { class: "card-header",
                        span { class: "card-icon", Globe { size: 16 } }
                        h3 { "Network" }
                    }
                    div { class: "card-stats",
                        div { class: "stat-row",
                            span { class: "stat-label", "Services" }
                            span { class: "stat-value", "{props.service_count}" }
                        }
                    }
                }
            }

            // Recent Warnings
            div { class: "overview-section",
                div { class: "section-header",
                    h3 { TriangleAlert { size: 16 } " Recent Warnings" }
                    if !warning_events.is_empty() {
                        span { class: "warning-count", "{warning_events.len()}" }
                    }
                }
                if warning_events.is_empty() {
                    div { class: "no-warnings",
                        span { class: "success-icon", Check { size: 16 } }
                        "No recent warnings"
                    }
                } else {
                    div { class: "warning-list",
                        for event in warning_events.iter() {
                            {
                                let involved = &event.involved_object;
                                let kind = involved.kind.as_deref().unwrap_or("");
                                let name = involved.name.as_deref().unwrap_or("");
                                let reason = event.reason.as_deref().unwrap_or("Unknown");
                                let message = event.message.as_deref().unwrap_or("");
                                let age = get_event_age(event);
                                let cluster = cluster_name_display.clone();

                                let ask_msg = format!(
                                    "Why is {}/{} in cluster {} showing warning: {} - {}",
                                    kind, name, cluster, reason, message,
                                );

                                rsx! {
                                    div { class: "warning-item",
                                        div { class: "warning-item-content",
                                            div { class: "warning-source",
                                                span { class: "warning-kind", "{kind}/" }
                                                span { class: "warning-name", "{name}" }
                                            }
                                            div { class: "warning-reason", "{reason}" }
                                            div { class: "warning-message", "{message}" }
                                            div { class: "warning-age", "{age}" }
                                        }
                                        if props.has_matrix {
                                            {
                                                let disabled = !props.ai_authenticated;
                                                let btn_class = if disabled { "warning-ask-agent-btn disabled" } else { "warning-ask-agent-btn" };
                                                let title = if disabled { "Sign in to ask AI agent" } else { "Ask AI agent about this warning" };
                                                rsx! {
                                                    button {
                                                        class: "{btn_class}",
                                                        disabled: disabled,
                                                        title: "{title}",
                                                        onclick: {
                                                            let on_ask = props.on_ask_agent;
                                                            let msg = ask_msg.clone();
                                                            move |_| {
                                                                if !disabled {
                                                                    on_ask.call(msg.clone());
                                                                }
                                                            }
                                                        },
                                                        "Ask Agent"
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
        }
    }
}

/// Count ready and not-ready nodes
fn count_node_status(nodes: &[Node]) -> (usize, usize) {
    let mut ready = 0;
    let mut not_ready = 0;

    for node in nodes {
        let is_ready = node
            .status
            .as_ref()
            .and_then(|s| s.conditions.as_ref())
            .map(|conditions| {
                conditions
                    .iter()
                    .any(|c| c.type_ == "Ready" && c.status == "True")
            })
            .unwrap_or(false);

        if is_ready {
            ready += 1;
        } else {
            not_ready += 1;
        }
    }

    (ready, not_ready)
}

/// Count pod statuses
fn count_pod_status(pods: &[Pod]) -> (usize, usize, usize) {
    let mut running = 0;
    let mut pending = 0;
    let mut failed = 0;

    for pod in pods {
        let phase = pod
            .status
            .as_ref()
            .and_then(|s| s.phase.as_deref())
            .unwrap_or("Unknown");

        match phase {
            "Running" => running += 1,
            "Pending" => pending += 1,
            "Failed" => failed += 1,
            _ => {}
        }
    }

    (running, pending, failed)
}

/// Get event age as a human-readable string
fn get_event_age(event: &Event) -> String {
    // Try last_timestamp first, then event_time
    let datetime = event
        .last_timestamp
        .as_ref()
        .map(|t| t.0)
        .or_else(|| event.event_time.as_ref().map(|t| t.0));

    if let Some(ts) = datetime {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(ts);

        if duration.num_days() > 0 {
            format!("{}d", duration.num_days())
        } else if duration.num_hours() > 0 {
            format!("{}h", duration.num_hours())
        } else if duration.num_minutes() > 0 {
            format!("{}m", duration.num_minutes())
        } else {
            format!("{}s", duration.num_seconds().max(0))
        }
    } else {
        "-".to_string()
    }
}
