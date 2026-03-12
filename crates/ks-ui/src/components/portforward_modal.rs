// Port-forward modal for selecting ports

use dioxus::prelude::*;

/// Focus zones in the port-forward modal
#[derive(Clone, Copy, PartialEq, Default)]
enum ModalFocus {
    #[default]
    LocalPort, // Local port input field
    PortList, // Container port selection list
}

#[derive(Props, Clone, PartialEq)]
pub struct PortForwardModalProps {
    /// Pod name
    pub pod_name: String,
    /// Namespace
    pub namespace: String,
    /// Available container ports (port, name, protocol)
    pub container_ports: Vec<(u16, Option<String>, String)>,
    /// Callback when user confirms port-forward
    pub on_confirm: EventHandler<(u16, u16)>, // (local_port, remote_port)
    /// Callback when user cancels
    pub on_cancel: EventHandler<()>,
}

#[component]
pub fn PortForwardModal(props: PortForwardModalProps) -> Element {
    let mut selected_port_index = use_signal(|| 0usize);
    let mut local_port_input = use_signal(|| {
        props
            .container_ports
            .first()
            .map(|(p, _, _)| p.to_string())
            .unwrap_or_else(|| "8080".to_string())
    });
    let mut error_message = use_signal(|| None::<String>);
    let mut focus_zone = use_signal(ModalFocus::default);

    let on_confirm = props.on_confirm;
    let on_cancel = props.on_cancel;

    let container_ports = props.container_ports.clone();
    let port_count = container_ports.len();

    rsx! {
        div {
            class: "modal-overlay",
            tabindex: 0,
            onclick: move |_| on_cancel.call(()),
            onkeydown: {
                let container_ports = props.container_ports.clone();
                move |e: KeyboardEvent| {
                    if crate::utils::is_escape(&e) {
                        on_cancel.call(());
                        e.stop_propagation();
                        e.prevent_default();
                    } else {
                        match e.key() {
                            Key::Enter => {
                                let local_port_str = local_port_input.read().clone();
                                match local_port_str.parse::<u16>() {
                                    Ok(local_port) => {
                                        if local_port == 0 {
                                            error_message.set(Some("Port must be greater than 0".to_string()));
                                        } else {
                                            let remote_port = if container_ports.is_empty() {
                                                local_port
                                            } else {
                                                container_ports.get(*selected_port_index.read()).map(|(p, _, _)| *p).unwrap_or(local_port)
                                            };
                                            on_confirm.call((local_port, remote_port));
                                        }
                                    }
                                    Err(_) => {
                                        error_message.set(Some("Invalid port number".to_string()));
                                    }
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::ArrowUp => {
                                let current_focus = *focus_zone.read();
                                if current_focus == ModalFocus::PortList && port_count > 0 {
                                    let current = *selected_port_index.read();
                                    if current > 0 {
                                        selected_port_index.set(current - 1);
                                        if let Some((p, _, _)) = container_ports.get(current - 1) {
                                            local_port_input.set(p.to_string());
                                        }
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                } else if current_focus == ModalFocus::LocalPort && port_count > 0 {
                                    focus_zone.set(ModalFocus::PortList);
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                            }
                            Key::ArrowDown => {
                                let current_focus = *focus_zone.read();
                                if current_focus == ModalFocus::PortList && port_count > 0 {
                                    let current = *selected_port_index.read();
                                    if current < port_count - 1 {
                                        selected_port_index.set(current + 1);
                                        if let Some((p, _, _)) = container_ports.get(current + 1) {
                                            local_port_input.set(p.to_string());
                                        }
                                    } else {
                                        focus_zone.set(ModalFocus::LocalPort);
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                            }
                            Key::Tab => {
                                let current = *focus_zone.read();
                                if current == ModalFocus::LocalPort && port_count > 0 {
                                    focus_zone.set(ModalFocus::PortList);
                                } else {
                                    focus_zone.set(ModalFocus::LocalPort);
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            _ => {}
                        }
                    }
                }
            },
            div {
                class: "modal-content portforward-modal",
                onclick: move |e| e.stop_propagation(),

                div { class: "modal-header",
                    h3 { "Port Forward" }
                    span { class: "modal-subtitle", "{props.pod_name}" }
                }

                div { class: "modal-body",
                    // Container port selection
                    div { class: "form-group",
                        label { "Container Port" }
                        if props.container_ports.is_empty() {
                            div { class: "empty-hint", "No ports defined - enter manually below" }
                        } else {
                            div { class: "port-list",
                                for (idx, (port, name, protocol)) in props.container_ports.iter().enumerate() {
                                    {
                                        let container_ports = props.container_ports.clone();
                                        let is_selected = idx == *selected_port_index.read() && *focus_zone.read() == ModalFocus::PortList;
                                        rsx! {
                                            div {
                                                class: if is_selected { "port-option selected" } else { "port-option" },
                                                onclick: move |_| {
                                                    selected_port_index.set(idx);
                                                    if let Some((p, _, _)) = container_ports.get(idx) {
                                                        local_port_input.set(p.to_string());
                                                    }
                                                    focus_zone.set(ModalFocus::PortList);
                                                },
                                                span { class: "port-number", "{port}" }
                                                if let Some(n) = name {
                                                    span { class: "port-name", "{n}" }
                                                }
                                                span { class: "port-protocol", "{protocol}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Local port input
                    div { class: "form-group",
                        label { "Local Port" }
                        input {
                            r#type: "text",
                            class: if *focus_zone.read() == ModalFocus::LocalPort { "focused" } else { "" },
                            onmounted: move |e| {
                                // Focus the input on mount
                                let data = e.data();
                                spawn(async move {
                                    let _ = data.set_focus(true).await;
                                });
                            },
                            onfocus: move |_| focus_zone.set(ModalFocus::LocalPort),
                            oninput: move |e| {
                                // Only allow numeric input
                                let val = e.value();
                                let filtered: String = val.chars().filter(|c| c.is_ascii_digit()).collect();
                                local_port_input.set(filtered);
                                error_message.set(None);
                            },
                            placeholder: "Local port (e.g., 8080)",
                        }
                        span { class: "form-hint",
                            {
                                let selected_port = if props.container_ports.is_empty() {
                                    local_port_input.read().clone()
                                } else {
                                    props.container_ports.get(*selected_port_index.read())
                                        .map(|(p, _, _)| p.to_string())
                                        .unwrap_or_else(|| local_port_input.read().clone())
                                };
                                format!("localhost:{} → pod:{}", local_port_input.read(), selected_port)
                            }
                        }
                    }

                    // Error message
                    if let Some(err) = error_message.read().as_ref() {
                        div { class: "form-error", "{err}" }
                    }
                }

                div { class: "modal-footer",
                    span { class: "modal-hints", "↑↓ Select Port • Tab Switch • Enter Confirm • Esc Cancel" }
                    div { class: "modal-actions",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| on_cancel.call(()),
                            "Cancel"
                        }
                        {
                            let container_ports = props.container_ports.clone();
                            rsx! {
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| {
                                        let local_port_str = local_port_input.read().clone();
                                        match local_port_str.parse::<u16>() {
                                            Ok(local_port) => {
                                                if local_port == 0 {
                                                    error_message.set(Some("Port must be greater than 0".to_string()));
                                                } else {
                                                    let remote_port = if container_ports.is_empty() {
                                                        local_port
                                                    } else {
                                                        container_ports.get(*selected_port_index.read()).map(|(p, _, _)| *p).unwrap_or(local_port)
                                                    };
                                                    on_confirm.call((local_port, remote_port));
                                                }
                                            }
                                            Err(_) => {
                                                error_message.set(Some("Invalid port number".to_string()));
                                            }
                                        }
                                    },
                                    "Start Forward"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
