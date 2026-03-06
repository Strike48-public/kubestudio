//! Keyboard context and hotkey helpers infrastructure for future refactor
//! Currently unused - will centralize keyboard handling from app.rs

#![allow(dead_code)]

use crate::components::Hotkey;
use dioxus::prelude::*;

/// Context for keyboard handling - determines which hotkeys are active
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum KeyboardContext {
    /// Main resource list (pods, deployments, etc.)
    #[default]
    ResourceList,
    /// Overview/dashboard
    Overview,
    /// Container drill-down view
    ContainerDrillDown,
    /// Log viewer
    LogViewer,
    /// YAML/Describe viewer
    YamlViewer,
    /// Delete confirmation modal
    DeleteModal,
    /// Search input active
    Search,
}

/// Get hotkeys for a given keyboard context
pub fn get_hotkeys_for_context(context: KeyboardContext, is_pods_view: bool) -> Vec<Hotkey> {
    match context {
        KeyboardContext::ResourceList => {
            let mut keys = vec![
                Hotkey {
                    key: "d".to_string(),
                    description: "Describe".to_string(),
                },
                Hotkey {
                    key: "^d".to_string(),
                    description: "Delete".to_string(),
                },
                Hotkey {
                    key: "^k".to_string(),
                    description: "Kill".to_string(),
                },
            ];
            if is_pods_view {
                keys.push(Hotkey {
                    key: "l".to_string(),
                    description: "Logs".to_string(),
                });
                keys.push(Hotkey {
                    key: "s".to_string(),
                    description: "Shell".to_string(),
                });
            }
            keys.extend(vec![
                Hotkey {
                    key: "/".to_string(),
                    description: "Search".to_string(),
                },
                Hotkey {
                    key: "n".to_string(),
                    description: "Namespace".to_string(),
                },
                Hotkey {
                    key: "←→".to_string(),
                    description: "Sidebar".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Back".to_string(),
                },
            ]);
            keys
        }
        KeyboardContext::Overview => {
            vec![
                Hotkey {
                    key: "1-9".to_string(),
                    description: "Resources".to_string(),
                },
                Hotkey {
                    key: "/".to_string(),
                    description: "Search".to_string(),
                },
                Hotkey {
                    key: "?".to_string(),
                    description: "Help".to_string(),
                },
            ]
        }
        KeyboardContext::ContainerDrillDown => {
            vec![
                Hotkey {
                    key: "↑↓".to_string(),
                    description: "Navigate".to_string(),
                },
                Hotkey {
                    key: "Enter".to_string(),
                    description: "View Logs".to_string(),
                },
                Hotkey {
                    key: "l".to_string(),
                    description: "Logs".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Back".to_string(),
                },
            ]
        }
        KeyboardContext::LogViewer => {
            vec![
                Hotkey {
                    key: "w".to_string(),
                    description: "Toggle Wrap".to_string(),
                },
                Hotkey {
                    key: "↑↓←→".to_string(),
                    description: "Scroll".to_string(),
                },
                Hotkey {
                    key: "PgUp/Dn".to_string(),
                    description: "Page".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Back".to_string(),
                },
            ]
        }
        KeyboardContext::YamlViewer => {
            vec![
                Hotkey {
                    key: "c".to_string(),
                    description: "Copy".to_string(),
                },
                Hotkey {
                    key: "h".to_string(),
                    description: "Toggle View".to_string(),
                },
                Hotkey {
                    key: "w".to_string(),
                    description: "Toggle Wrap".to_string(),
                },
                Hotkey {
                    key: "↑↓←→".to_string(),
                    description: "Scroll".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Back".to_string(),
                },
            ]
        }
        KeyboardContext::DeleteModal => {
            vec![
                Hotkey {
                    key: "←→".to_string(),
                    description: "Select".to_string(),
                },
                Hotkey {
                    key: "Enter".to_string(),
                    description: "Confirm".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Cancel".to_string(),
                },
            ]
        }
        KeyboardContext::Search => {
            vec![
                Hotkey {
                    key: "Enter".to_string(),
                    description: "Search".to_string(),
                },
                Hotkey {
                    key: "Esc".to_string(),
                    description: "Cancel".to_string(),
                },
            ]
        }
    }
}

/// Universal keys that are handled the same way across most contexts
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum UniversalKey {
    Escape,
    Enter,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Search,   // '/'
    Help,     // '?'
    Describe, // 'd'
    Logs,     // 'l'
    Delete,   // Ctrl+d
    Kill,     // Ctrl+k
}

/// Try to parse a keyboard event into a universal key
pub fn parse_universal_key(
    key: &dioxus::prelude::Key,
    modifiers: &dioxus::prelude::Modifiers,
) -> Option<UniversalKey> {
    match key {
        dioxus::prelude::Key::Escape => Some(UniversalKey::Escape),
        dioxus::prelude::Key::Enter => Some(UniversalKey::Enter),
        dioxus::prelude::Key::ArrowUp => Some(UniversalKey::ArrowUp),
        dioxus::prelude::Key::ArrowDown => Some(UniversalKey::ArrowDown),
        dioxus::prelude::Key::ArrowLeft => Some(UniversalKey::ArrowLeft),
        dioxus::prelude::Key::ArrowRight => Some(UniversalKey::ArrowRight),
        dioxus::prelude::Key::Character(c) => {
            if modifiers.ctrl() {
                match c.as_str() {
                    "d" => Some(UniversalKey::Delete),
                    "k" => Some(UniversalKey::Kill),
                    _ => None,
                }
            } else if !modifiers.meta() {
                match c.as_str() {
                    "/" => Some(UniversalKey::Search),
                    "?" => Some(UniversalKey::Help),
                    "d" => Some(UniversalKey::Describe),
                    "l" => Some(UniversalKey::Logs),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Hook that provides context-aware hotkeys
/// Returns a signal that updates based on the current keyboard context
pub fn use_hotkeys(
    context: Signal<KeyboardContext>,
    is_pods_view: Signal<bool>,
) -> Memo<Vec<Hotkey>> {
    use_memo(move || {
        let ctx = *context.read();
        let pods = *is_pods_view.read();
        get_hotkeys_for_context(ctx, pods)
    })
}
