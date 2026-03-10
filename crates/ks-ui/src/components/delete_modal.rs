use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct DeleteTarget {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct DeleteModalProps {
    pub open: bool,
    pub target: Option<DeleteTarget>,
    pub on_confirm: EventHandler<DeleteTarget>,
    pub on_cancel: EventHandler<()>,
    #[props(default = false)]
    pub is_force: bool, // For ctrl-k "kill" which is force delete
}

#[component]
pub fn DeleteModal(props: DeleteModalProps) -> Element {
    let mut selected_option = use_signal(|| None::<bool>); // None = no selection, Some(true) = confirm, Some(false) = cancel

    // Reset selection when modal opens/closes
    use_effect(move || {
        if props.open {
            selected_option.set(None);
        }
    });

    if !props.open {
        return rsx! {};
    }

    let target = match &props.target {
        Some(t) => t.clone(),
        None => return rsx! {},
    };

    // Handle keyboard navigation
    let target_for_confirm = target.clone();
    let onkeydown = move |e: KeyboardEvent| {
        if crate::utils::is_escape(&e) {
            props.on_cancel.call(());
            e.stop_propagation();
        } else {
            match e.key() {
                Key::Enter => {
                    if let Some(true) = *selected_option.read() {
                        props.on_confirm.call(target_for_confirm.clone());
                    } else if let Some(false) = *selected_option.read() {
                        props.on_cancel.call(());
                    }
                    e.stop_propagation();
                }
                Key::ArrowLeft | Key::ArrowRight | Key::Tab => {
                    let current = *selected_option.read();
                    match current {
                        None => selected_option.set(Some(false)),
                        Some(true) => selected_option.set(Some(false)),
                        Some(false) => selected_option.set(Some(true)),
                    }
                    e.stop_propagation();
                    e.prevent_default();
                }
                _ => {}
            }
        }
    };

    let action_text = if props.is_force {
        "Force Delete (Kill)"
    } else {
        "Delete"
    };
    let action_class = if props.is_force {
        "delete-btn danger"
    } else {
        "delete-btn"
    };

    rsx! {
        div {
            class: "delete-modal-overlay",
            tabindex: 0,
            onkeydown: onkeydown,
            onclick: move |_| props.on_cancel.call(()),
            onmounted: move |e| {
                spawn(async move {
                    let _ = e.data().set_focus(true).await;
                });
            },

            div {
                class: "delete-modal",
                onclick: move |e| e.stop_propagation(),

                div { class: "delete-modal-header",
                    h3 { "{action_text} Resource" }
                }

                div { class: "delete-modal-body",
                    p { class: "delete-warning",
                        if props.is_force {
                            "This will forcefully terminate the resource immediately."
                        } else {
                            "Are you sure you want to delete this resource?"
                        }
                    }

                    div { class: "delete-target-info",
                        div { class: "target-row",
                            span { class: "target-label", "Kind:" }
                            span { class: "target-value", "{target.kind}" }
                        }
                        div { class: "target-row",
                            span { class: "target-label", "Name:" }
                            span { class: "target-value resource-name", "{target.name}" }
                        }
                        if let Some(ns) = &target.namespace {
                            div { class: "target-row",
                                span { class: "target-label", "Namespace:" }
                                span { class: "target-value", "{ns}" }
                            }
                        }
                    }

                    p { class: "delete-hint",
                        "Use " kbd { "Tab" } " or arrow keys to select, " kbd { "Enter" } " to confirm, " kbd { "Esc" } " to cancel"
                    }
                }

                div { class: "delete-modal-footer",
                    button {
                        class: if *selected_option.read() == Some(false) { "cancel-btn selected" } else { "cancel-btn" },
                        onclick: move |_| props.on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: if *selected_option.read() == Some(true) { format!("{} selected", action_class) } else { action_class.to_string() },
                        onclick: {
                            let target = target.clone();
                            move |_| props.on_confirm.call(target.clone())
                        },
                        "{action_text}"
                    }
                }
            }
        }
    }
}
