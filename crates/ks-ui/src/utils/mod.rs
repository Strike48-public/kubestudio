use dioxus::prelude::*;

/// Returns true if the keyboard event represents an Escape action.
/// Matches both the Escape key and Ctrl+[ (VT100 escape sequence).
pub fn is_escape(e: &KeyboardEvent) -> bool {
    e.key() == Key::Escape
        || (e.key() == Key::Character("[".into()) && e.modifiers().ctrl())
}
