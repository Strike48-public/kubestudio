//! YAML viewer component with syntax highlighting, secret masking, and edit support

mod describe;
mod helpers;
mod secrets;
mod syntax;

use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use ks_kube::CrdInfo;
use std::rc::Rc;

use describe::yaml_to_describe_format;
use helpers::strip_managed_fields;
use secrets::mask_secret_data;
use syntax::highlight_yaml_line;

#[derive(Debug, Clone, PartialEq)]
pub enum ApplyState {
    Idle,
    Confirming,
    Applying,
    Success(String),
    Error(String),
}

#[derive(Props, Clone, PartialEq)]
pub struct YamlViewerProps {
    /// Resource kind (e.g., "Pod", "Deployment")
    pub kind: String,
    /// Resource name
    pub name: String,
    /// Resource namespace (None for cluster-scoped resources)
    pub namespace: Option<String>,
    /// Cluster connection for fetching YAML
    pub cluster: Signal<Option<ClusterContext>>,
    /// Called when user presses Escape or Back button
    pub on_back: EventHandler<()>,
    /// Optional CRD info for custom resources
    #[props(default = None)]
    pub crd_info: Option<CrdInfo>,
    /// Read-only mode - disables editing
    #[props(default = false)]
    pub read_only: bool,
}

#[component]
pub fn YamlViewer(props: YamlViewerProps) -> Element {
    // Internal state
    let mut content = use_signal(String::new);
    let mut content_full = use_signal(String::new);
    let mut loading = use_signal(|| true);
    let mut view_mode = use_signal(|| "describe".to_string()); // "yaml" or "describe"
    let mut show_managed_fields = use_signal(|| false);
    let mut text_wrap = use_signal(|| true);
    #[allow(unused_mut)]
    let mut copied = use_signal(|| false);
    let mut viewer_ref = use_signal(|| None::<Rc<MountedData>>);

    // Edit mode state
    let mut edit_mode = use_signal(|| false);
    let mut edited_content = use_signal(String::new);
    let mut apply_state = use_signal(|| ApplyState::Idle);
    let mut modal_selected = use_signal(|| 0usize); // 0 = Cancel, 1 = Apply (Cancel preselected)
    let mut should_refocus_content = use_signal(|| false);

    // Secret masking state - secrets are masked by default
    let mut secrets_revealed = use_signal(|| false);
    let is_secret = props.kind == "Secret";

    // Fetch YAML when props change
    let kind = props.kind.clone();
    let name = props.name.clone();
    let namespace = props.namespace.clone();
    let cluster = props.cluster;
    let crd_info = props.crd_info.clone();

    use_effect(move || {
        let kind = kind.clone();
        let name = name.clone();
        let namespace = namespace.clone();
        let crd_info = crd_info.clone();

        loading.set(true);
        content.set(String::new());
        content_full.set(String::new());
        show_managed_fields.set(false);
        view_mode.set("describe".to_string());
        secrets_revealed.set(false); // Reset secret reveal state when switching resources

        spawn(async move {
            if let Some(ctx) = cluster.read().clone() {
                let result = ctx
                    .client
                    .get_resource_yaml_with_crd(
                        &kind,
                        &name,
                        namespace.as_deref(),
                        crd_info.as_ref(),
                    )
                    .await;
                match result {
                    Ok(yaml) => {
                        content_full.set(yaml.clone());
                        // Generate describe format by default
                        let describe_text = yaml_to_describe_format(&yaml);
                        content.set(describe_text);
                    }
                    Err(e) => {
                        let error_msg = format!("Error fetching resource: {}", e);
                        content.set(error_msg.clone());
                        content_full.set(error_msg);
                    }
                }
            }
            loading.set(false);
        });
    });

    // Effect to refocus content after edit mode exits
    use_effect(move || {
        if *should_refocus_content.read() {
            should_refocus_content.set(false);
            spawn(async move {
                // Small delay to ensure DOM is updated
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                let _ = document::eval(
                    r#"
                    const content = document.querySelector('.describe-content');
                    if (content) {
                        content.focus();
                    }
                    "#,
                );
            });
        }
    });

    // Clone for closures
    let on_back = props.on_back;
    let on_back_key = props.on_back;
    let kind_display = props.kind.clone();
    let name_display = props.name.clone();
    let namespace_display = props.namespace.clone();

    rsx! {
        div { class: "describe-viewer-container",
            div { class: "describe-viewer-header",
                div { style: "display: flex; align-items: center; gap: 1rem; flex: 1;",
                    h3 {
                        {
                            let mode = view_mode.read().clone();
                            let is_editing = *edit_mode.read();
                            if is_editing {
                                format!("Edit: {}/{}", kind_display, name_display)
                            } else if mode == "yaml" {
                                format!("YAML: {}/{}", kind_display, name_display)
                            } else {
                                format!("Describe: {}/{}", kind_display, name_display)
                            }
                        }
                    }
                    if let Some(ns) = &namespace_display {
                        span { class: "container-name", " in {ns}" }
                    }
                    // Edit mode indicator
                    if *edit_mode.read() {
                        span { class: "status-badge status-warning", "Editing" }
                    }
                    // Secret reveal indicator
                    if is_secret && *secrets_revealed.read() {
                        span { class: "status-badge status-error", "Secrets Revealed" }
                    } else if is_secret {
                        span { class: "status-badge status-success", "Secrets Masked" }
                    }
                    // Status messages
                    match &*apply_state.read() {
                        ApplyState::Success(msg) => rsx! {
                            span {
                                style: "margin-left: auto; color: var(--success); font-size: 0.875rem; font-weight: 600;",
                                "{msg}"
                            }
                        },
                        ApplyState::Error(msg) => rsx! {
                            span {
                                style: "margin-left: auto; color: var(--destructive); font-size: 0.875rem;",
                                "{msg}"
                            }
                        },
                        ApplyState::Applying => rsx! {
                            span {
                                style: "margin-left: auto; color: var(--muted-foreground); font-size: 0.875rem;",
                                "Applying..."
                            }
                        },
                        _ => {
                            if copied() {
                                rsx! {
                                    span {
                                        style: "margin-left: auto; color: var(--success); font-size: 0.875rem; font-weight: 600;",
                                        "Copied to clipboard!"
                                    }
                                }
                            } else {
                                rsx! {
                                    span {
                                        style: "margin-left: auto; color: var(--muted-foreground); font-size: 0.75rem;",
                                        {
                                            let mode = view_mode.read().clone();
                                            let is_editing = *edit_mode.read();
                                            let show_mgmt = show_managed_fields();
                                            let revealed = *secrets_revealed.read();
                                            let secret_hint = if is_secret {
                                                if revealed { " | r: hide secrets" } else { " | r: reveal secrets" }
                                            } else { "" };
                                            let edit_hint = if props.read_only { "" } else { "e: edit | " };
                                            if is_editing {
                                                "Ctrl+S: Apply | Esc: Cancel".to_string()
                                            } else if mode == "yaml" {
                                                if show_mgmt {
                                                    format!("{}c: copy | m: hide mgmt | h: human view{}", edit_hint, secret_hint)
                                                } else {
                                                    format!("{}c: copy | m: show mgmt | h: human view{}", edit_hint, secret_hint)
                                                }
                                            } else {
                                                format!("{}c: copy | h: yaml view{}", edit_hint, secret_hint)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if *edit_mode.read() {
                    button {
                        class: "back-btn",
                        onclick: move |_| {
                            edit_mode.set(false);
                            apply_state.set(ApplyState::Idle);
                            // Restore view
                            if show_managed_fields() {
                                content.set(content_full.read().clone());
                            } else {
                                let stripped = strip_managed_fields(&content_full.read());
                                content.set(stripped);
                            }
                        },
                        "Cancel (Esc)"
                    }
                } else {
                    button {
                        class: "back-btn",
                        onclick: move |_| on_back.call(()),
                        "Back (Esc)"
                    }
                }
            }
            div {
                class: if text_wrap() { "describe-content" } else { "describe-content nowrap" },
                tabindex: 0,
                onmounted: move |e| {
                    viewer_ref.set(Some(e.data()));
                    let data = e.data();
                    spawn(async move {
                        let _ = data.set_focus(true).await;
                    });
                },
                onkeydown: move |e: KeyboardEvent| {
                    // Handle Ctrl+S for save/apply
                    if (e.modifiers().ctrl() || e.modifiers().meta()) && e.key() == Key::Character("s".to_string()) {
                        if *edit_mode.read() {
                            // Trigger apply confirmation
                            apply_state.set(ApplyState::Confirming);
                        }
                        e.stop_propagation();
                        e.prevent_default();
                        return;
                    }

                    match e.key() {
                        Key::Character(ref c) if !e.modifiers().ctrl() && !e.modifiers().meta() => {
                            // Skip most shortcuts in edit mode (let textarea handle them)
                            if *edit_mode.read() {
                                e.stop_propagation();
                                return;
                            }
                            match c.as_str() {
                                "e" => {
                                    // Enter edit mode - switch to YAML view
                                    if props.read_only {
                                        tracing::warn!("Edit disabled in read-only mode (KUBESTUDIO_MODE=read)");
                                    } else if !*edit_mode.read() {
                                        view_mode.set("yaml".to_string());
                                        // Get clean YAML without managed fields for editing
                                        let yaml_for_edit = strip_managed_fields(&content_full.read());
                                        edited_content.set(yaml_for_edit.clone());
                                        content.set(yaml_for_edit);
                                        edit_mode.set(true);
                                        apply_state.set(ApplyState::Idle);
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                "w" => {
                                    let new_wrap = !*text_wrap.read();
                                    text_wrap.set(new_wrap);
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                "c" => {
                                    #[cfg(feature = "desktop")]
                                    {
                                        let current_content = content.read().clone();
                                        if !current_content.is_empty() {
                                            spawn(async move {
                                                match arboard::Clipboard::new() {
                                                    Ok(mut clipboard) => {
                                                        if clipboard.set_text(&current_content).is_ok() {
                                                            copied.set(true);
                                                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                                            copied.set(false);
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to access clipboard: {}", e);
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    #[cfg(not(feature = "desktop"))]
                                    {
                                        // Clipboard not available in web/fullstack mode
                                        tracing::debug!("Clipboard copy not available in web mode");
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                "m" => {
                                    let show = !show_managed_fields();
                                    show_managed_fields.set(show);
                                    if view_mode.read().as_str() == "yaml" {
                                        if show {
                                            content.set(content_full.read().clone());
                                        } else {
                                            let stripped = strip_managed_fields(&content_full.read());
                                            content.set(stripped);
                                        }
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                "h" => {
                                    let current_mode = view_mode.read().clone();
                                    if current_mode == "yaml" {
                                        view_mode.set("describe".to_string());
                                        let full_yaml = content_full.read().clone();
                                        let describe_text = yaml_to_describe_format(&full_yaml);
                                        content.set(describe_text);
                                    } else {
                                        view_mode.set("yaml".to_string());
                                        if show_managed_fields() {
                                            content.set(content_full.read().clone());
                                        } else {
                                            let stripped = strip_managed_fields(&content_full.read());
                                            content.set(stripped);
                                        }
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                "r" => {
                                    // Toggle secret value reveal (only for Secrets)
                                    if is_secret {
                                        let new_revealed = !*secrets_revealed.read();
                                        secrets_revealed.set(new_revealed);
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                _ => {
                                    e.stop_propagation();
                                }
                            }
                        }
                        Key::Escape => {
                            if *edit_mode.read() {
                                // Cancel edit mode - stay in YAML view
                                edit_mode.set(false);
                                apply_state.set(ApplyState::Idle);
                                // Keep YAML view, restore content without managed fields
                                view_mode.set("yaml".to_string());
                                let stripped = strip_managed_fields(&content_full.read());
                                content.set(stripped);
                                // Trigger refocus on content
                                should_refocus_content.set(true);
                            } else if matches!(*apply_state.read(), ApplyState::Confirming) {
                                // Cancel confirmation - go back to edit mode
                                apply_state.set(ApplyState::Idle);
                            } else {
                                on_back_key.call(());
                            }
                            e.stop_propagation();
                        }
                        Key::ArrowLeft | Key::ArrowRight => {
                            e.stop_propagation();
                        }
                        Key::ArrowDown | Key::ArrowUp | Key::PageDown | Key::PageUp | Key::Home | Key::End => {
                            e.stop_propagation();
                        }
                        _ => {}
                    }
                },
                if loading() {
                    div { class: "yaml-loading", "Loading YAML..." }
                } else if content.read().is_empty() {
                    div { class: "yaml-empty", "No data available" }
                } else if *edit_mode.read() {
                    // Edit mode - show textarea
                    textarea {
                        class: "yaml-editor",
                        value: "{edited_content}",
                        oninput: move |e| {
                            edited_content.set(e.value());
                        },
                        onmounted: move |_e| {
                            // Focus and scroll to top, position cursor at start
                            spawn(async move {
                                let _ = document::eval(
                                    r#"
                                    const editor = document.querySelector('.yaml-editor');
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
                } else {
                    pre {
                        class: "yaml-content",
                        {
                            let yaml_text = content.read().clone();
                            let current_view_mode = view_mode.read().clone();
                            let revealed = *secrets_revealed.read();

                            // Apply secret masking if viewing a secret and not revealed
                            // Masking applies to both YAML and describe views
                            let display_text = if is_secret && !revealed {
                                mask_secret_data(&yaml_text)
                            } else {
                                yaml_text
                            };
                            let lines: Vec<&str> = display_text.lines().collect();

                            rsx! {
                                for (idx, line) in lines.iter().enumerate() {
                                    div { class: "yaml-line", key: "{idx}",
                                        span { class: "line-number", "{idx + 1}" }
                                        span {
                                            class: "line-content",
                                            dangerous_inner_html: if current_view_mode == "yaml" {
                                                highlight_yaml_line(line)
                                            } else {
                                                line.replace('&', "&amp;")
                                                    .replace('<', "&lt;")
                                                    .replace('>', "&gt;")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Confirmation modal with keyboard navigation
            if matches!(*apply_state.read(), ApplyState::Confirming) {
                div {
                    class: "modal-overlay",
                    tabindex: 0,
                    onmounted: move |e| {
                        let data = e.data();
                        modal_selected.set(0); // Default to Cancel button
                        spawn(async move {
                            let _ = data.set_focus(true).await;
                        });
                    },
                    onkeydown: move |e: KeyboardEvent| {
                        match e.key() {
                            Key::Escape => {
                                apply_state.set(ApplyState::Idle);
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::Tab | Key::ArrowLeft | Key::ArrowRight => {
                                // Toggle selection
                                let current = *modal_selected.read();
                                modal_selected.set(if current == 0 { 1 } else { 0 });
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            Key::Enter => {
                                let selected = *modal_selected.read();
                                if selected == 0 {
                                    // Cancel
                                    apply_state.set(ApplyState::Idle);
                                } else {
                                    // Apply
                                    let yaml_to_apply = edited_content.read().clone();
                                    apply_state.set(ApplyState::Applying);

                                    spawn(async move {
                                        if let Some(ctx) = cluster.read().clone() {
                                            match ctx.client.apply_yaml(&yaml_to_apply).await {
                                                Ok(result) => {
                                                    let msg = format!("{}/{} applied", result.kind, result.name);
                                                    apply_state.set(ApplyState::Success(msg));
                                                    // Update the stored content
                                                    content_full.set(yaml_to_apply.clone());
                                                    content.set(yaml_to_apply);
                                                    edit_mode.set(false);
                                                    // Stay in YAML view and refocus
                                                    view_mode.set("yaml".to_string());
                                                    should_refocus_content.set(true);
                                                    // Clear success message after delay
                                                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                                                    apply_state.set(ApplyState::Idle);
                                                }
                                                Err(e) => {
                                                    apply_state.set(ApplyState::Error(e.to_string()));
                                                }
                                            }
                                        }
                                    });
                                }
                                e.stop_propagation();
                                e.prevent_default();
                            }
                            _ => {
                                e.stop_propagation();
                                e.prevent_default();
                            }
                        }
                    },
                    div { class: "modal-content",
                        h3 { "Apply Changes?" }
                        p { "This will apply the modified YAML to the cluster." }
                        div { class: "modal-actions",
                            button {
                                class: if *modal_selected.read() == 0 { "btn btn-secondary selected" } else { "btn btn-secondary" },
                                onclick: move |_| {
                                    apply_state.set(ApplyState::Idle);
                                },
                                "Cancel"
                            }
                            button {
                                class: if *modal_selected.read() == 1 { "btn btn-primary selected" } else { "btn btn-primary" },
                                onclick: move |_| {
                                    let yaml_to_apply = edited_content.read().clone();
                                    apply_state.set(ApplyState::Applying);

                                    spawn(async move {
                                        if let Some(ctx) = cluster.read().clone() {
                                            match ctx.client.apply_yaml(&yaml_to_apply).await {
                                                Ok(result) => {
                                                    let msg = format!("{}/{} applied", result.kind, result.name);
                                                    apply_state.set(ApplyState::Success(msg));
                                                    content_full.set(yaml_to_apply.clone());
                                                    content.set(yaml_to_apply);
                                                    edit_mode.set(false);
                                                    view_mode.set("yaml".to_string());
                                                    should_refocus_content.set(true);
                                                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                                                    apply_state.set(ApplyState::Idle);
                                                }
                                                Err(e) => {
                                                    apply_state.set(ApplyState::Error(e.to_string()));
                                                }
                                            }
                                        }
                                    });
                                },
                                "Apply"
                            }
                        }
                    }
                }
            }
        }
    }
}
