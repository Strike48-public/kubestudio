//! Navigation stack for view state management
//! Provides push/pop history for drill-down views

#![allow(dead_code)] // Some methods are reserved for future use

use dioxus::prelude::*;

/// Generic navigation stack that can work with any view state type
#[derive(Clone)]
pub struct NavigationStack<T: Clone + Default + PartialEq> {
    /// History stack (previous views)
    stack: Vec<T>,
    /// Current view
    current: T,
}

impl<T: Clone + Default + PartialEq> Default for NavigationStack<T> {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            current: T::default(),
        }
    }
}

impl<T: Clone + Default + PartialEq> NavigationStack<T> {
    /// Create a new navigation stack starting at the given view
    pub fn new(initial: T) -> Self {
        Self {
            stack: Vec::new(),
            current: initial,
        }
    }

    /// Push a new view onto the stack (drill into)
    /// The current view becomes part of the history
    pub fn push(&mut self, view: T) {
        // Don't push if already at the same view
        if self.current != view {
            self.stack.push(self.current.clone());
            self.current = view;
        }
    }

    /// Pop back to the previous view (escape/back)
    /// Returns the new current view, or None if already at root
    pub fn pop(&mut self) -> Option<T> {
        if let Some(prev) = self.stack.pop() {
            self.current = prev.clone();
            Some(prev)
        } else {
            None
        }
    }

    /// Pop back to root, clearing all history
    /// Returns true if there was anything to pop
    pub fn pop_to_root(&mut self) -> bool {
        if self.stack.is_empty() {
            false
        } else {
            self.stack.clear();
            self.current = T::default();
            true
        }
    }

    /// Replace the current view without adding to history
    /// Used for switching between resource types
    pub fn replace(&mut self, view: T) {
        self.current = view;
    }

    /// Replace and clear history - start fresh at a new view
    pub fn reset(&mut self, view: T) {
        self.stack.clear();
        self.current = view;
    }

    /// Get the current view
    pub fn current(&self) -> &T {
        &self.current
    }

    /// Check if we can go back
    pub fn can_go_back(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Get the stack depth
    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if at root (no history)
    pub fn is_at_root(&self) -> bool {
        self.stack.is_empty()
    }
}

/// Represents the current view state in the application
#[derive(Clone, PartialEq, Debug, Default)]
pub enum ViewState {
    /// Main resource list view
    #[default]
    ResourceList,

    // === Pod drill-downs ===
    /// Container drill-down for a pod
    ContainerDrillDown { pod_name: String, namespace: String },
    /// Log viewer for a pod/container
    LogViewer {
        pod_name: String,
        namespace: String,
        container: Option<String>,
    },
    /// Exec/Shell viewer for a pod/container
    ExecViewer {
        pod_name: String,
        namespace: String,
        container: Option<String>,
    },
    /// YAML/Describe viewer for any resource
    YamlViewer {
        kind: String,
        name: String,
        namespace: Option<String>,
    },

    // === Workload drill-downs ===
    /// Pods belonging to a Deployment
    DeploymentPods {
        deployment_name: String,
        namespace: String,
    },
    /// Pods belonging to a StatefulSet
    StatefulSetPods {
        statefulset_name: String,
        namespace: String,
    },
    /// Pods belonging to a DaemonSet
    DaemonSetPods {
        daemonset_name: String,
        namespace: String,
    },
    /// Pods belonging to a Job
    JobPods { job_name: String, namespace: String },
    /// Jobs triggered by a CronJob
    CronJobJobs {
        cronjob_name: String,
        namespace: String,
    },

    // === Network drill-downs ===
    /// Endpoints/Pods backing a Service
    ServiceEndpoints {
        service_name: String,
        namespace: String,
    },
    /// Backends for an Ingress
    IngressBackends {
        ingress_name: String,
        namespace: String,
    },

    // === Storage drill-downs ===
    /// Pods using a PVC
    PvcPods { pvc_name: String, namespace: String },

    // === Resource operations ===
    /// Create a new resource from template
    CreateResource,
    /// Apply manifest from file
    ApplyFile { path: String },
}

/// Type alias for navigation state using ViewState
pub type NavigationState = NavigationStack<ViewState>;

/// Hook for navigation state management
pub fn use_navigation() -> Signal<NavigationState> {
    use_signal(NavigationState::default)
}
