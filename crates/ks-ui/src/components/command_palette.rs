use dioxus::prelude::*;
use lucide_dioxus::Lightbulb;

#[derive(Clone, PartialEq)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
}

#[derive(Props, Clone, PartialEq)]
pub struct CommandPaletteProps {
    pub open: bool,
    pub commands: Vec<Command>,
    pub on_close: EventHandler<()>,
    pub on_select: EventHandler<String>,
}

#[component]
pub fn CommandPalette(props: CommandPaletteProps) -> Element {
    let mut search = use_signal(String::new);

    let filtered_commands: Vec<&Command> = props
        .commands
        .iter()
        .filter(|cmd| {
            cmd.label
                .to_lowercase()
                .contains(&search.read().to_lowercase())
        })
        .collect();

    if !props.open {
        return rsx! {};
    }

    rsx! {
        div { class: "command-palette-overlay",
            onclick: move |_| props.on_close.call(()),
            div { class: "command-palette",
                onclick: move |e| e.stop_propagation(),
                input {
                    class: "command-search",
                    placeholder: "Type a command...",
                    oninput: move |e| search.set(e.value().clone()),
                    autofocus: true,
                }
                ul { class: "command-list",
                    for cmd in filtered_commands {
                        li {
                            class: "command-item",
                            onclick: {
                                let id = cmd.id.clone();
                                move |_| {
                                    props.on_select.call(id.clone());
                                    props.on_close.call(());
                                }
                            },
                            span { class: "command-label", "{cmd.label}" }
                            if let Some(shortcut) = &cmd.shortcut {
                                span { class: "command-shortcut", "{shortcut}" }
                            }
                        }
                    }
                }
                div { class: "command-palette-hint",
                    Lightbulb { size: 14 }
                    " Tip: Press the shortcut key directly anywhere in the app (no menu needed!)"
                }
            }
        }
    }
}
