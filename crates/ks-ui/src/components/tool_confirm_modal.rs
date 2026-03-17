use dioxus::prelude::*;
use std::collections::HashMap;

/// Per-tool consent decision from user.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolConsentChoice {
    AllowOnce,
    AlwaysAllow,
    Deny,
}

/// Batch decision result: per-tool-call-id decisions.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsentResult {
    pub decisions: HashMap<String, ToolConsentChoice>,
}

/// Info about a tool call awaiting consent.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Option<String>,
}

#[derive(Props, Clone, PartialEq)]
pub struct ToolConsentBarProps {
    pub pending_tools: Vec<PendingToolCall>,
    pub on_decide: EventHandler<ConsentResult>,
}

/// Extract just the command for toolbox_exec, otherwise pretty-print.
fn format_arguments(args: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(args) {
        if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
            return cmd.to_string();
        }
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| args.to_string())
    } else {
        args.to_string()
    }
}

/// Inline consent bar rendered within the chat message stream.
#[component]
pub fn ToolConsentBar(props: ToolConsentBarProps) -> Element {
    if props.pending_tools.is_empty() {
        return rsx! {};
    }

    // Per-tool selection: 0=Deny, 1=Allow, 2=Always. Default: Allow (1).
    let mut selections = use_hook(|| Signal::new(HashMap::<String, usize>::new()));

    // Init defaults for new tool calls (in effect to avoid signal write in body).
    {
        let ids: Vec<String> = props
            .pending_tools
            .iter()
            .map(|t| t.tool_call_id.clone())
            .collect();
        use_effect(move || {
            let mut sel = selections.write();
            for id in &ids {
                sel.entry(id.clone()).or_insert(1);
            }
        });
    }

    let tool_ids: Vec<String> = props
        .pending_tools
        .iter()
        .map(|t| t.tool_call_id.clone())
        .collect();
    let tool_count = props.pending_tools.len();
    let single = tool_count == 1;

    // Helper: build decisions from current selections
    let ids_for_build = tool_ids.clone();
    let build_decisions =
        move |sel: &HashMap<String, usize>| -> HashMap<String, ToolConsentChoice> {
            ids_for_build
                .iter()
                .map(|id| {
                    let choice = match sel.get(id).copied().unwrap_or(1) {
                        0 => ToolConsentChoice::Deny,
                        2 => ToolConsentChoice::AlwaysAllow,
                        _ => ToolConsentChoice::AllowOnce,
                    };
                    (id.clone(), choice)
                })
                .collect()
        };

    rsx! {
        div {
            class: "chat-consent-bar",
            tabindex: 0,
            onkeydown: {
                let ids_esc = tool_ids.clone();
                move |e: KeyboardEvent| {
                    if crate::utils::is_escape(&e) {
                        let decisions = ids_esc.iter().map(|id| (id.clone(), ToolConsentChoice::Deny)).collect();
                        props.on_decide.call(ConsentResult { decisions });
                        e.stop_propagation();
                    }
                }
            },
            onmounted: move |e| {
                spawn(async move {
                    let _ = e.data().set_focus(true).await;
                });
            },

            div { class: "consent-header",
                span { class: "consent-title", "Approval required" }
            }

            for tc in props.pending_tools.iter() {
                {
                    let tc_id_0 = tc.tool_call_id.clone();
                    let tc_id_1 = tc.tool_call_id.clone();
                    let tc_id_2 = tc.tool_call_id.clone();
                    let sel_val = selections.read().get(&tc.tool_call_id).copied().unwrap_or(1);

                    rsx! {
                        div { class: "consent-tool-item",
                            div { class: "consent-tool-name", "{tc.tool_name}" }
                            if let Some(ref args) = tc.arguments {
                                div { class: "consent-tool-args", "{format_arguments(args)}" }
                            }
                            if !single {
                                div { class: "consent-tool-toggle",
                                    button {
                                        class: if sel_val == 0 { "consent-chip active-deny" } else { "consent-chip" },
                                        onclick: move |_| { selections.write().insert(tc_id_0.clone(), 0); },
                                        "Deny"
                                    }
                                    button {
                                        class: if sel_val == 1 { "consent-chip active-allow" } else { "consent-chip" },
                                        onclick: move |_| { selections.write().insert(tc_id_1.clone(), 1); },
                                        "Allow"
                                    }
                                    button {
                                        class: if sel_val == 2 { "consent-chip active-always" } else { "consent-chip" },
                                        onclick: move |_| { selections.write().insert(tc_id_2.clone(), 2); },
                                        "Always"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "consent-actions",
                if single {
                    {
                        let ids_deny = tool_ids.clone();
                        let ids_allow = tool_ids.clone();
                        let ids_always = tool_ids.clone();
                        rsx! {
                            button {
                                class: "consent-btn consent-deny",
                                onclick: move |_| {
                                    let decisions = ids_deny.iter().map(|id| (id.clone(), ToolConsentChoice::Deny)).collect();
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Deny"
                            }
                            button {
                                class: "consent-btn consent-allow",
                                onclick: move |_| {
                                    let decisions = ids_allow.iter().map(|id| (id.clone(), ToolConsentChoice::AllowOnce)).collect();
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Allow"
                            }
                            button {
                                class: "consent-btn consent-always",
                                onclick: move |_| {
                                    let decisions = ids_always.iter().map(|id| (id.clone(), ToolConsentChoice::AlwaysAllow)).collect();
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Always"
                            }
                        }
                    }
                } else {
                    {
                        let ids_deny = tool_ids.clone();
                        let ids_allow = tool_ids.clone();
                        rsx! {
                            button {
                                class: "consent-btn consent-deny",
                                onclick: move |_| {
                                    let decisions = ids_deny.iter().map(|id| (id.clone(), ToolConsentChoice::Deny)).collect();
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Deny All"
                            }
                            button {
                                class: "consent-btn consent-allow",
                                onclick: move |_| {
                                    let decisions = ids_allow.iter().map(|id| (id.clone(), ToolConsentChoice::AllowOnce)).collect();
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Allow All"
                            }
                            button {
                                class: "consent-btn consent-confirm",
                                onclick: move |_| {
                                    let sel = selections.read();
                                    let decisions = build_decisions(&sel);
                                    props.on_decide.call(ConsentResult { decisions });
                                },
                                "Confirm"
                            }
                        }
                    }
                }
            }
        }
    }
}
