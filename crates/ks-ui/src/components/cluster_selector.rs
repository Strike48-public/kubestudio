use dioxus::prelude::*;
use lucide_dioxus::{Check, ChevronDown, ChevronUp, Crosshair};

#[derive(Clone, PartialEq)]
pub struct Context {
    pub name: String,
    pub is_current: bool,
}

#[derive(Props, Clone, PartialEq)]
pub struct ClusterSelectorProps {
    pub contexts: Vec<Context>,
    pub current_context: Option<String>,
    pub on_context_select: EventHandler<String>,
    pub is_connected: Option<bool>,
}

#[component]
pub fn ClusterSelector(props: ClusterSelectorProps) -> Element {
    let mut expanded = use_signal(|| false);

    rsx! {
        div { class: "cluster-selector",
            div {
                class: "cluster-selector-header",
                onclick: move |_| expanded.set(!expanded()),
                div { class: "cluster-info",
                    div { class: "cluster-icon", Crosshair { size: 16 } }
                    div { class: "cluster-details",
                        div { class: "cluster-label", "Cluster" }
                        div { class: "cluster-name",
                            if let Some(ctx) = &props.current_context {
                                "{ctx}"
                            } else {
                                span { class: "no-cluster", "No cluster selected" }
                            }
                        }
                    }
                }
                div { class: "status-indicator",
                    match props.is_connected {
                        Some(true) => rsx! { span { class: "status-dot connected", title: "Connected" } },
                        Some(false) => rsx! { span { class: "status-dot disconnected", title: "Disconnected" } },
                        None => rsx! { span { class: "status-dot pending", title: "Connecting..." } },
                    }
                }
                div { class: "expand-icon",
                    if expanded() {
                        ChevronUp { size: 14 }
                    } else {
                        ChevronDown { size: 14 }
                    }
                }
            }

            if expanded() {
                div { class: "cluster-dropdown",
                    if props.contexts.is_empty() {
                        div { class: "empty-contexts",
                            "No contexts available"
                        }
                    } else {
                        for context in props.contexts.iter() {
                            div {
                                class: if props.current_context.as_ref() == Some(&context.name) {
                                    "context-item selected"
                                } else {
                                    "context-item"
                                },
                                onclick: {
                                    let name = context.name.clone();
                                    move |_| {
                                        props.on_context_select.call(name.clone());
                                        expanded.set(false);
                                    }
                                },
                                div { class: "context-name", "{context.name}" }
                                if props.current_context.as_ref() == Some(&context.name) {
                                    div { class: "current-badge", Check { size: 14 } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
