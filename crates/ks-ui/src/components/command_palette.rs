use dioxus::prelude::*;

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
    let mut selected_index = use_signal(|| 0usize);

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

    let count = filtered_commands.len();

    rsx! {
        div {
            class: "command-palette-overlay",
            tabindex: 0,
            onmounted: move |e| {
                let data = e.data();
                spawn(async move {
                    let _ = data.set_focus(true).await;
                });
            },
            onclick: move |_| props.on_close.call(()),
            onkeydown: move |e| {
                match e.key() {
                    Key::ArrowDown => {
                        let cur = *selected_index.read();
                        if count > 0 {
                            selected_index.set((cur + 1) % count);
                        }
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    Key::ArrowUp => {
                        let cur = *selected_index.read();
                        if count > 0 {
                            selected_index.set(if cur == 0 { count - 1 } else { cur - 1 });
                        }
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    Key::Enter => {
                        let idx = *selected_index.read();
                        let filtered: Vec<&Command> = props
                            .commands
                            .iter()
                            .filter(|cmd| {
                                cmd.label
                                    .to_lowercase()
                                    .contains(&search.read().to_lowercase())
                            })
                            .collect();
                        if let Some(cmd) = filtered.get(idx) {
                            props.on_select.call(cmd.id.clone());
                            props.on_close.call(());
                        }
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    Key::Escape => {
                        props.on_close.call(());
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    _ => {}
                }
            },
            div {
                class: "command-palette",
                onclick: move |e| e.stop_propagation(),
                input {
                    class: "command-search",
                    placeholder: "Type a command...",
                    oninput: move |e| {
                        search.set(e.value().clone());
                        selected_index.set(0);
                    },
                    onmounted: move |e| {
                        let data = e.data();
                        spawn(async move {
                            let _ = data.set_focus(true).await;
                        });
                    },
                }
                ul { class: "command-list",
                    for (i, cmd) in filtered_commands.iter().enumerate() {
                        li {
                            class: if i == *selected_index.read() { "command-item command-item-selected" } else { "command-item" },
                            onclick: {
                                let id = cmd.id.clone();
                                move |_| {
                                    props.on_select.call(id.clone());
                                    props.on_close.call(());
                                }
                            },
                            onmouseenter: move |_| {
                                selected_index.set(i);
                            },
                            span { class: "command-label", "{cmd.label}" }
                            if let Some(shortcut) = &cmd.shortcut {
                                span { class: "command-shortcut", "{shortcut}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
