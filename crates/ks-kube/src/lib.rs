// ks-kube: Kubernetes API client layer for KubeStudio

pub mod auth;
pub mod client;
pub mod matrix;
pub mod toolbox;

pub use client::{
    ApplyResult, ContainerMetrics, CrdInfo, CrdScope, ExecHandle, KubeClient, MultiApplyResult,
    NodeMetrics, PodMetrics, PortForwardHandle, PortForwardInfo, PrinterColumn, WatchEvent,
    WatchStream,
};
pub use matrix::{
    AgentInfo, ChatClient, ChatMessage, ConversationInfo, ConversationState, CreateAgentInput,
    MatrixChatClient, MessagePart, ToolCallInfo, UpdateAgentInput,
};
pub use toolbox::{ExecResult, PermissionMode, Toolbox, ToolboxStatus, cleanup_orphaned_toolbox};
