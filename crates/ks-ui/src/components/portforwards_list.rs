// Port-forwards list view component

use crate::hooks::use_portforward::ActivePortForward;
use dioxus::prelude::*;
use lucide_dioxus::{ArrowLeftRight, ArrowRight};

#[derive(Props, Clone, PartialEq)]
pub struct PortForwardsListProps {
    /// List of active port-forwards
    pub forwards: Vec<ActivePortForward>,
    /// Currently selected index
    pub selected_index: Option<usize>,
    /// Whether this list is focused
    pub is_focused: bool,
    /// Callback when a forward is selected
    pub on_select: EventHandler<ActivePortForward>,
    /// Callback when user wants to stop a forward
    pub on_stop: EventHandler<String>, // forward ID
}

#[component]
pub fn PortForwardsList(props: PortForwardsListProps) -> Element {
    let forwards = &props.forwards;
    let is_empty = forwards.is_empty();

    rsx! {
        div { class: "container-drilldown",
            // Header - matches drilldown style
            div { class: "drilldown-header",
                h3 { "Active Port Forwards" }
                span { class: "drilldown-count", "{forwards.len()}" }
            }

            // Hint text
            div { class: "drilldown-hint",
                "Use ↑↓ to navigate • Ctrl+D to stop • Esc to go back"
            }

            if is_empty {
                div { class: "empty-state",
                    div { class: "empty-icon", ArrowLeftRight { size: 20 } }
                    div { class: "empty-title", "No Active Port Forwards" }
                    div { class: "empty-hint", "Press 'f' on a pod to create a port-forward" }
                }
            } else {
                // Table - matches drilldown table style
                div { class: "container-table-wrapper",
                    table { class: "container-table",
                        thead {
                            tr {
                                th { "Local" }
                                th { "" }
                                th { "Remote" }
                                th { "Pod" }
                                th { "Namespace" }
                                th { "" }
                            }
                        }
                        tbody {
                            for (idx, forward) in forwards.iter().enumerate() {
                                {
                                    let is_selected = props.selected_index == Some(idx) && props.is_focused;
                                    let forward_clone = forward.clone();
                                    let forward_id = forward.id.clone();
                                    let on_stop = props.on_stop;
                                    let row_class = if is_selected {
                                        "container-row selected"
                                    } else {
                                        "container-row"
                                    };

                                    rsx! {
                                        tr {
                                            key: "{forward.id}",
                                            class: "{row_class}",
                                            onclick: move |_| props.on_select.call(forward_clone.clone()),
                                            td { class: "col-port",
                                                span { class: "port-badge local", ":{forward.local_port}" }
                                            }
                                            td { class: "col-arrow", ArrowRight { size: 14 } }
                                            td { class: "col-port",
                                                span { class: "port-badge remote", ":{forward.remote_port}" }
                                            }
                                            td { class: "col-name", "{forward.pod_name}" }
                                            td { class: "col-namespace", "{forward.namespace}" }
                                            td { class: "col-actions",
                                                button {
                                                    class: "btn-icon btn-danger",
                                                    onclick: move |e| {
                                                        e.stop_propagation();
                                                        on_stop.call(forward_id.clone());
                                                    },
                                                    title: "Stop forward (x)",
                                                    "×"
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
