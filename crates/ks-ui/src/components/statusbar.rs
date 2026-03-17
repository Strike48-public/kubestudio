use crate::hooks::WatchConnectionState;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct StatusBarProps {
    pub cluster_name: Option<String>,
    pub namespace: Option<String>,
    pub resource_count: usize,
    #[props(default)]
    pub watch_state: WatchConnectionState,
    #[props(default)]
    pub port_forward_count: usize,
    /// Callback when port-forwards indicator is clicked
    #[props(default)]
    pub on_portforwards_click: Option<EventHandler<()>>,
    /// Read-only mode indicator
    #[props(default = false)]
    pub read_only: bool,
    /// Remappable keybindings
    #[props(default)]
    pub keybindings: ks_plugin::KeyBindings,
}

#[component]
pub fn StatusBar(props: StatusBarProps) -> Element {
    let (state_class, state_text) = match props.watch_state {
        WatchConnectionState::Live => ("status-live", "Live"),
        WatchConnectionState::Syncing => ("status-syncing", "Syncing"),
        WatchConnectionState::Reconnecting => ("status-reconnecting", "Reconnecting"),
        WatchConnectionState::Disconnected => ("status-disconnected", "Disconnected"),
    };

    rsx! {
        div { class: "status-bar",
            // Watch connection status indicator
            div { class: "status-item status-watch {state_class}",
                span { class: "status-indicator" }
                span { class: "status-text", "{state_text}" }
            }
            // Read-only mode indicator
            if props.read_only {
                div { class: "status-item status-readonly",
                    title: "Read-only mode: write operations disabled (KUBESTUDIO_MODE=read)",
                    span { class: "status-badge status-warning", "READ-ONLY" }
                }
            }
            div { class: "status-item",
                span { class: "status-label", "Cluster: " }
                span { class: "status-value",
                    if let Some(name) = &props.cluster_name {
                        "{name}"
                    } else {
                        "Not connected"
                    }
                }
            }
            if let Some(ns) = &props.namespace {
                div { class: "status-item",
                    span { class: "status-label", "Namespace: " }
                    span { class: "status-value", "{ns}" }
                }
            }
            div { class: "status-item",
                span { class: "status-label", "Resources: " }
                span { class: "status-value", "{props.resource_count}" }
            }
            // Port-forwards indicator - clickable when there are active forwards
            if props.port_forward_count > 0 {
                div {
                    class: "status-item status-portforward clickable",
                    onclick: move |_| {
                        if let Some(handler) = &props.on_portforwards_click {
                            handler.call(());
                        }
                    },
                    title: format!("Click or press {} to view port-forwards", props.keybindings.display("port_forwards")),
                    span { class: "status-indicator pf-active" }
                    span { class: "status-label", "Forwards: " }
                    span { class: "status-value", "{props.port_forward_count}" }
                }
            }
            div { class: "status-item status-help",
                kbd { {props.keybindings.display("overview").to_string()} } span { " overview  " }
                kbd { {props.keybindings.display("pods").to_string()} } span { " pods  " }
                kbd { {props.keybindings.display("deployments").to_string()} } span { " deploy  " }
                kbd { {props.keybindings.display("services").to_string()} } span { " svc  " }
                kbd { {props.keybindings.display("port_forwards").to_string()} } span { " fwd  " }
                kbd { {props.keybindings.display("help").to_string()} } span { " help" }
            }
        }
    }
}
