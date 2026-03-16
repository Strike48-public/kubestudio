//! Matrix GraphQL client for agent chat integration.
//!
//! Provides a trait-based abstraction (`ChatClient`) so the backend
//! can be swapped without touching UI code.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Minimal agent info surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub greeting: Option<String>,
}

/// A tool call attached to a message.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub status: String,
}

/// A single part of a chat message (text, tool call, or thinking).
#[derive(Debug, Clone)]
pub enum MessagePart {
    Text(String),
    ToolCall(ToolCallInfo),
    Thinking(String),
}

/// A single chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    /// "USER" or "AGENT" (or other profile types the API returns).
    pub sender_type: String,
    pub sender_name: String,
    pub text: String,
    /// Rich parts (text, tool calls, thinking blocks).
    pub parts: Vec<MessagePart>,
}

/// Result of polling a conversation.
#[derive(Debug, Clone)]
pub struct ConversationState {
    pub messages: Vec<ChatMessage>,
    /// Uppercased agent status: IDLE, PROCESSING, STREAM_END, ERROR, etc.
    pub agent_status: String,
}

/// Lightweight conversation summary for the history list.
#[derive(Debug, Clone)]
pub struct ConversationInfo {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Input for creating an agent persona via the Matrix API.
///
/// The `context` and `tools` fields hold raw JSON values.  They are
/// serialized as JSON **strings** for the GraphQL `Json` scalar when
/// sent to the API (see `CreateAgentInput::to_gql_variables`).
#[derive(Debug, Clone)]
pub struct CreateAgentInput {
    pub name: String,
    pub description: Option<String>,
    pub system_message: Option<String>,
    pub agent_greeting: Option<String>,
    pub context: Option<serde_json::Value>,
    pub tools: Option<serde_json::Value>,
}

impl CreateAgentInput {
    /// Convert to a `serde_json::Value` suitable for GraphQL variables.
    /// Json scalar fields (`context`, `tools`) are stringified.
    fn to_gql_variables(&self) -> serde_json::Value {
        let mut input = serde_json::Map::new();
        input.insert("name".into(), serde_json::json!(self.name));
        if let Some(ref d) = self.description {
            input.insert("description".into(), serde_json::json!(d));
        }
        if let Some(ref s) = self.system_message {
            input.insert("systemMessage".into(), serde_json::json!(s));
        }
        if let Some(ref g) = self.agent_greeting {
            input.insert("agentGreeting".into(), serde_json::json!(g));
        }
        if let Some(ref c) = self.context {
            input.insert("context".into(), serde_json::json!(c.to_string()));
        }
        if let Some(ref t) = self.tools {
            input.insert("tools".into(), serde_json::json!(t.to_string()));
        }
        serde_json::json!({ "input": input })
    }
}

/// Input for updating an existing agent's configuration via the Matrix API.
#[derive(Debug, Clone)]
pub struct UpdateAgentInput {
    pub id: String,
    pub tools: Option<serde_json::Value>,
}

impl UpdateAgentInput {
    fn to_gql_variables(&self) -> serde_json::Value {
        let mut input = serde_json::Map::new();
        input.insert("id".into(), serde_json::json!(self.id));
        if let Some(ref t) = self.tools {
            input.insert("tools".into(), serde_json::json!(t.to_string()));
        }
        serde_json::json!({ "input": input })
    }
}

/// Abstraction over the chat backend. Implement this trait to swap Matrix
/// for another provider without touching the UI layer.
#[async_trait]
pub trait ChatClient: Send + Sync {
    async fn list_agents(&self) -> anyhow::Result<Vec<AgentInfo>>;
    async fn find_agent_by_name(&self, name: &str) -> anyhow::Result<Option<AgentInfo>>;
    async fn create_agent(&self, input: CreateAgentInput) -> anyhow::Result<AgentInfo>;
    async fn update_agent(&self, input: UpdateAgentInput) -> anyhow::Result<AgentInfo>;
    async fn create_conversation(&self, title: Option<&str>) -> anyhow::Result<String>;
    async fn send_message(
        &self,
        conversation_id: &str,
        agent_id: &str,
        message: &str,
    ) -> anyhow::Result<String>;
    async fn get_conversation(&self, conversation_id: &str) -> anyhow::Result<ConversationState>;
    async fn poll_for_response(
        &self,
        conversation_id: &str,
        poll_interval_ms: u64,
        max_polls: u32,
    ) -> anyhow::Result<ConversationState>;
    async fn list_conversations(
        &self,
        agent_id: Option<&str>,
    ) -> anyhow::Result<Vec<ConversationInfo>>;
    async fn delete_conversation(&self, conversation_id: &str) -> anyhow::Result<()>;
}

// ---------------------------------------------------------------------------
// Matrix implementation
// ---------------------------------------------------------------------------

/// Concrete Matrix GraphQL implementation of `ChatClient`.
pub struct MatrixChatClient {
    api_url: String,
    client: reqwest::Client,
    auth_token: Option<String>,
}

impl MatrixChatClient {
    pub fn new(api_url: impl Into<String>) -> Self {
        let tls_insecure = std::env::var("MATRIX_TLS_INSECURE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(tls_insecure)
            .build()
            .expect("failed to build HTTP client");
        Self {
            api_url: api_url.into(),
            client,
            auth_token: None,
        }
    }

    /// Create a client reusing an existing `reqwest::Client` connection pool.
    pub fn with_client(api_url: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_url: api_url.into(),
            client,
            auth_token: None,
        }
    }

    /// Create a new client sharing the same HTTP connection pool and API URL
    /// as an existing client, but with no auth token set.
    pub fn from_shared(other: &Self) -> Self {
        Self {
            api_url: other.api_url.clone(),
            client: other.client.clone(),
            auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn set_auth_token(&mut self, token: impl Into<String>) {
        self.auth_token = Some(token.into());
    }

    /// Build a POST request to the GraphQL endpoint with Bearer auth.
    fn authed_post(&self) -> reqwest::RequestBuilder {
        let url = format!("{}/api/v1alpha", self.api_url.trim_end_matches('/'));
        let mut req = self.client.post(&url);
        if let Some(ref token) = self.auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        req
    }
}

// ---------------------------------------------------------------------------
// Internal serde helpers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GqlRequest<V: Serialize> {
    query: String,
    variables: V,
    operation_name: String,
}

#[derive(Deserialize)]
struct GqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Deserialize)]
struct GqlError {
    message: String,
}

fn check_errors(errors: Option<Vec<GqlError>>) -> anyhow::Result<()> {
    if let Some(errs) = errors {
        let msgs: Vec<_> = errs.into_iter().map(|e| e.message).collect();
        anyhow::bail!("GraphQL errors: {}", msgs.join(", "));
    }
    Ok(())
}

// -- Agents --

#[derive(Deserialize)]
struct AgentsData {
    agents: Vec<AgentNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentNode {
    id: String,
    name: String,
    description: Option<String>,
    agent_greeting: Option<String>,
}

// -- CreateAgent --

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAgentData {
    create_agent: CreatedAgentNode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedAgentNode {
    id: String,
    name: String,
    description: Option<String>,
    agent_greeting: Option<String>,
}

// -- UpdateAgent --

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAgentData {
    update_agent: CreatedAgentNode,
}

// -- Conversation --

#[derive(Serialize)]
struct CreateConvVars {
    input: CreateConvInput,
}

#[derive(Serialize)]
struct CreateConvInput {
    title: Option<String>,
    #[serde(rename = "type")]
    conversation_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConvData {
    create_conversation: CreateConvNode,
}

#[derive(Deserialize)]
struct CreateConvNode {
    id: String,
}

// -- Ask --

#[derive(Serialize)]
struct AskVars {
    input: AskInput,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AskInput {
    conversation_id: String,
    agent_id: String,
    parts: Vec<MsgPartInput>,
}

#[derive(Serialize)]
struct MsgPartInput {
    #[serde(rename = "type")]
    part_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AskData {
    ask: AskNode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskNode {
    user_message: Option<UserMsgRef>,
}

#[derive(Deserialize)]
struct UserMsgRef {
    id: String,
}

// ---------------------------------------------------------------------------
// Parse helpers for rich message parts
// ---------------------------------------------------------------------------

fn parse_message_parts(parts_json: &[serde_json::Value]) -> (String, Vec<MessagePart>) {
    let mut text_buf = String::new();
    let mut rich_parts = Vec::new();

    for part in parts_json {
        // TextPart
        if let Some(text) = part.get("text").and_then(|t| t.as_str())
            && !text.is_empty()
        {
            if !text_buf.is_empty() {
                text_buf.push('\n');
            }
            text_buf.push_str(text);
            rich_parts.push(MessagePart::Text(text.to_string()));
        }

        // ThinkingPart — thinking is an object with { content }
        if let Some(thinking_obj) = part.get("thinking").and_then(|t| t.as_object())
            && let Some(content) = thinking_obj.get("content").and_then(|c| c.as_str())
            && !content.is_empty()
        {
            rich_parts.push(MessagePart::Thinking(content.to_string()));
        }

        // ToolCallPart
        if let Some(tc) = part.get("toolCall") {
            let tool = ToolCallInfo {
                id: tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: tc
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                arguments: tc
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                result: tc
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                error: tc
                    .get("error")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                status: tc
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
            };
            rich_parts.push(MessagePart::ToolCall(tool));
        }
    }

    (text_buf, rich_parts)
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ChatClient for MatrixChatClient {
    async fn list_agents(&self) -> anyhow::Result<Vec<AgentInfo>> {
        if self.auth_token.is_none() {
            return Ok(Vec::new());
        }

        let query = r#"
            query ListAgents {
                agents(filter: { isEnabled: true }) {
                    id
                    name
                    description
                    agentGreeting
                }
            }
        "#;

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables: serde_json::json!({}),
                operation_name: "ListAgents".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ListAgents failed: {} - {}", status, body);
        }

        let gql: GqlResponse<AgentsData> = resp.json().await?;
        check_errors(gql.errors)?;

        let agents = gql
            .data
            .map(|d| d.agents)
            .unwrap_or_default()
            .into_iter()
            .map(|a| AgentInfo {
                id: a.id,
                name: a.name,
                description: a.description,
                greeting: a.agent_greeting,
            })
            .collect();

        Ok(agents)
    }

    async fn find_agent_by_name(&self, name: &str) -> anyhow::Result<Option<AgentInfo>> {
        let agents = self.list_agents().await?;
        let lower = name.to_lowercase();
        Ok(agents
            .into_iter()
            .find(|a| a.name.to_lowercase().contains(&lower)))
    }

    async fn create_agent(&self, input: CreateAgentInput) -> anyhow::Result<AgentInfo> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            mutation CreateAgent($input: AgentInput!) {
                createAgent(input: $input) {
                    id
                    name
                    description
                    agentGreeting
                }
            }
        "#;

        tracing::info!("Creating agent: {}", input.name);

        let variables = input.to_gql_variables();

        let resp = self
            .authed_post()
            .json(&serde_json::json!({
                "query": query,
                "variables": variables,
                "operationName": "CreateAgent",
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("CreateAgent failed: {} - {}", status, body);
        }

        let gql: GqlResponse<CreateAgentData> = resp.json().await?;
        check_errors(gql.errors)?;

        let node = gql
            .data
            .map(|d| d.create_agent)
            .ok_or_else(|| anyhow::anyhow!("No agent in create response"))?;

        Ok(AgentInfo {
            id: node.id,
            name: node.name,
            description: node.description,
            greeting: node.agent_greeting,
        })
    }

    async fn update_agent(&self, input: UpdateAgentInput) -> anyhow::Result<AgentInfo> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            mutation UpdateAgent($input: UpdateAgentInput!) {
                updateAgent(input: $input) {
                    id
                    name
                    description
                    agentGreeting
                }
            }
        "#;

        tracing::info!("Updating agent tools: id={}", input.id);

        let variables = input.to_gql_variables();

        let resp = self
            .authed_post()
            .json(&serde_json::json!({
                "query": query,
                "variables": variables,
                "operationName": "UpdateAgent",
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("UpdateAgent failed: {} - {}", status, body);
        }

        let gql: GqlResponse<UpdateAgentData> = resp.json().await?;
        check_errors(gql.errors)?;

        let node = gql
            .data
            .map(|d| d.update_agent)
            .ok_or_else(|| anyhow::anyhow!("No agent in update response"))?;

        Ok(AgentInfo {
            id: node.id,
            name: node.name,
            description: node.description,
            greeting: node.agent_greeting,
        })
    }

    async fn create_conversation(&self, title: Option<&str>) -> anyhow::Result<String> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            mutation CreateConversation($input: CreateConversationInput!) {
                createConversation(input: $input) { id }
            }
        "#;

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables: CreateConvVars {
                    input: CreateConvInput {
                        title: title.map(|s| s.to_string()),
                        conversation_type: "CHAT".to_string(),
                    },
                },
                operation_name: "CreateConversation".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("CreateConversation failed: {} - {}", status, body);
        }

        let gql: GqlResponse<CreateConvData> = resp.json().await?;
        check_errors(gql.errors)?;

        let id = gql
            .data
            .map(|d| d.create_conversation.id)
            .ok_or_else(|| anyhow::anyhow!("No conversation id in response"))?;

        Ok(id)
    }

    async fn send_message(
        &self,
        conversation_id: &str,
        agent_id: &str,
        message: &str,
    ) -> anyhow::Result<String> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            mutation SendMessage($input: ConversationInput!) {
                ask(input: $input) {
                    userMessage { id }
                }
            }
        "#;

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables: AskVars {
                    input: AskInput {
                        conversation_id: conversation_id.to_string(),
                        agent_id: agent_id.to_string(),
                        parts: vec![MsgPartInput {
                            part_type: "TEXT".to_string(),
                            text: Some(message.to_string()),
                        }],
                    },
                },
                operation_name: "SendMessage".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("SendMessage failed: {} - {}", status, body);
        }

        let gql: GqlResponse<AskData> = resp.json().await?;
        check_errors(gql.errors)?;

        let msg_id = gql
            .data
            .and_then(|d| d.ask.user_message)
            .map(|m| m.id)
            .unwrap_or_default();

        Ok(msg_id)
    }

    async fn get_conversation(&self, conversation_id: &str) -> anyhow::Result<ConversationState> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            query GetConversation($id: ID!) {
                conversation(id: $id) {
                    id
                    agentStatus
                    messages {
                        id
                        parts {
                            ... on TextPart { id text }
                            ... on ThinkingPart { id thinking { content } }
                            ... on ToolCallPart {
                                id
                                toolCall {
                                    id
                                    name
                                    arguments
                                    result
                                    error
                                    status
                                }
                            }
                        }
                        profile {
                            id
                            type
                            name
                        }
                    }
                }
            }
        "#;

        #[derive(Serialize)]
        struct Vars {
            id: String,
        }

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables: Vars {
                    id: conversation_id.to_string(),
                },
                operation_name: "GetConversation".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GetConversation failed: {} - {}", status, body);
        }

        let body: serde_json::Value = resp.json().await?;

        if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
            let msgs: Vec<_> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect();
            anyhow::bail!("GraphQL errors: {}", msgs.join(", "));
        }

        let conv = body
            .get("data")
            .and_then(|d| d.get("conversation"))
            .ok_or_else(|| anyhow::anyhow!("No conversation in response"))?;

        let agent_status = conv
            .get("agentStatus")
            .and_then(|s| s.as_str())
            .unwrap_or("IDLE")
            .to_uppercase();

        let messages = conv
            .get("messages")
            .and_then(|m| m.as_array())
            .map(|msgs| {
                msgs.iter()
                    .map(|m| {
                        let id = m
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let sender_type = m
                            .get("profile")
                            .and_then(|p| p.get("type"))
                            .and_then(|t| t.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let sender_name = m
                            .get("profile")
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let parts_json = m
                            .get("parts")
                            .and_then(|p| p.as_array())
                            .cloned()
                            .unwrap_or_default();

                        let (text, parts) = parse_message_parts(&parts_json);

                        ChatMessage {
                            id,
                            sender_type,
                            sender_name,
                            text,
                            parts,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(ConversationState {
            messages,
            agent_status,
        })
    }

    async fn poll_for_response(
        &self,
        conversation_id: &str,
        poll_interval_ms: u64,
        max_polls: u32,
    ) -> anyhow::Result<ConversationState> {
        for _attempt in 0..max_polls {
            let result = self.get_conversation(conversation_id).await?;

            let done = matches!(
                result.agent_status.as_str(),
                "IDLE" | "STREAM_END" | "ERROR"
            );
            let has_agent_msg = result
                .messages
                .iter()
                .any(|m| m.sender_type != "USER" && !m.text.is_empty());

            if done && has_agent_msg {
                return Ok(result);
            }

            tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
        }

        self.get_conversation(conversation_id).await
    }

    async fn list_conversations(
        &self,
        agent_id: Option<&str>,
    ) -> anyhow::Result<Vec<ConversationInfo>> {
        if self.auth_token.is_none() {
            return Ok(Vec::new());
        }

        let query = r#"
            query ListConversations($first: Int, $filter: ListConversationsFilter) {
                listConversations(first: $first, filter: $filter) {
                    edges {
                        node {
                            id
                            title
                            summary
                            updatedAt
                        }
                    }
                }
            }
        "#;

        let mut filter = serde_json::json!({ "type": "CHAT" });
        if let Some(aid) = agent_id {
            filter
                .as_object_mut()
                .unwrap()
                .insert("agentIds".into(), serde_json::json!([aid]));
        }

        let variables = serde_json::json!({
            "first": 50,
            "filter": filter,
        });

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables,
                operation_name: "ListConversations".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ListConversations failed: {} - {}", status, body);
        }

        let body: serde_json::Value = resp.json().await?;

        if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
            let msgs: Vec<_> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect();
            anyhow::bail!("GraphQL errors: {}", msgs.join(", "));
        }

        let edges = body
            .get("data")
            .and_then(|d| d.get("listConversations"))
            .and_then(|lc| lc.get("edges"))
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        let conversations = edges
            .iter()
            .filter_map(|edge| {
                let node = edge.get("node")?;
                Some(ConversationInfo {
                    id: node.get("id")?.as_str()?.to_string(),
                    title: node
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Untitled")
                        .to_string(),
                    summary: node
                        .get("summary")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string()),
                    updated_at: node
                        .get("updatedAt")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect();

        Ok(conversations)
    }

    async fn delete_conversation(&self, conversation_id: &str) -> anyhow::Result<()> {
        if self.auth_token.is_none() {
            anyhow::bail!("Auth token required");
        }

        let query = r#"
            mutation DeleteConversation($id: ID!) {
                deleteConversation(id: $id) { id }
            }
        "#;

        #[derive(Serialize)]
        struct Vars {
            id: String,
        }

        let resp = self
            .authed_post()
            .json(&GqlRequest {
                query: query.to_string(),
                variables: Vars {
                    id: conversation_id.to_string(),
                },
                operation_name: "DeleteConversation".to_string(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeleteConversation failed: {} - {}", status, body);
        }

        let body: serde_json::Value = resp.json().await?;
        if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
            let msgs: Vec<_> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect();
            anyhow::bail!("GraphQL errors: {}", msgs.join(", "));
        }

        Ok(())
    }
}
