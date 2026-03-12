use dioxus::prelude::*;

/// The type of action being confirmed
#[derive(Clone, PartialEq)]
pub enum ActionType {
    Restart,
    Trigger,
}

impl ActionType {
    pub fn title(&self) -> &'static str {
        match self {
            ActionType::Restart => "Restart Workload",
            ActionType::Trigger => "Trigger Job",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ActionType::Restart => {
                "This will perform a rolling restart of the workload, replacing all pods."
            }
            ActionType::Trigger => "This will create a new Job from the CronJob template.",
        }
    }

    pub fn button_text(&self) -> &'static str {
        match self {
            ActionType::Restart => "Restart",
            ActionType::Trigger => "Trigger",
        }
    }

    pub fn button_class(&self) -> &'static str {
        match self {
            ActionType::Restart => "action-btn warning",
            ActionType::Trigger => "action-btn primary",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ActionTarget {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct ActionConfirmModalProps {
    pub open: bool,
    pub action_type: ActionType,
    pub target: Option<ActionTarget>,
    pub on_confirm: EventHandler<ActionTarget>,
    pub on_cancel: EventHandler<()>,
}

#[component]
pub fn ActionConfirmModal(props: ActionConfirmModalProps) -> Element {
    let mut selected_option = use_signal(|| None::<bool>);

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

    let action_type = props.action_type.clone();
    let button_class = if *selected_option.read() == Some(true) {
        format!("{} selected", action_type.button_class())
    } else {
        action_type.button_class().to_string()
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
                    h3 { "{action_type.title()}" }
                }

                div { class: "delete-modal-body",
                    p { class: "delete-warning action-warning",
                        "{action_type.description()}"
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
                        class: "{button_class}",
                        onclick: {
                            let target = target.clone();
                            move |_| props.on_confirm.call(target.clone())
                        },
                        "{action_type.button_text()}"
                    }
                }
            }
        }
    }
}
