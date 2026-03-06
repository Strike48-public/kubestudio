use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct Hotkey {
    pub key: String,
    pub description: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct HotkeysBarProps {
    pub hotkeys: Vec<Hotkey>,
}

#[component]
pub fn HotkeysBar(props: HotkeysBarProps) -> Element {
    rsx! {
        div { class: "hotkeys-bar",
            div { class: "hotkeys-container",
                for hotkey in props.hotkeys.iter() {
                    div { class: "hotkey",
                        kbd { "{hotkey.key}" }
                        span { class: "hotkey-description", "{hotkey.description}" }
                    }
                }
            }
        }
    }
}
