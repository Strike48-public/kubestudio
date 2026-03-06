use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, watch};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    pub clusters: Vec<ClusterState>,
    pub active_cluster_idx: Option<usize>,
    pub active_namespace: Option<String>,
    pub selected_resource: Option<ResourceRef>,
    pub ui: UiState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterState {
    pub name: String,
    pub connected: bool,
    pub namespaces: Vec<String>,
    pub resources: HashMap<String, Vec<ResourceEntry>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceEntry {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub yaml: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiState {
    pub sidebar_visible: bool,
    pub command_palette_open: bool,
    pub current_view: View,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum View {
    #[default]
    Overview,
    ResourceList {
        kind: String,
    },
    ResourceDetail {
        kind: String,
        name: String,
        namespace: Option<String>,
    },
    Logs {
        pod: String,
        namespace: String,
        container: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRef {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

/// Thread-safe state store with subscription support
pub struct Store {
    state: Arc<RwLock<AppState>>,
    tx: watch::Sender<AppState>,
}

impl Store {
    pub fn new() -> Self {
        let initial_state = AppState::default();
        let (tx, _rx) = watch::channel(initial_state.clone());
        Self {
            state: Arc::new(RwLock::new(initial_state)),
            tx,
        }
    }

    pub async fn get(&self) -> AppState {
        self.state.read().await.clone()
    }

    pub async fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut AppState),
    {
        let mut state = self.state.write().await;
        f(&mut state);
        let _ = self.tx.send(state.clone());
    }

    pub fn subscribe(&self) -> watch::Receiver<AppState> {
        self.tx.subscribe()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
