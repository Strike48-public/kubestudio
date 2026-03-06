use crate::components::templates::TEMPLATES;
use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum CreateState {
    SelectTemplate,
    EditYaml(String),
    Applying,
    Success(String),
    Error(String),
}

#[derive(Props, Clone, PartialEq)]
pub struct CreateResourceProps {
    /// Cluster connection for applying resources
    pub cluster: Signal<Option<ClusterContext>>,
    /// Current namespace (used as default in templates)
    pub namespace: Option<String>,
    /// Called when user cancels or finishes
    pub on_close: EventHandler<()>,
}

#[component]
pub fn CreateResource(props: CreateResourceProps) -> Element {
    let mut state = use_signal(|| CreateState::SelectTemplate);
    let mut selected_index = use_signal(|| 0usize);
    let mut edited_yaml = use_signal(String::new);
    let namespace = props.namespace.clone();
    let cluster = props.cluster;

    let on_close = props.on_close;

    // Track container ref for focus management
    let mut container_ref = use_signal(|| None::<Rc<MountedData>>);
    // Signal to trigger refocus when returning to template selection
    let mut should_refocus = use_signal(|| false);
    // Track keyboard navigation to suppress mouse hover
    let mut keyboard_nav_active = use_signal(|| false);

    // Effect to refocus container when needed
    use_effect(move || {
        if *should_refocus.read() {
            if let Some(container) = container_ref.read().clone() {
                spawn(async move {
                    let _ = container.set_focus(true).await;
                });
            }
            should_refocus.set(false);
        }
    });

    // Scroll selected template into view when navigating with keyboard
    use_effect(move || {
        let idx = *selected_index.read();
        let is_keyboard = *keyboard_nav_active.read();
        if is_keyboard {
            let js = format!(
                r#"
                const item = document.querySelector('[data-template-idx="{}"]');
                if (item) {{
                    item.scrollIntoView({{ block: 'nearest', behavior: 'smooth' }});
                }}
                "#,
                idx
            );
            spawn(async move {
                let _ = document::eval(&js).await;
            });
        }
    });

    // Clone namespace for closures
    let namespace_for_keydown = namespace.clone();
    let namespace_for_render = namespace.clone();

    rsx! {
        div {
            class: "create-resource-container",
            tabindex: 0,
            onmounted: move |e| {
                let data = e.data();
                container_ref.set(Some(data.clone()));
                spawn(async move {
                    let _ = data.set_focus(true).await;
                });
            },
            onkeydown: move |e: KeyboardEvent| {
                let current_state = state.read().clone();
                match current_state {
                    CreateState::SelectTemplate => {
                        match e.key() {
                            Key::Escape => {
                                on_close.call(());
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::ArrowDown => {
                                keyboard_nav_active.set(true);
                                let current = *selected_index.read();
                                if current < TEMPLATES.len() - 1 {
                                    selected_index.set(current + 1);
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::Character(ref c) if c == "j" => {
                                keyboard_nav_active.set(true);
                                let current = *selected_index.read();
                                if current < TEMPLATES.len() - 1 {
                                    selected_index.set(current + 1);
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::ArrowUp => {
                                keyboard_nav_active.set(true);
                                let current = *selected_index.read();
                                if current > 0 {
                                    selected_index.set(current - 1);
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::Character(ref c) if c == "k" => {
                                keyboard_nav_active.set(true);
                                let current = *selected_index.read();
                                if current > 0 {
                                    selected_index.set(current - 1);
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::Enter => {
                                let idx = *selected_index.read();
                                if let Some(template) = TEMPLATES.get(idx) {
                                    // Replace namespace in template if provided
                                    let yaml = if let Some(ns) = &namespace_for_keydown {
                                        template.yaml.replace("namespace: default", &format!("namespace: {}", ns))
                                    } else {
                                        template.yaml.to_string()
                                    };
                                    edited_yaml.set(yaml.clone());
                                    state.set(CreateState::EditYaml(yaml));
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            _ => {
                                e.stop_propagation();
                            }
                        }
                    }
                    CreateState::EditYaml(_) => {
                        // Handle Ctrl+S for apply
                        if (e.modifiers().ctrl() || e.modifiers().meta()) && e.key() == Key::Character("s".to_string()) {
                            let yaml_to_apply = edited_yaml.read().clone();
                            state.set(CreateState::Applying);

                            spawn(async move {
                                if let Some(ctx) = cluster.read().clone() {
                                    match ctx.client.apply_yaml(&yaml_to_apply).await {
                                        Ok(result) => {
                                            let msg = format!("{}/{} created", result.kind, result.name);
                                            state.set(CreateState::Success(msg));
                                            // Refocus container so hotkeys work
                                            should_refocus.set(true);
                                        }
                                        Err(err) => {
                                            state.set(CreateState::Error(err.to_string()));
                                            // Refocus container so hotkeys work
                                            should_refocus.set(true);
                                        }
                                    }
                                }
                            });
                            e.stop_propagation();
                            e.prevent_default();
                            return;
                        }

                        if e.key() == Key::Escape {
                            // Go back to template selection and refocus container
                            state.set(CreateState::SelectTemplate);
                            should_refocus.set(true);
                            e.stop_propagation();
                            e.prevent_default();
                            return;
                        }

                        // Stop all other events from bubbling to app container
                        // Textarea handles its own input (arrow keys, typing, etc.)
                        e.stop_propagation();
                    }
                    CreateState::Success(_) | CreateState::Error(_) => {
                        match e.key() {
                            Key::Escape | Key::Enter => {
                                on_close.call(());
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            },

            {
                let current_state = state.read().clone();
                match current_state {
                    CreateState::SelectTemplate => rsx! {
                        div { class: "create-header",
                            h3 { "Create Resource" }
                            span { class: "create-hint", "Select a template • Enter to edit • Esc to cancel" }
                        }
                        div {
                            class: if *keyboard_nav_active.read() { "template-list keyboard-nav" } else { "template-list" },
                            onmousemove: move |_| {
                                if *keyboard_nav_active.read() {
                                    keyboard_nav_active.set(false);
                                }
                            },
                            for (idx, template) in TEMPLATES.iter().enumerate() {
                                {
                                    let ns = namespace_for_render.clone();
                                    rsx! {
                                        div {
                                            key: "{idx}",
                                            "data-template-idx": "{idx}",
                                            class: if *selected_index.read() == idx { "template-item selected" } else { "template-item" },
                                            onclick: move |_| {
                                                selected_index.set(idx);
                                            },
                                            ondoubleclick: move |_| {
                                                if let Some(t) = TEMPLATES.get(idx) {
                                                    let yaml = if let Some(ref n) = ns {
                                                        t.yaml.replace("namespace: default", &format!("namespace: {}", n))
                                                    } else {
                                                        t.yaml.to_string()
                                                    };
                                                    edited_yaml.set(yaml.clone());
                                                    state.set(CreateState::EditYaml(yaml));
                                                }
                                            },
                                            div { class: "template-name", "{template.name}" }
                                            div { class: "template-desc", "{template.description}" }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    CreateState::EditYaml(_) => rsx! {
                        div { class: "create-header",
                            h3 { "Edit YAML" }
                            span { class: "create-hint", "Ctrl+S to apply • Esc to go back" }
                        }
                        textarea {
                            class: "yaml-editor create-editor",
                            value: "{edited_yaml}",
                            oninput: move |e| {
                                edited_yaml.set(e.value());
                            },
                            onmounted: move |_e| {
                                spawn(async move {
                                    let _ = document::eval(
                                        r#"
                                        const editor = document.querySelector('.create-editor');
                                        if (editor) {
                                            editor.focus();
                                            editor.scrollTop = 0;
                                            editor.setSelectionRange(0, 0);
                                        }
                                        "#,
                                    );
                                });
                            },
                        }
                    },
                    CreateState::Applying => rsx! {
                        div { class: "create-status",
                            div { class: "create-loading", "Applying..." }
                        }
                    },
                    CreateState::Success(ref msg) => rsx! {
                        div { class: "create-results",
                            div { class: "create-success-section",
                                h4 { "Created Successfully" }
                                div { class: "create-success-item", "{msg}" }
                            }
                            div { class: "create-hint", "Press Enter or Esc to close" }
                        }
                    },
                    CreateState::Error(ref msg) => rsx! {
                        div { class: "create-results",
                            div { class: "create-error-section",
                                h4 { "Error" }
                                div { class: "create-error-item", "{msg}" }
                            }
                            div { class: "create-hint", "Press Enter or Esc to close" }
                        }
                    },
                }
            }
        }
    }
}
