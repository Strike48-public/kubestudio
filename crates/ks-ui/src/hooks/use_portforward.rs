// Port-forward management hook

use dioxus::prelude::*;
use ks_kube::PortForwardHandle;
use std::collections::HashMap;
use std::sync::Mutex;

/// Info about an active port-forward for display
#[derive(Clone, Debug, PartialEq)]
pub struct ActivePortForward {
    pub id: String,
    pub pod_name: String,
    pub namespace: String,
    pub local_port: u16,
    pub remote_port: u16,
}

/// Global store for port-forward handles that persists across webview refreshes.
/// The Tokio tasks keep running even when the Dioxus component tree re-mounts,
/// so we need state that outlives the component lifecycle.
static PORT_FORWARD_STORE: Mutex<Option<HashMap<String, (ActivePortForward, PortForwardHandle)>>> =
    Mutex::new(None);

fn with_store<R>(
    f: impl FnOnce(&mut HashMap<String, (ActivePortForward, PortForwardHandle)>) -> R,
) -> R {
    let mut guard = PORT_FORWARD_STORE.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

/// State for managing active port-forwards
#[derive(Clone, Default)]
pub struct PortForwardState {
    /// Active port-forwards by ID (display info only; handles live in the global store)
    forwards: HashMap<String, ActivePortForward>,
}

impl PortForwardState {
    /// Build state from the global store
    fn from_global() -> Self {
        let forwards = with_store(|store| {
            store
                .iter()
                .map(|(id, (info, _))| (id.clone(), info.clone()))
                .collect()
        });
        Self { forwards }
    }

    /// Get list of active port-forwards
    pub fn list(&self) -> Vec<ActivePortForward> {
        self.forwards.values().cloned().collect()
    }

    /// Add a new port-forward
    pub fn add(&mut self, handle: PortForwardHandle) -> String {
        let id = format!(
            "{}:{}:{}",
            handle.pod_name, handle.namespace, handle.local_port
        );
        let info = ActivePortForward {
            id: id.clone(),
            pod_name: handle.pod_name.clone(),
            namespace: handle.namespace.clone(),
            local_port: handle.local_port,
            remote_port: handle.remote_port,
        };
        with_store(|store| {
            store.insert(id.clone(), (info.clone(), handle));
        });
        self.forwards.insert(id.clone(), info);
        id
    }

    /// Stop and remove a port-forward
    pub fn remove(&mut self, id: &str) {
        with_store(|store| {
            if let Some((_, handle)) = store.remove(id) {
                handle.stop();
            }
        });
        self.forwards.remove(id);
    }

    /// Stop all port-forwards
    #[allow(dead_code)]
    pub fn stop_all(&mut self) {
        with_store(|store| {
            for (_, handle) in store.values() {
                handle.stop();
            }
            store.clear();
        });
        self.forwards.clear();
    }

    /// Check if a port-forward exists for the given local port
    pub fn has_port(&self, local_port: u16) -> bool {
        self.forwards
            .values()
            .any(|info| info.local_port == local_port)
    }

    /// Get count of active forwards
    pub fn count(&self) -> usize {
        self.forwards.len()
    }
}

/// Hook to manage port-forwards
/// Returns a signal that is initialized from the global store, so active
/// forwards survive webview refreshes.
pub fn use_port_forwards() -> Signal<PortForwardState> {
    use_signal(PortForwardState::from_global)
}
