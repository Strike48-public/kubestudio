// Keybindings settings modal — click-to-record + text fallback

use dioxus::prelude::*;
use ks_plugin::{KeyBindings, ParsedHotkey};

#[derive(Props, Clone, PartialEq)]
pub struct KeybindingsModalProps {
    pub keybindings: KeyBindings,
    pub on_apply: EventHandler<KeyBindings>,
    pub on_cancel: EventHandler<()>,
}

/// Check if a Key is a bare modifier or Escape (should be ignored during recording).
fn is_modifier_or_escape(key: &Key) -> bool {
    matches!(
        key,
        Key::Control | Key::Shift | Key::Alt | Key::Meta | Key::Escape
    )
}

/// Build a hotkey string from a KeyboardEvent (e.g. "Ctrl+Shift+L").
/// Uses `e.code()` as fallback when modifiers cause `e.key()` to report
/// a control character instead of the physical key.
fn hotkey_from_event(e: &KeyboardEvent) -> Option<String> {
    let key = e.key();

    // Ignore bare modifier keys — we wait for the actual key
    if is_modifier_or_escape(&key) {
        return None;
    }

    let has_mods = e.modifiers().ctrl() || e.modifiers().alt() || e.modifiers().meta();

    let key_str = match &key {
        Key::Character(c) if !has_mods || c.chars().all(|ch| !ch.is_control()) => c.clone(),
        Key::Character(_) => {
            // Modifier held and key() gave a control character (e.g. Ctrl+B → '\x02').
            // Fall back to physical key code to get the actual letter.
            crate::app::code_to_key_str(e.code())?
        }
        Key::Enter if !has_mods => "Enter".to_string(),
        Key::Tab if !has_mods => "Tab".to_string(),
        Key::Backspace if !has_mods => "Backspace".to_string(),
        Key::Delete if !has_mods => "Delete".to_string(),
        _ if has_mods => {
            // Modifier held but key() didn't give Character — use physical key code
            crate::app::code_to_key_str(e.code())?
        }
        Key::Enter => "Enter".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::Backspace => "Backspace".to_string(),
        Key::Delete => "Delete".to_string(),
        Key::ArrowUp => "ArrowUp".to_string(),
        Key::ArrowDown => "ArrowDown".to_string(),
        Key::ArrowLeft => "ArrowLeft".to_string(),
        Key::ArrowRight => "ArrowRight".to_string(),
        Key::Home => "Home".to_string(),
        Key::End => "End".to_string(),
        Key::PageUp => "PageUp".to_string(),
        Key::PageDown => "PageDown".to_string(),
        _ => return None,
    };

    let mods = e.modifiers();
    let mut parts = Vec::new();
    if mods.ctrl() {
        parts.push("Ctrl");
    }
    if mods.shift() {
        parts.push("Shift");
    }
    if mods.alt() {
        parts.push("Alt");
    }
    if mods.meta() {
        parts.push("Meta");
    }
    parts.push(&key_str);
    Some(parts.join("+"))
}

/// Detect duplicate bindings within the same context.
/// Bindings in different contexts (e.g. Pod Actions vs Log Viewer) don't conflict
/// because they're never active simultaneously.
fn detect_conflicts(kb: &KeyBindings) -> Option<String> {
    let entries = kb.entries();
    // (canonical_key, context, action_id, label)
    let mut seen: Vec<(String, String, String, String)> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    for (_cat, action_id, label, value, context) in &entries {
        if value.is_empty() {
            continue;
        }
        let parsed = ParsedHotkey::parse(value);
        let canonical = format!(
            "{}{}{}{}{}",
            if parsed.ctrl { "C" } else { "" },
            if parsed.shift { "S" } else { "" },
            if parsed.alt { "A" } else { "" },
            if parsed.meta { "M" } else { "" },
            parsed.key.to_lowercase(),
        );
        for (prev_canon, prev_ctx, prev_action, prev_label) in &seen {
            if *prev_canon == canonical && prev_action != action_id && prev_ctx == context {
                conflicts.push(format!(
                    "\"{}\" and \"{}\" both use {}",
                    prev_label, label, value
                ));
            }
        }
        seen.push((
            canonical,
            context.to_string(),
            action_id.to_string(),
            label.to_string(),
        ));
    }

    if conflicts.is_empty() {
        None
    } else {
        Some(conflicts.join("; "))
    }
}

/// A single keybinding row entry (owned, for passing into RSX).
#[derive(Clone, PartialEq)]
struct KbEntry {
    category: String,
    action_id: String,
    label: String,
    value: String,
}

#[component]
pub fn KeybindingsModal(props: KeybindingsModalProps) -> Element {
    let initial_kb = props.keybindings.clone();
    let mut draft = use_signal(|| initial_kb.clone());
    let mut recording = use_signal(|| None::<String>);
    let mut editing = use_signal(|| None::<String>);
    let mut edit_text = use_signal(String::new);
    let mut conflict_warning = use_signal(|| detect_conflicts(&initial_kb));

    let on_apply = props.on_apply;
    let on_cancel = props.on_cancel;

    let warning = conflict_warning.read().clone();

    // Build owned entries from draft
    let entries: Vec<KbEntry> = draft
        .read()
        .entries()
        .into_iter()
        .map(|(c, a, l, v, _ctx)| KbEntry {
            category: c.to_string(),
            action_id: a.to_string(),
            label: l.to_string(),
            value: v.to_string(),
        })
        .collect();

    // Build categories list preserving order
    let mut categories: Vec<String> = Vec::new();
    for e in &entries {
        if categories.last().map(|c| c != &e.category).unwrap_or(true) {
            categories.push(e.category.clone());
        }
    }

    rsx! {
        div {
            class: "modal-overlay",
            tabindex: 0,
            onclick: move |_| on_cancel.call(()),
            onmounted: move |e| {
                let data = e.data();
                spawn(async move {
                    let _ = data.set_focus(true).await;
                });
            },
            onkeydown: move |e: KeyboardEvent| {
                let rec = recording.read().clone();
                if let Some(action_id) = rec {
                    e.stop_propagation();
                    e.prevent_default();

                    // Escape cancels recording
                    if e.key() == Key::Escape {
                        recording.set(None);
                        return;
                    }

                    if let Some(hotkey_str) = hotkey_from_event(&e) {
                        let mut d = draft.write();
                        d.set_binding(&action_id, hotkey_str);
                        conflict_warning.set(detect_conflicts(&d));
                        drop(d);
                        recording.set(None);
                    }
                    return;
                }

                if crate::utils::is_escape(&e) {
                    on_cancel.call(());
                    e.stop_propagation();
                    e.prevent_default();
                }
            },
            div {
                class: "keybindings-modal",
                onclick: move |e| e.stop_propagation(),

                h2 { "Keybinding Settings" }

                if let Some(ref warn) = warning {
                    div {
                        class: "conflict-warning",
                        "⚠ Conflicts: {warn}"
                    }
                }

                div {
                    class: "keybindings-body",

                    for cat in categories.iter() {
                        div {
                            class: "keybindings-category",
                            key: "{cat}",
                            h3 { "{cat}" }

                            {
                                let cat_entries: Vec<KbEntry> = entries.iter()
                                    .filter(|e| e.category == *cat)
                                    .cloned()
                                    .collect();
                                rsx! {
                                    for entry in cat_entries.into_iter() {
                                        {render_row(entry, &mut recording, &mut editing, &mut edit_text, &mut draft, &mut conflict_warning)}
                                    }
                                }
                            }
                        }
                    }
                }

                div {
                    class: "keybindings-footer",
                    button {
                        class: "kb-reset-all-btn",
                        onclick: move |_| {
                            draft.set(KeyBindings::default());
                            conflict_warning.set(None);
                        },
                        "Reset All"
                    }
                    div {
                        class: "keybindings-footer-right",
                        button {
                            class: "kb-cancel-btn",
                            onclick: move |_| on_cancel.call(()),
                            "Cancel"
                        }
                        button {
                            class: "kb-apply-btn",
                            onclick: move |_| {
                                on_apply.call(draft.read().clone());
                            },
                            "Apply"
                        }
                    }
                }
            }
        }
    }
}

fn render_row(
    entry: KbEntry,
    recording: &mut Signal<Option<String>>,
    editing: &mut Signal<Option<String>>,
    edit_text: &mut Signal<String>,
    draft: &mut Signal<KeyBindings>,
    conflict_warning: &mut Signal<Option<String>>,
) -> Element {
    let action_id = entry.action_id.clone();
    let label = entry.label.clone();
    let value = entry.value.clone();

    let rec_val = recording.read().clone();
    let edit_val = editing.read().clone();

    let is_recording = rec_val.as_deref() == Some(&action_id);
    let is_editing = edit_val.as_deref() == Some(&action_id);
    let row_class = if is_recording {
        "keybindings-row recording"
    } else {
        "keybindings-row"
    };

    let mut recording = *recording;
    let mut editing = *editing;
    let mut edit_text = *edit_text;
    let mut draft = *draft;
    let mut conflict_warning = *conflict_warning;

    let action_for_click = action_id.clone();
    let action_for_dblclick = action_id.clone();
    let value_for_dblclick = value.clone();
    let action_for_enter = action_id.clone();
    let action_for_blur = action_id.clone();
    let action_for_reset = action_id.clone();

    rsx! {
        div {
            class: "{row_class}",
            key: "{action_id}",
            span {
                class: "kb-label",
                "{label}"
            }
            if is_recording {
                span {
                    class: "kb-recording",
                    "Press a key\u{2026}"
                }
            } else if is_editing {
                input {
                    class: "kb-edit-input",
                    r#type: "text",
                    value: "{edit_text}",
                    autofocus: true,
                    onmounted: move |e| {
                        let data = e.data();
                        spawn(async move {
                            let _ = data.set_focus(true).await;
                        });
                    },
                    oninput: move |e: FormEvent| {
                        edit_text.set(e.value());
                    },
                    onkeydown: {
                        let action_id = action_for_enter.clone();
                        move |e: KeyboardEvent| {
                            e.stop_propagation();
                            if e.key() == Key::Enter {
                                let val = edit_text.read().clone();
                                if !val.is_empty() {
                                    let mut d = draft.write();
                                    d.set_binding(&action_id, val);
                                    conflict_warning.set(detect_conflicts(&d));
                                }
                                editing.set(None);
                                edit_text.set(String::new());
                                e.prevent_default();
                            } else if e.key() == Key::Escape {
                                editing.set(None);
                                edit_text.set(String::new());
                                e.prevent_default();
                            }
                        }
                    },
                    onfocusout: {
                        let action_id = action_for_blur.clone();
                        move |_| {
                            let val = edit_text.read().clone();
                            if !val.is_empty() {
                                let mut d = draft.write();
                                d.set_binding(&action_id, val);
                                conflict_warning.set(detect_conflicts(&d));
                            }
                            editing.set(None);
                            edit_text.set(String::new());
                        }
                    },
                }
            } else {
                kbd {
                    class: "kb-value",
                    onclick: move |e| {
                        e.stop_propagation();
                        editing.set(None);
                        recording.set(Some(action_for_click.clone()));
                    },
                    ondoubleclick: move |e| {
                        e.stop_propagation();
                        recording.set(None);
                        edit_text.set(value_for_dblclick.clone());
                        editing.set(Some(action_for_dblclick.clone()));
                    },
                    "{value}"
                }
            }
            button {
                class: "kb-reset-btn",
                title: "Reset to default",
                onclick: {
                    let action_id = action_for_reset;
                    move |e: MouseEvent| {
                        e.stop_propagation();
                        let default_val = KeyBindings::default()
                            .display(&action_id)
                            .to_string();
                        let mut d = draft.write();
                        d.set_binding(&action_id, default_val);
                        conflict_warning.set(detect_conflicts(&d));
                    }
                },
                "\u{21BA}"
            }
        }
    }
}
