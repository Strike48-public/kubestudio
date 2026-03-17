use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum ApplySource {
    File(String), // File path
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApplyManifestState {
    /// Initial state - show options or loading file
    Loading,
    /// Preview the YAML before applying
    Preview {
        yaml: String,
        source: String,
        doc_count: usize,
    },
    /// Applying the manifests
    Applying,
    /// Show results
    Results {
        applied: Vec<String>,
        errors: Vec<String>,
    },
    /// Error loading the manifest
    Error(String),
}

#[derive(Props, Clone, PartialEq)]
pub struct ApplyManifestProps {
    /// Cluster connection for applying resources
    pub cluster: Signal<Option<ClusterContext>>,
    /// Source of the manifest (file or clipboard)
    pub source: ApplySource,
    /// Called when user closes the view
    pub on_close: EventHandler<()>,
    /// Remappable keybindings
    #[props(default)]
    pub keybindings: ks_plugin::KeyBindings,
}

#[component]
pub fn ApplyManifest(props: ApplyManifestProps) -> Element {
    let mut state = use_signal(|| ApplyManifestState::Loading);
    let mut edited_yaml = use_signal(String::new);
    let cluster = props.cluster;
    let on_close = props.on_close;
    let source = props.source.clone();

    // Track container ref for focus management
    let mut container_ref = use_signal(|| None::<Rc<MountedData>>);
    let mut should_refocus = use_signal(|| false);

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

    // Load content on mount
    use_effect(move || {
        let source = source.clone();
        spawn(async move {
            match source {
                ApplySource::File(path) => match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        let doc_count = count_yaml_documents(&content);
                        edited_yaml.set(content.clone());
                        state.set(ApplyManifestState::Preview {
                            yaml: content,
                            source: path,
                            doc_count,
                        });
                    }
                    Err(e) => {
                        state.set(ApplyManifestState::Error(format!(
                            "Failed to read file: {}",
                            e
                        )));
                    }
                },
            }
        });
    });

    rsx! {
        div {
            class: "apply-manifest-container",
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
                    ApplyManifestState::Preview { .. } => {
                        // Handle apply_manifest_confirm keybinding (default Ctrl+Enter)
                        let confirm_match = if let Key::Character(ref c) = e.key() {
                            props.keybindings.matches("apply_manifest_confirm", c, e.modifiers().ctrl(), e.modifiers().shift(), e.modifiers().alt(), e.modifiers().meta())
                        } else if e.key() == Key::Enter {
                            // Also check if Enter with modifiers matches (for Ctrl+Enter default)
                            props.keybindings.matches("apply_manifest_confirm", "Enter", e.modifiers().ctrl(), e.modifiers().shift(), e.modifiers().alt(), e.modifiers().meta())
                        } else { false };
                        if confirm_match {
                            let yaml_to_apply = edited_yaml.read().clone();
                            state.set(ApplyManifestState::Applying);

                            spawn(async move {
                                if let Some(ctx) = cluster.read().clone() {
                                    match ctx.client.apply_multi_yaml(&yaml_to_apply).await {
                                        Ok(result) => {
                                            let applied: Vec<String> = result.results
                                                .iter()
                                                .map(|r| format!("{}/{}", r.kind, r.name))
                                                .collect();
                                            state.set(ApplyManifestState::Results {
                                                applied,
                                                errors: result.errors,
                                            });
                                            // Refocus container so hotkeys work
                                            should_refocus.set(true);
                                        }
                                        Err(err) => {
                                            state.set(ApplyManifestState::Error(err.to_string()));
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

                        if crate::utils::is_escape(&e) {
                            on_close.call(());
                            e.stop_propagation();
                            e.prevent_default();
                            return;
                        }

                        // Stop all other events from bubbling to app container
                        // Textarea handles its own input (arrow keys, typing, etc.)
                        e.stop_propagation();
                    }
                    ApplyManifestState::Results { .. } | ApplyManifestState::Error(_) => {
                        if crate::utils::is_escape(&e) || e.key() == Key::Enter {
                            on_close.call(());
                            e.stop_propagation();
                            e.prevent_default();
                        }
                    }
                    _ => {
                        if crate::utils::is_escape(&e) {
                            on_close.call(());
                            e.stop_propagation();
                            e.prevent_default();
                        }
                    }
                }
            },

            {
                let current_state = state.read().clone();
                match current_state {
                    ApplyManifestState::Loading => rsx! {
                        div { class: "apply-status",
                            div { class: "apply-loading", "Loading manifest..." }
                        }
                    },
                    ApplyManifestState::Preview { source, doc_count, .. } => rsx! {
                        div { class: "apply-header",
                            h3 { "Apply Manifest" }
                            span { class: "apply-hint",
                                {format!("Source: {} • {} document(s) • {} to apply • Esc to cancel", source, doc_count, props.keybindings.display("apply_manifest_confirm"))}
                            }
                        }
                        textarea {
                            class: "yaml-editor apply-editor",
                            value: "{edited_yaml}",
                            oninput: move |e| {
                                edited_yaml.set(e.value());
                            },
                            onmounted: move |_e| {
                                spawn(async move {
                                    let _ = document::eval(
                                        r#"
                                        const editor = document.querySelector('.apply-editor');
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
                    ApplyManifestState::Applying => rsx! {
                        div { class: "apply-status",
                            div { class: "apply-loading", "Applying manifests..." }
                        }
                    },
                    ApplyManifestState::Results { applied, errors } => rsx! {
                        div { class: "apply-results",
                            if !applied.is_empty() {
                                div { class: "apply-success-section",
                                    h4 { "Applied Successfully ({applied.len()})" }
                                    for item in applied.iter() {
                                        div { class: "apply-success-item", "{item}" }
                                    }
                                }
                            }
                            if !errors.is_empty() {
                                div { class: "apply-error-section",
                                    h4 { "Errors ({errors.len()})" }
                                    for err in errors.iter() {
                                        div { class: "apply-error-item", "{err}" }
                                    }
                                }
                            }
                            div { class: "apply-hint", "Press Enter or Esc to close" }
                        }
                    },
                    ApplyManifestState::Error(ref msg) => rsx! {
                        div { class: "apply-status",
                            div { class: "apply-error", "{msg}" }
                            div { class: "apply-hint", "Press Enter or Esc to close" }
                        }
                    },
                }
            }
        }
    }
}

/// Count YAML documents in a string (separated by ---)
fn count_yaml_documents(yaml: &str) -> usize {
    yaml.split("\n---")
        .map(|s| s.trim_start_matches("---").trim())
        .filter(|s| !s.is_empty())
        .count()
        .max(1) // At least 1 document if there's content
}
