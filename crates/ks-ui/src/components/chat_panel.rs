//! Agent chat panel component.
//!
//! Right-side slide-out panel for conversing with Matrix AI agents.
//! Supports rich rendering: tool calls, markdown, thinking blocks.
//! Drag-to-resize on the left edge (mirroring the sidebar pattern).

use dioxus::prelude::*;
use ks_kube::{
    AgentInfo, ChatClient, ChatMessage, ConversationInfo, CreateAgentInput, MatrixChatClient,
    MessagePart, ToolCallInfo, UpdateAgentInput,
};
use lucide_dioxus::{ArrowDown, ChevronDown, ChevronRight, X};
use pulldown_cmark::{Options, Parser, html};
use std::collections::HashMap;
use std::sync::Arc;

/// Name used to auto-select the KubeStudio agent from the agent list.
const KUBESTUDIO_AGENT_NAME: &str = "kubestudio";

const CHAT_MIN_WIDTH: i32 = 280;
const CHAT_MAX_WIDTH: i32 = 800;
const CHAT_DEFAULT_WIDTH: i32 = 380;

/// Build the default CreateAgentInput for auto-creating a kubestudio persona.
///
/// `tenant_id` is the tenant/realm name (e.g. "non-prod") used to build the
/// connector address pattern `{tenant}.app-kube-studio.*` so the Matrix
/// backend can match registered connector tools to this agent.
fn default_kubestudio_agent_input(tenant_id: &str) -> CreateAgentInput {
    let connector_key = format!("{}.app-kube-studio.*", tenant_id);

    let mut connectors = serde_json::Map::new();
    connectors.insert(
        connector_key,
        serde_json::json!({
            "consent_mode": "auto",
            "enabled": true,
            "tool_configs": {
                "list_clusters": { "consent_mode": "auto", "enabled": true },
                "get_cluster_info": { "consent_mode": "auto", "enabled": true },
                "get_current_context": { "consent_mode": "auto", "enabled": true },
                "get_permissions": { "consent_mode": "auto", "enabled": true },
                "toolbox_deploy": { "consent_mode": "auto", "enabled": true },
                "toolbox_exec": { "consent_mode": "auto", "enabled": true },
                "toolbox_status": { "consent_mode": "auto", "enabled": true },
                "toolbox_delete": { "consent_mode": "auto", "enabled": true }
            }
        }),
    );

    CreateAgentInput {
        name: "kubestudio".to_string(),
        description: Some("KubeStudio cluster management agent".to_string()),
        system_message: Some(
            "You are KubeStudio, an AI assistant for Kubernetes cluster management. \
             Use the available connector tools to inspect and manage the user's cluster."
                .to_string(),
        ),
        agent_greeting: Some("How can I help with your cluster?".to_string()),
        context: Some(serde_json::json!({
            "created_by": "kubestudio-desktop",
            "description": "Auto-created by KubeStudio desktop"
        })),
        tools: Some(serde_json::json!({
            "allow_patterns": [],
            "deny_patterns": [],
            "predefined_names": [],
            "system_tools": {
                "system:document_list": { "consent_mode": "auto", "enabled": true },
                "system:document_read": { "consent_mode": "auto", "enabled": true },
                "system:document_write": { "consent_mode": "auto", "enabled": true },
                "system:echarts_guide": { "consent_mode": "auto", "enabled": true },
                "system:mermaid_guide": { "consent_mode": "auto", "enabled": true },
                "system:validate_echarts": { "consent_mode": "auto", "enabled": true },
                "system:validate_mermaid": { "consent_mode": "auto", "enabled": true },
                "system:validate_react": { "consent_mode": "auto", "enabled": true }
            },
            "mcp_servers": {},
            "connectors": connectors,
            "workflow_tools": {}
        })),
    }
}

/// Props for the ChatPanel component.
#[derive(Props, Clone, PartialEq)]
pub struct ChatPanelProps {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Matrix API URL (e.g. "http://localhost:4000").
    pub api_url: String,
    /// Auth token for Matrix GraphQL calls (signal so effects re-run when it arrives).
    pub auth_token: ReadOnlySignal<String>,
    /// Tenant/realm name (e.g. "non-prod") used when auto-creating the agent
    /// so connector tool patterns resolve correctly.
    pub tenant_id: String,
    /// Callback to close the panel.
    pub on_close: EventHandler<()>,
    /// Optional initial message to auto-send when the panel opens
    /// (used by "Ask Agent" quick actions).
    #[props(default = None)]
    pub initial_message: Option<String>,
    /// Callback when the initial message has been consumed.
    #[props(default)]
    pub on_initial_message_consumed: EventHandler<()>,
}

#[component]
pub fn ChatPanel(props: ChatPanelProps) -> Element {
    // Agents list
    let mut agents = use_signal(Vec::<AgentInfo>::new);
    let mut selected_agent = use_signal(|| None::<AgentInfo>);
    let mut agents_loaded = use_signal(|| false);

    // Conversation state
    let mut conversation_id = use_signal(|| None::<String>);
    let mut messages = use_signal(Vec::<ChatMessage>::new);
    let mut input_text = use_signal(String::new);
    let mut is_sending = use_signal(|| false);
    let mut agent_thinking = use_signal(|| false);
    let mut agent_status_text = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    // Per-agent conversation tracking
    let mut agent_conversations: Signal<HashMap<String, String>> = use_signal(HashMap::new);
    let mut conversation_list: Signal<Vec<ConversationInfo>> = use_signal(Vec::new);
    let mut show_history: Signal<bool> = use_signal(|| false);
    let mut history_loading: Signal<bool> = use_signal(|| false);

    // Tool call expand/collapse state (by tool call id)
    let mut expanded_tools = use_signal(Vec::<String>::new);

    // Auto-scroll state: track if user has scrolled up from the bottom
    let mut user_scrolled_up = use_signal(|| false);
    // Track previous message count to detect new messages (non-reactive to avoid rerender loops)
    let prev_msg_count = use_hook(|| std::cell::Cell::new(0usize));

    // Resize state
    let mut panel_width = use_signal(|| CHAT_DEFAULT_WIDTH);
    let mut is_resizing = use_signal(|| false);

    // Track if we've consumed the initial message
    let initial_consumed = use_hook(|| std::cell::Cell::new(false));

    let api_url = props.api_url.clone();
    let auth_token = props.auth_token;

    // Shared base client — reuses the connection pool across all make_client
    // calls to avoid leaking file descriptors. Created once per component mount.
    let base_client = use_hook({
        let api_url = api_url.clone();
        move || Arc::new(MatrixChatClient::new(api_url))
    });

    // Build client helper — clones the shared client and always reads the
    // latest token from the session store so refreshed tokens take effect.
    let make_client = {
        let base_client = base_client.clone();
        move || -> Arc<MatrixChatClient> {
            // Prefer session store (refreshable) over signal prop
            let session_token = crate::session::get_auth_token();
            let prop_token = auth_token.read();
            let token = if session_token.is_empty() {
                prop_token.as_str()
            } else {
                session_token.as_str()
            };
            let mut c = MatrixChatClient::from_shared(&base_client);
            if !token.is_empty() {
                c.set_auth_token(token.to_string());
            }
            Arc::new(c)
        }
    };

    // Inject chart processor JS (mermaid + echarts CDN + post-processor)
    let chart_init = use_hook(|| std::cell::Cell::new(false));
    if !chart_init.get() {
        chart_init.set(true);
        spawn(async move {
            let _ = document::eval(CHART_PROCESSOR_JS).await;
        });
    }

    // Fetch agents once when we have a token. use_effect re-runs
    // reactively when auth_token signal changes.
    {
        let make_client = make_client.clone();
        let api_url = api_url.clone();
        let tenant_id = props.tenant_id.clone();
        use_effect(move || {
            let auth_token_dep = auth_token.read().clone();
            if auth_token_dep.is_empty() {
                return;
            }
            let make_client = make_client.clone();
            let api_url = api_url.clone();
            let tenant_id = tenant_id.clone();
            let token_len = auth_token_dep.len();
            spawn(async move {
                let client = make_client();
                tracing::info!(
                    "ChatPanel: fetching agents from {} (token length: {})",
                    api_url,
                    token_len
                );
                match client.list_agents().await {
                    Ok(mut list) => {
                        tracing::info!("ChatPanel: loaded {} agents", list.len());
                        let auto = list
                            .iter()
                            .find(|a| a.name.to_lowercase().contains(KUBESTUDIO_AGENT_NAME))
                            .cloned();

                        if let Some(ks_agent) = auto {
                            tracing::info!(
                                "ChatPanel: auto-selected agent: {}, updating tool configs",
                                ks_agent.name
                            );
                            let fresh_input = default_kubestudio_agent_input(&tenant_id);
                            let update_input = UpdateAgentInput {
                                id: ks_agent.id.clone(),
                                tools: fresh_input.tools,
                            };
                            match client.update_agent(update_input).await {
                                Ok(updated) => {
                                    tracing::info!(
                                        "ChatPanel: updated agent tools for {}",
                                        updated.name
                                    );
                                    agents.set(list);
                                    agents_loaded.set(true);
                                    selected_agent.set(Some(updated));
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "ChatPanel: failed to update agent tools: {}, using existing",
                                        e
                                    );
                                    agents.set(list);
                                    agents_loaded.set(true);
                                    selected_agent.set(Some(ks_agent));
                                }
                            }
                        } else {
                            tracing::info!("ChatPanel: no kubestudio agent found, creating one");
                            match client
                                .create_agent(default_kubestudio_agent_input(&tenant_id))
                                .await
                            {
                                Ok(new_agent) => {
                                    tracing::info!("ChatPanel: created agent: {}", new_agent.name);
                                    list.push(new_agent.clone());
                                    agents.set(list);
                                    agents_loaded.set(true);
                                    selected_agent.set(Some(new_agent));
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "ChatPanel: failed to create kubestudio agent: {}",
                                        e
                                    );
                                    agents.set(list);
                                    agents_loaded.set(true);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let err_str = format!("{}", e);
                        if err_str.contains("Not authenticated") || err_str.contains("401") {
                            // User session not ready yet (e.g. StrikeHub proxy has
                            // no user token). Don't retry — the use_effect will
                            // re-run when auth_token signal changes on sign-in.
                            tracing::info!("ChatPanel: not authenticated, waiting for sign-in");
                        } else {
                            tracing::error!("ChatPanel: failed to fetch agents: {}", err_str);
                            error_msg.set(Some(format!("Failed to load agents: {}", err_str)));
                        }
                    }
                }
            });
        });
    }

    // Handle initial_message prop — auto-send when agent is ready
    // Always start a new conversation for Ask Agent flow.
    // All signal writes go inside spawn() to avoid write-in-component-body warnings.
    if props.visible
        && !initial_consumed.get()
        && props.initial_message.is_some()
        && selected_agent.read().is_some()
        && !is_sending()
    {
        initial_consumed.set(true);

        let msg = props.initial_message.clone().unwrap();
        let agent = selected_agent.read().clone().unwrap();
        let client = make_client();
        let on_consumed = props.on_initial_message_consumed;

        spawn(async move {
            // Save current conversation before starting fresh
            if let Some(cid) = conversation_id.read().clone() {
                agent_conversations.write().insert(agent.id.clone(), cid);
            }
            conversation_id.set(None);
            messages.set(Vec::new());
            is_sending.set(true);
            error_msg.set(None);

            on_consumed.call(());
            let conv_id = match client
                .create_conversation(Some(&format!("Chat with {}", agent.name)))
                .await
            {
                Ok(id) => {
                    conversation_id.set(Some(id.clone()));
                    id
                }
                Err(e) => {
                    error_msg.set(Some(format!("Failed to create conversation: {}", e)));
                    is_sending.set(false);
                    return;
                }
            };

            let user_msg = ChatMessage {
                id: format!("local-{}", messages.read().len()),
                sender_type: "USER".to_string(),
                sender_name: "You".to_string(),
                text: msg.clone(),
                parts: vec![MessagePart::Text(msg.clone())],
            };
            messages.write().push(user_msg);

            match client.send_message(&conv_id, &agent.id, &msg).await {
                Ok(_) => {
                    agent_thinking.set(true);
                    agent_status_text.set("Thinking...".to_string());
                    is_sending.set(false);
                    poll_and_update(
                        client,
                        conv_id,
                        conversation_id,
                        messages,
                        agent_thinking,
                        agent_status_text,
                        error_msg,
                    )
                    .await;
                }
                Err(e) => {
                    error_msg.set(Some(format!("Failed to send: {}", e)));
                    is_sending.set(false);
                }
            }
        });
    }

    // Reset consumed flag when initial_message changes to None
    if props.initial_message.is_none() && initial_consumed.get() {
        initial_consumed.set(false);
    }

    // Handler: select agent
    let on_agent_select = {
        let make_client = make_client.clone();
        move |evt: Event<FormData>| {
            let val = evt.value().to_string();
            if val.is_empty() {
                selected_agent.set(None);
                conversation_id.set(None);
                messages.set(Vec::new());
                show_history.set(false);
                return;
            }

            // Save current conversation for the old agent
            if let Some(old_agent) = selected_agent.read().as_ref()
                && let Some(cid) = conversation_id.read().clone()
            {
                agent_conversations
                    .write()
                    .insert(old_agent.id.clone(), cid);
            }

            let agent = agents.read().iter().find(|a| a.id == val).cloned();
            error_msg.set(None);
            show_history.set(false);
            agent_thinking.set(false);
            agent_status_text.set(String::new());

            if let Some(ref ag) = agent {
                // Check if we have a stored conversation for this agent
                let stored_cid = agent_conversations.read().get(&ag.id).cloned();
                if let Some(cid) = stored_cid {
                    conversation_id.set(Some(cid.clone()));
                    let client = make_client();
                    spawn(async move {
                        match client.get_conversation(&cid).await {
                            Ok(state) => {
                                let active = !matches!(
                                    state.agent_status.as_str(),
                                    "IDLE" | "STREAM_END" | "ERROR"
                                );
                                messages.set(state.messages);
                                if active {
                                    agent_thinking.set(true);
                                    agent_status_text.set("Thinking...".to_string());
                                    poll_and_update(
                                        client,
                                        cid,
                                        conversation_id,
                                        messages,
                                        agent_thinking,
                                        agent_status_text,
                                        error_msg,
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to restore conversation: {}", e);
                                conversation_id.set(None);
                                messages.set(Vec::new());
                            }
                        }
                    });
                } else {
                    conversation_id.set(None);
                    messages.set(Vec::new());
                }

                // Fetch conversation list in background
                let client = make_client();
                history_loading.set(true);
                spawn(async move {
                    match client.list_conversations(None).await {
                        Ok(list) => conversation_list.set(list),
                        Err(e) => tracing::warn!("Failed to fetch conversation list: {}", e),
                    }
                    history_loading.set(false);
                });
            } else {
                conversation_id.set(None);
                messages.set(Vec::new());
            }

            selected_agent.set(agent);
        }
    };

    // Handler: send message
    let mut on_send = {
        let make_client = make_client.clone();
        move |_| {
            let text = input_text.read().trim().to_string();
            if text.is_empty() || is_sending() {
                return;
            }
            let Some(agent) = selected_agent.read().clone() else {
                return;
            };

            let client = make_client();
            is_sending.set(true);
            error_msg.set(None);

            spawn(async move {
                let existing_id: Option<String> = conversation_id.read().clone();
                let conv_id: String = match existing_id {
                    Some(id) => id,
                    None => match client
                        .create_conversation(Some(&format!("Chat with {}", agent.name)))
                        .await
                    {
                        Ok(id) => {
                            conversation_id.set(Some(id.clone()));
                            agent_conversations
                                .write()
                                .insert(agent.id.clone(), id.clone());
                            id
                        }
                        Err(e) => {
                            error_msg.set(Some(format!("Failed to create conversation: {}", e)));
                            is_sending.set(false);
                            return;
                        }
                    },
                };

                let user_msg = ChatMessage {
                    id: format!("local-{}", messages.read().len()),
                    sender_type: "USER".to_string(),
                    sender_name: "You".to_string(),
                    text: text.clone(),
                    parts: vec![MessagePart::Text(text.clone())],
                };
                messages.write().push(user_msg);
                input_text.set(String::new());
                // Clear the uncontrolled textarea in the DOM
                let _ = document::eval("document.querySelector('.chat-input').value = ''");
                // Reset scroll state so auto-scroll kicks in for the response
                user_scrolled_up.set(false);

                match client.send_message(&conv_id, &agent.id, &text).await {
                    Ok(_) => {
                        agent_thinking.set(true);
                        agent_status_text.set("Thinking...".to_string());
                        is_sending.set(false);
                        poll_and_update(
                            client,
                            conv_id,
                            conversation_id,
                            messages,
                            agent_thinking,
                            agent_status_text,
                            error_msg,
                        )
                        .await;
                    }
                    Err(e) => {
                        error_msg.set(Some(format!("Failed to send: {}", e)));
                        is_sending.set(false);
                    }
                }
            });
        }
    };

    let mut on_send_clone = on_send.clone();
    let on_keydown = move |evt: Event<KeyboardData>| {
        if evt.key() == Key::Enter && !evt.modifiers().shift() {
            evt.prevent_default();
            on_send_clone(());
        }
    };

    // Resize handlers (mirror sidebar pattern, inverted for right panel)
    // Track the mouse X at drag start so we can compute delta
    let mut drag_start_x = use_signal(|| 0i32);
    let mut drag_start_width = use_signal(|| CHAT_DEFAULT_WIDTH);

    let handle_mousemove = move |evt: MouseEvent| {
        if is_resizing() {
            let mouse_x = evt.client_coordinates().x as i32;
            // Delta: how far left the mouse moved from start = how much wider the panel gets
            let delta = drag_start_x() - mouse_x;
            let new_width = (drag_start_width() + delta).clamp(CHAT_MIN_WIDTH, CHAT_MAX_WIDTH);
            panel_width.set(new_width);
        }
    };

    let handle_mouseup = move |_evt: MouseEvent| {
        if is_resizing() {
            is_resizing.set(false);
        }
    };

    // Auto-scroll to bottom when new messages arrive (unless user scrolled up)
    use_effect(move || {
        let current_count = messages.read().len();
        let thinking = agent_thinking();
        if current_count != prev_msg_count.get() || thinking {
            prev_msg_count.set(current_count);
            if !user_scrolled_up() {
                spawn(async move {
                    let _ = document::eval(
                        r#"
                        const el = document.querySelector('.chat-messages');
                        if (el) { el.scrollTo({ top: el.scrollHeight, behavior: 'instant' }); }
                        "#,
                    )
                    .await;
                });
            }
        }
    });

    if !props.visible {
        return rsx! {};
    }

    let selected_id = selected_agent
        .read()
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_default();

    let user_select = if is_resizing() { "none" } else { "auto" };
    let panel_style = format!("width: {}px; user-select: {};", panel_width(), user_select,);

    rsx! {
        // Resize overlay (captures mouse events globally during drag)
        if is_resizing() {
            div {
                style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; z-index: 9999; cursor: col-resize; user-select: none;",
                onmousemove: handle_mousemove,
                onmouseup: handle_mouseup,
            }
        }

        div {
            class: "chat-panel",
            style: "{panel_style}",
            onkeydown: move |evt: Event<KeyboardData>| {
                evt.stop_propagation();
            },

            // Resize handle (left edge)
            div {
                class: "chat-resize-handle",
                onmousedown: move |evt: MouseEvent| {
                    drag_start_x.set(evt.client_coordinates().x as i32);
                    drag_start_width.set(panel_width());
                    is_resizing.set(true);
                    evt.stop_propagation();
                },
            }

            div { class: "chat-panel-header",
                h3 { "Agent Chat" }
                button {
                    class: "chat-panel-close",
                    onclick: move |_| props.on_close.call(()),
                    X { size: 16 }
                }
            }

            div { class: "chat-agent-selector",
                if !agents_loaded() {
                    div { class: "chat-loading", "Loading agents..." }
                } else {
                    select {
                        class: "chat-agent-select",
                        value: "{selected_id}",
                        onchange: on_agent_select,
                        option { value: "", "Select an agent..." }
                        for agent in agents.read().iter() {
                            option {
                                key: "{agent.id}",
                                value: "{agent.id}",
                                selected: agent.id == selected_id,
                                "{agent.name}"
                            }
                        }
                    }
                }
            }

            // Conversation controls (New + History)
            if selected_agent.read().is_some() && agents_loaded() {
                div {
                    class: if show_history() { "chat-conversation-controls history-open" } else { "chat-conversation-controls" },
                    button {
                        class: "chat-conv-btn",
                        onclick: {
                            let make_client = make_client.clone();
                            move |_| {
                                // Save current conversation before starting fresh
                                if let Some(agent) = selected_agent.read().as_ref()
                                    && let Some(cid) = conversation_id.read().clone()
                                {
                                    agent_conversations.write().insert(agent.id.clone(), cid);
                                }
                                conversation_id.set(None);
                                messages.set(Vec::new());
                                if let Some(agent) = selected_agent.read().as_ref() {
                                    agent_conversations.write().remove(&agent.id);
                                }
                                show_history.set(false);
                                error_msg.set(None);

                                // Refresh conversation list in background
                                {
                                    let client = make_client();
                                    history_loading.set(true);
                                    spawn(async move {
                                        match client.list_conversations(None).await {
                                            Ok(list) => conversation_list.set(list),
                                            Err(e) => tracing::warn!("Failed to refresh conversation list: {}", e),
                                        }
                                        history_loading.set(false);
                                    });
                                }
                            }
                        },
                        "+ New"
                    }
                    button {
                        class: if show_history() { "chat-conv-btn chat-conv-btn-active" } else { "chat-conv-btn" },
                        onclick: {
                            let make_client = make_client.clone();
                            move |_| {
                                let opening = !show_history();
                                show_history.set(opening);
                                if opening {
                                    // Fetch fresh list when opening
                                    let client = make_client();
                                    history_loading.set(true);
                                    spawn(async move {
                                        match client.list_conversations(None).await {
                                            Ok(list) => conversation_list.set(list),
                                            Err(e) => tracing::warn!("Failed to fetch conversation list: {}", e),
                                        }
                                        history_loading.set(false);
                                    });
                                }
                            }
                        },
                        "History"
                    }
                }

                // Conversation history dropdown
                if show_history() {
                    div { class: "chat-history-dropdown",
                        if history_loading() {
                            div { class: "chat-history-loading", "Loading..." }
                        } else if conversation_list.read().is_empty() {
                            div { class: "chat-history-empty", "No past conversations" }
                        } else {
                            for conv in conversation_list.read().iter() {
                                {
                                    let conv_id_val = conv.id.clone();
                                    let conv_title = if conv.title.is_empty() {
                                        "Untitled".to_string()
                                    } else if conv.title.len() > 40 {
                                        format!("{}...", &conv.title[..37])
                                    } else {
                                        conv.title.clone()
                                    };
                                    let is_active = conversation_id
                                        .read()
                                        .as_ref()
                                        .map(|c| c == &conv_id_val)
                                        .unwrap_or(false);
                                    let item_class = if is_active {
                                        "chat-history-item active"
                                    } else {
                                        "chat-history-item"
                                    };
                                    let time_str = format_relative_time(&conv.updated_at);
                                    let cid = conv_id_val.clone();
                                    let make_client2 = make_client.clone();
                                    rsx! {
                                        div {
                                            key: "{conv_id_val}",
                                            class: "{item_class}",
                                            onclick: move |_| {
                                                let cid = cid.clone();
                                                conversation_id.set(Some(cid.clone()));
                                                // Store in agent_conversations map
                                                if let Some(agent) = selected_agent.read().as_ref() {
                                                    agent_conversations
                                                        .write()
                                                        .insert(agent.id.clone(), cid.clone());
                                                }
                                                show_history.set(false);
                                                // Clear stale state so the loaded conversation
                                                // starts fresh — thinking/error will be set again
                                                // if the conversation is still active.
                                                agent_thinking.set(false);
                                                agent_status_text.set(String::new());
                                                error_msg.set(None);
                                                user_scrolled_up.set(false);
                                                // Load conversation messages
                                                let client = make_client2();
                                                spawn(async move {
                                                    match client.get_conversation(&cid).await {
                                                        Ok(state) => {
                                                            messages.set(state.messages);
                                                            // Scroll to bottom after loading
                                                            let _ = document::eval(
                                                                r#"
                                                                requestAnimationFrame(function() {
                                                                    const el = document.querySelector('.chat-messages');
                                                                    if (el) { el.scrollTo({ top: el.scrollHeight, behavior: 'instant' }); }
                                                                });
                                                                "#,
                                                            ).await;
                                                            // Resume polling if agent is still active
                                                            let active = !matches!(
                                                                state.agent_status.as_str(),
                                                                "IDLE" | "STREAM_END" | "ERROR"
                                                            );
                                                            if active {
                                                                agent_thinking.set(true);
                                                                agent_status_text.set("Thinking...".to_string());
                                                                poll_and_update(
                                                                    client, cid, conversation_id, messages, agent_thinking,
                                                                    agent_status_text, error_msg,
                                                                )
                                                                .await;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error_msg.set(Some(format!(
                                                                "Failed to load conversation: {}",
                                                                e
                                                            )));
                                                        }
                                                    }
                                                });
                                            },
                                            span { class: "chat-history-title", "{conv_title}" }
                                            span { class: "chat-history-time", "{time_str}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(err) = error_msg.read().as_ref() {
                div { class: "chat-error", "{err}" }
            }

            div { class: "chat-messages-wrapper",
                div {
                    class: "chat-messages",
                    onscroll: move |_| {
                        // Check scroll position via JS to determine if user scrolled up.
                        // Use a comfortable threshold (80px) so the user doesn't get "stuck"
                        // near the bottom, and only update the signal when the state actually
                        // changes to avoid unnecessary re-renders / scroll fighting.
                        let current = user_scrolled_up();
                        spawn(async move {
                            if let Ok(val) = document::eval(
                                r#"
                                const el = document.querySelector('.chat-messages');
                                if (el) {
                                    const atBottom = (el.scrollHeight - el.scrollTop - el.clientHeight) < 80;
                                    return atBottom ? 'bottom' : 'up';
                                }
                                return 'bottom';
                                "#,
                            )
                            .await
                            {
                                let is_at_bottom = val.as_str() == Some("bottom");
                                let should_be_scrolled_up = !is_at_bottom;
                                // Only write the signal when state actually changes
                                if current != should_be_scrolled_up {
                                    user_scrolled_up.set(should_be_scrolled_up);
                                }
                            }
                        });
                    },

                    if messages.read().is_empty() && selected_agent.read().is_some() {
                        div { class: "chat-empty",
                            if let Some(agent) = selected_agent.read().as_ref() {
                                if let Some(greeting) = &agent.greeting {
                                    p { class: "chat-greeting", "{greeting}" }
                                } else {
                                    p { class: "chat-greeting", "Start a conversation with {agent.name}" }
                                }
                            }
                        }
                    } else if selected_agent.read().is_none() {
                        div { class: "chat-empty",
                            p { "Select an agent to begin" }
                        }
                    }

                    for msg in messages.read().iter() {
                        {render_message(msg, &mut expanded_tools)}
                    }

                    if agent_thinking() {
                        div { class: "chat-bubble chat-bubble-agent chat-thinking",
                            div { class: "chat-bubble-sender",
                                if let Some(agent) = selected_agent.read().as_ref() {
                                    "{agent.name}"
                                }
                            }
                            div { class: "chat-thinking-status",
                                if !agent_status_text.read().is_empty() {
                                    span { class: "chat-status-label", "{agent_status_text}" }
                                }
                                div { class: "chat-thinking-dots",
                                    span { "." }
                                    span { "." }
                                    span { "." }
                                }
                            }
                        }
                    }
                }

                // Scroll-to-bottom button (shown when user has scrolled up)
                if user_scrolled_up() && !messages.read().is_empty() {
                    button {
                        class: "chat-scroll-to-bottom",
                        title: "Scroll to bottom",
                        onclick: move |_| {
                            user_scrolled_up.set(false);
                            spawn(async move {
                                let _ = document::eval(
                                    r#"
                                    const el = document.querySelector('.chat-messages');
                                    if (el) { el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' }); }
                                    "#,
                                )
                                .await;
                            });
                        },
                        ArrowDown { size: 18 }
                    }
                }
            }

            if selected_agent.read().is_some() {
                div { class: "chat-input-area",
                    textarea {
                        class: "chat-input",
                        rows: "3",
                        placeholder: if is_sending() || agent_thinking() { "Waiting for response..." } else { "Type a message... (Enter to send, Shift+Enter for newline)" },
                        disabled: is_sending() || agent_thinking(),
                        // Uncontrolled: browser owns the DOM value to avoid liveview
                        // round-trip latency eating keystrokes. We sync via eval when
                        // clearing programmatically (on send).
                        oninput: move |evt| input_text.set(evt.value().to_string()),
                        onkeydown: on_keydown,
                    }
                    button {
                        class: "chat-send-btn",
                        disabled: is_sending() || agent_thinking() || input_text.read().trim().is_empty(),
                        onclick: move |_| on_send(()),
                        "Send"
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Polling helper with live status updates
// ---------------------------------------------------------------------------

async fn poll_and_update(
    client: Arc<MatrixChatClient>,
    conv_id: String,
    active_conversation_id: Signal<Option<String>>,
    mut messages: Signal<Vec<ChatMessage>>,
    mut agent_thinking: Signal<bool>,
    mut agent_status_text: Signal<String>,
    mut error_msg: Signal<Option<String>>,
) {
    let poll_interval_ms = 800u64;
    let max_polls = 150u32;

    /// Check if the UI is currently showing this conversation.
    fn is_active(active: &Signal<Option<String>>, conv_id: &str) -> bool {
        active
            .peek()
            .as_ref()
            .map(|c| c.as_str() == conv_id)
            .unwrap_or(false)
    }

    for _attempt in 0..max_polls {
        match client.get_conversation(&conv_id).await {
            Ok(state) => {
                let done = matches!(state.agent_status.as_str(), "IDLE" | "STREAM_END" | "ERROR");
                let has_agent_msg = state
                    .messages
                    .iter()
                    .any(|m| m.sender_type != "USER" && !m.text.is_empty());

                // Only update UI if this conversation is currently displayed
                if is_active(&active_conversation_id, &conv_id) {
                    let status_label = match state.agent_status.as_str() {
                        "STREAMING" => "Responding...",
                        "EXECUTING_TOOLS" => "Running tools...",
                        "AWAITING_CONSENT" => "Waiting for approval...",
                        "AWAITING_CLIENT_TOOLS" => "Running client tools...",
                        _ => "Thinking...",
                    };
                    agent_status_text.set(status_label.to_string());

                    if !state.messages.is_empty() {
                        messages.set(state.messages.clone());
                    }

                    if done && has_agent_msg {
                        messages.set(state.messages);
                        agent_thinking.set(false);
                        agent_status_text.set(String::new());
                        return;
                    }
                } else if done && has_agent_msg {
                    // Conversation finished while user was viewing another one.
                    // Don't touch UI — it belongs to the other conversation now.
                    return;
                }
            }
            Err(e) => {
                if is_active(&active_conversation_id, &conv_id) {
                    error_msg.set(Some(format!("Failed to get response: {}", e)));
                    agent_thinking.set(false);
                    agent_status_text.set(String::new());
                }
                return;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
    }

    // Final poll after timeout
    if is_active(&active_conversation_id, &conv_id) {
        match client.get_conversation(&conv_id).await {
            Ok(state) => messages.set(state.messages),
            Err(e) => error_msg.set(Some(format!("Polling timed out: {}", e))),
        }
        agent_thinking.set(false);
        agent_status_text.set(String::new());
    }
}

// ---------------------------------------------------------------------------
// Message rendering with rich parts
// ---------------------------------------------------------------------------

fn render_message(msg: &ChatMessage, expanded_tools: &mut Signal<Vec<String>>) -> Element {
    let is_user = msg.sender_type == "USER";
    let bubble_class = if is_user {
        "chat-bubble chat-bubble-user"
    } else {
        "chat-bubble chat-bubble-agent"
    };
    let sender = if is_user {
        "You".to_string()
    } else {
        msg.sender_name.clone()
    };
    let msg_id = msg.id.clone();

    if msg.parts.is_empty() {
        let html = render_markdown(&msg.text);
        return rsx! {
            div {
                key: "{msg_id}",
                class: "{bubble_class}",
                div { class: "chat-bubble-sender", "{sender}" }
                div {
                    class: "chat-bubble-text chat-markdown",
                    dangerous_inner_html: "{html}",
                    onmounted: move |_| {
                        spawn(async move {
                            let _ = document::eval("requestAnimationFrame(function() { setTimeout(function() { if (typeof window.__processChatCharts === 'function') window.__processChatCharts(); }, 50); });").await;
                        });
                    },
                }
            }
        };
    }

    rsx! {
        div {
            key: "{msg_id}",
            class: "{bubble_class}",
            div { class: "chat-bubble-sender", "{sender}" }
            for part in msg.parts.iter() {
                {match part {
                    MessagePart::Text(text) => {
                        let html = render_markdown(text);
                        rsx! {
                            div {
                                class: "chat-bubble-text chat-markdown",
                                dangerous_inner_html: "{html}",
                                onmounted: move |_| {
                                    spawn(async move {
                                        let _ = document::eval("requestAnimationFrame(function() { setTimeout(function() { if (typeof window.__processChatCharts === 'function') window.__processChatCharts(); }, 50); });").await;
                                    });
                                },
                            }
                        }
                    }
                    MessagePart::Thinking(text) => {
                        let text = text.clone();
                        rsx! {
                            div { class: "chat-thinking-block",
                                div { class: "chat-thinking-label", "Thinking" }
                                div { class: "chat-thinking-content", "{text}" }
                            }
                        }
                    }
                    MessagePart::ToolCall(tc) => {
                        render_tool_call(tc, expanded_tools)
                    }
                }}
            }
        }
    }
}

fn render_tool_call(tc: &ToolCallInfo, expanded_tools: &mut Signal<Vec<String>>) -> Element {
    let is_expanded = expanded_tools.read().contains(&tc.id);
    let tc_id_toggle = tc.id.clone();
    let name = tc.name.clone();
    let status = tc.status.clone();
    let args = tc.arguments.clone();
    let result = tc.result.clone();
    let error = tc.error.clone();

    let status_class = match status.as_str() {
        "completed" | "success" => "tool-status-success",
        "error" | "failed" => "tool-status-error",
        _ => "tool-status-pending",
    };

    rsx! {
        div { class: "chat-tool-call",
            div {
                class: "chat-tool-header",
                onclick: {
                    let mut expanded = *expanded_tools;
                    move |_| {
                        let mut list = expanded.write();
                        if let Some(pos) = list.iter().position(|id| id == &tc_id_toggle) {
                            list.remove(pos);
                        } else {
                            list.push(tc_id_toggle.clone());
                        }
                    }
                },
                span { class: "chat-tool-icon",
                    if is_expanded {
                        ChevronDown { size: 14 }
                    } else {
                        ChevronRight { size: 14 }
                    }
                }
                span { class: "chat-tool-name", "{name}" }
                span { class: "chat-tool-status {status_class}", "{status}" }
            }
            if is_expanded {
                div { class: "chat-tool-details",
                    if let Some(ref args_str) = args {
                        div { class: "chat-tool-section",
                            div { class: "chat-tool-section-label", "Arguments" }
                            pre { class: "chat-tool-code", "{args_str}" }
                        }
                    }
                    if let Some(ref result_str) = result {
                        div { class: "chat-tool-section",
                            div { class: "chat-tool-section-label", "Result" }
                            pre { class: "chat-tool-code", "{result_str}" }
                        }
                    }
                    if let Some(ref err_str) = error {
                        div { class: "chat-tool-section chat-tool-error",
                            div { class: "chat-tool-section-label", "Error" }
                            pre { class: "chat-tool-code", "{err_str}" }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Markdown rendering (pulldown-cmark) + chart post-processing
// ---------------------------------------------------------------------------

/// Convert markdown text to HTML using pulldown-cmark.
/// Produces proper `<pre><code class="language-X">` blocks for fenced code,
/// which the JS chart processor can then pick up for mermaid/echarts.
fn render_markdown(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(input, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// JS snippet that loads mermaid + echarts CDN scripts and defines
/// `window.__processChatCharts()` to post-process code blocks.
/// Injected once on first load.
const CHART_PROCESSOR_JS: &str = r#"
(function() {
    if (window.__chatChartsInit) return;
    window.__chatChartsInit = true;

    // Load Mermaid
    if (!window.mermaid) {
        var ms = document.createElement('script');
        ms.src = 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js';
        ms.onload = function() {
            window.mermaid.initialize({ startOnLoad: false, theme: 'dark' });
            console.log('[KubeStudio] Mermaid loaded');
        };
        document.head.appendChild(ms);
    }

    // Load ECharts
    if (!window.echarts) {
        var es = document.createElement('script');
        es.src = 'https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js';
        es.onload = function() { console.log('[KubeStudio] ECharts loaded'); };
        document.head.appendChild(es);
    }

    // Chart processor: finds unprocessed code blocks and renders them
    window.__processChatCharts = function() {
        var container = document.querySelector('.chat-messages');
        if (!container) return;

        // Mermaid
        if (window.mermaid) {
            var blocks = container.querySelectorAll('pre code.language-mermaid:not([data-processed])');
            blocks.forEach(function(block, idx) {
                block.setAttribute('data-processed', 'true');
                var pre = block.closest('pre') || block;
                var code = block.textContent || block.innerText;
                var div = document.createElement('div');
                div.className = 'chat-viz-block';
                div.id = 'chat-mermaid-' + Date.now() + '-' + idx;
                div.style.cssText = 'background:rgba(0,0,0,0.3);border-radius:6px;padding:12px;margin:8px 0;overflow:auto;width:100%;box-sizing:border-box;';
                try {
                    window.mermaid.render(div.id + '-svg', code).then(function(result) {
                        div.innerHTML = result.svg;
                        var svg = div.querySelector('svg');
                        if (svg) { svg.style.display='block'; svg.style.width='100%'; svg.style.height='auto'; svg.style.minHeight='80px'; }
                    }).catch(function(err) {
                        div.innerHTML = '<div style="color:var(--destructive);font-size:0.75rem;">Mermaid error: ' + err.message + '</div>';
                    });
                } catch(e) {
                    div.innerHTML = '<div style="color:var(--destructive);font-size:0.75rem;">Mermaid error: ' + e.message + '</div>';
                }
                pre.parentNode.replaceChild(div, pre);
            });
        }

        // ECharts
        if (window.echarts) {
            var eblocks = container.querySelectorAll('pre code.language-echarts:not([data-processed]), pre code.language-echart:not([data-processed])');
            eblocks.forEach(function(block, idx) {
                block.setAttribute('data-processed', 'true');
                var pre = block.closest('pre') || block;
                var code = block.textContent || block.innerText;
                var div = document.createElement('div');
                div.className = 'chat-viz-block chat-echarts-block';
                div.style.cssText = 'width:100%;min-height:180px;height:220px;background:rgba(0,0,0,0.3);border-radius:6px;margin:8px 0;box-sizing:border-box;';
                try {
                    var option = JSON.parse(code);
                    pre.parentNode.replaceChild(div, pre);
                    setTimeout(function() {
                        var chart = window.echarts.init(div, 'dark');
                        option.backgroundColor = option.backgroundColor || 'transparent';
                        if (!option.textStyle) option.textStyle = {};
                        option.textStyle.color = option.textStyle.color || getComputedStyle(document.documentElement).getPropertyValue('--foreground').trim();
                        chart.setOption(option);
                        var ro = new ResizeObserver(function() { chart.resize(); });
                        ro.observe(div);
                        var panel = document.querySelector('.chat-panel');
                        if (panel) { var po = new ResizeObserver(function() { chart.resize(); }); po.observe(panel); }
                    }, 10);
                } catch(e) {
                    div.style.height = 'auto';
                    div.style.padding = '8px';
                    div.innerHTML = '<div style="color:var(--destructive);font-size:0.75rem;">ECharts error: ' + e.message + '</div>';
                    pre.parentNode.replaceChild(div, pre);
                }
            });
        }
    };
})();
"#;

/// Format an ISO 8601 timestamp as a relative time string (e.g. "2m ago").
fn format_relative_time(iso: &str) -> String {
    // Parse ISO 8601 basic: "2025-01-15T10:30:00Z" or with offset
    // Use a simple approach — parse with chrono-less method
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Try to parse the ISO string manually
    let ts = parse_iso_timestamp(iso).unwrap_or(now);
    if ts >= now {
        return "now".to_string();
    }
    let diff = now - ts;
    if diff < 60 {
        return format!("{}s ago", diff);
    }
    let mins = diff / 60;
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = hours / 24;
    format!("{}d ago", days)
}

/// Parse a basic ISO 8601 timestamp to Unix seconds.
fn parse_iso_timestamp(iso: &str) -> Option<u64> {
    // Expected: "2025-01-15T10:30:00Z" or "2025-01-15T10:30:00.000Z"
    let s = iso.trim().replace('Z', "+00:00");
    let s = if s.contains('+') || s.rfind('-').map(|i| i > 10).unwrap_or(false) {
        s
    } else {
        format!("{}+00:00", s)
    };

    // Simple parse: extract date and time parts
    let dt_part = s.split('+').next().unwrap_or(&s);
    let dt_part = dt_part.split('-').collect::<Vec<_>>();
    if dt_part.len() < 3 {
        return None;
    }

    // Re-parse properly
    let clean = iso.trim();
    let date_time: Vec<&str> = clean.split('T').collect();
    if date_time.len() != 2 {
        return None;
    }
    let date_parts: Vec<&str> = date_time[0].split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let year: u64 = date_parts[0].parse().ok()?;
    let month: u64 = date_parts[1].parse().ok()?;
    let day: u64 = date_parts[2].parse().ok()?;

    let time_str = date_time[1]
        .trim_end_matches('Z')
        .split('+')
        .next()
        .unwrap_or("")
        .split('-')
        .next()
        .unwrap_or("");
    let time_parts: Vec<&str> = time_str.split(':').collect();
    if time_parts.len() < 2 {
        return None;
    }
    let hour: u64 = time_parts[0].parse().ok()?;
    let min: u64 = time_parts[1].parse().ok()?;
    let sec: u64 = time_parts
        .get(2)
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Approximate Unix timestamp (ignoring leap seconds, timezone offsets for simplicity)
    let mut days_total: u64 = 0;
    for y in 1970..year {
        days_total += if is_leap_year(y) { 366 } else { 365 };
    }
    let month_days = [
        31,
        28 + if is_leap_year(year) { 1 } else { 0 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for m in 0..(month.saturating_sub(1) as usize) {
        days_total += month_days.get(m).copied().unwrap_or(30);
    }
    days_total += day.saturating_sub(1);

    Some(days_total * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap_year(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}
