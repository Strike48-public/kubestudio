//! Drill-down component showing pods belonging to a workload (Deployment, StatefulSet, etc.)

use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Pod;

/// Props for the WorkloadPodsDrillDown component
#[derive(Props, Clone, PartialEq)]
pub struct WorkloadPodsDrillDownProps {
    /// The kind of workload (e.g., "Deployment", "StatefulSet")
    pub workload_kind: String,
    /// Name of the workload
    pub workload_name: String,
    /// Namespace of the workload
    pub namespace: String,
    /// Cluster connection
    pub cluster: Signal<Option<ClusterContext>>,
    /// Currently selected pod index
    pub selected_index: Signal<Option<usize>>,
    /// Callback when back is requested (Escape)
    pub on_back: EventHandler<()>,
    /// Callback when a pod is selected (Enter/click) - provides (pod_name, namespace)
    pub on_select_pod: EventHandler<(String, String)>,
    /// Callback when selection changes - for tracking selected pod in keyboard nav
    #[props(default)]
    pub on_selection_change: EventHandler<Option<(String, String)>>,
}

/// Get pod status information - returns (status_text, status_class)
fn get_pod_status(pod: &Pod) -> (&'static str, &'static str) {
    let status = pod.status.as_ref();
    let phase = status.and_then(|s| s.phase.as_ref()).map(|s| s.as_str());

    match phase {
        Some("Running") => {
            // Check container statuses
            let ready = status
                .and_then(|s| s.container_statuses.as_ref())
                .map(|cs| cs.iter().all(|c| c.ready))
                .unwrap_or(false);

            if ready {
                ("Running", "status-success")
            } else {
                ("NotReady", "status-warning")
            }
        }
        Some("Succeeded") => ("Completed", "status-success"),
        Some("Failed") => ("Failed", "status-error"),
        Some("Pending") => ("Pending", "status-warning"),
        _ => ("Unknown", "status-unknown"),
    }
}

/// Get ready count string (e.g., "2/3")
fn get_ready_count(pod: &Pod) -> String {
    let container_statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref());

    match container_statuses {
        Some(statuses) => {
            let ready = statuses.iter().filter(|c| c.ready).count();
            let total = statuses.len();
            format!("{}/{}", ready, total)
        }
        None => "-".to_string(),
    }
}

/// Get restart count
fn get_restart_count(pod: &Pod) -> i32 {
    pod.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| cs.iter().map(|c| c.restart_count).sum())
        .unwrap_or(0)
}

/// Component for drilling down into pods belonging to a workload
#[component]
pub fn WorkloadPodsDrillDown(mut props: WorkloadPodsDrillDownProps) -> Element {
    let mut pods = use_signal(Vec::<Pod>::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);

    // Fetch pods for the workload
    let workload_kind = props.workload_kind.clone();
    let workload_name = props.workload_name.clone();
    let namespace = props.namespace.clone();

    use_effect(move || {
        let cluster = props.cluster.read().clone();
        let workload_kind = workload_kind.clone();
        let workload_name = workload_name.clone();
        let namespace = namespace.clone();

        spawn(async move {
            loading.set(true);
            error.set(None);

            if let Some(ctx) = cluster {
                let result = match workload_kind.as_str() {
                    "Deployment" => {
                        ctx.client
                            .list_pods_for_deployment(&workload_name, &namespace)
                            .await
                    }
                    "StatefulSet" => {
                        ctx.client
                            .list_pods_for_statefulset(&workload_name, &namespace)
                            .await
                    }
                    "DaemonSet" => {
                        ctx.client
                            .list_pods_for_daemonset(&workload_name, &namespace)
                            .await
                    }
                    "Job" => {
                        ctx.client
                            .list_pods_for_job(&workload_name, &namespace)
                            .await
                    }
                    _ => {
                        error.set(Some(format!("Unknown workload kind: {}", workload_kind)));
                        loading.set(false);
                        return;
                    }
                };

                match result {
                    Ok(pod_list) => {
                        pods.set(pod_list);
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to fetch pods: {}", e)));
                    }
                }
            }
            loading.set(false);
        });
    });

    // Notify parent of selection changes for keyboard Enter support
    {
        let on_selection_change = props.on_selection_change;

        use_effect(move || {
            let pods_read = pods.read();
            let selected_idx = *props.selected_index.read();

            let selected_pod_info = selected_idx.and_then(|idx| {
                pods_read.get(idx).and_then(|pod| {
                    let name = pod.metadata.name.clone()?;
                    let ns = pod.metadata.namespace.clone()?;
                    Some((name, ns))
                })
            });

            on_selection_change.call(selected_pod_info);
        });
    }

    let pod_list = pods.read();
    let selected_idx = *props.selected_index.read();
    let on_back = props.on_back;

    rsx! {
        div { class: "container-drilldown",
            // Header - matches ContainerDrillDown style
            div { class: "drilldown-header",
                h3 { "{props.workload_kind}: {props.workload_name}" }
                span { class: "drilldown-namespace", "in {props.namespace}" }
                button {
                    class: "back-btn",
                    onclick: move |_| on_back.call(()),
                    "Back (Esc)"
                }
            }

            // Hint text
            div { class: "drilldown-hint",
                "Use ↑↓ to navigate • Enter to view containers • Esc to go back"
            }

            // Content
            if *loading.read() {
                div { class: "empty-state",
                    "Loading pods..."
                }
            } else if let Some(err) = error.read().as_ref() {
                div { class: "empty-state error",
                    "Error: {err}"
                }
            } else if pod_list.is_empty() {
                div { class: "empty-state",
                    "No pods found for this {props.workload_kind}"
                }
            } else {
                // Pod table - matches ContainerDrillDown table style
                div { class: "container-table-wrapper",
                    table { class: "container-table",
                        thead {
                            tr {
                                th { "" }  // Status dot column
                                th { "Pod" }
                                th { "Status" }
                                th { "Ready" }
                                th { "Restarts" }
                                th { "Node" }
                            }
                        }
                        tbody {
                            for (idx, pod) in pod_list.iter().enumerate() {
                                {
                                    let pod_name = pod.metadata.name.clone().unwrap_or_default();
                                    let pod_ns = pod.metadata.namespace.clone().unwrap_or_default();
                                    let (status_text, status_class) = get_pod_status(pod);
                                    let ready = get_ready_count(pod);
                                    let restarts = get_restart_count(pod);
                                    let node = pod.spec.as_ref()
                                        .and_then(|s| s.node_name.clone())
                                        .unwrap_or_else(|| "-".to_string());
                                    let is_selected = selected_idx == Some(idx);
                                    let row_class = if is_selected {
                                        "container-row selected"
                                    } else {
                                        "container-row"
                                    };

                                    rsx! {
                                        tr {
                                            key: "{pod_name}",
                                            class: "{row_class}",
                                            "data-pod-idx": "{idx}",
                                            onclick: {
                                                let pod_name = pod_name.clone();
                                                let pod_ns = pod_ns.clone();
                                                move |_| {
                                                    props.selected_index.set(Some(idx));
                                                    props.on_select_pod.call((pod_name.clone(), pod_ns.clone()));
                                                }
                                            },
                                            td { class: "col-status",
                                                span { class: "status-dot {status_class}" }
                                            }
                                            td { class: "col-name", "{pod_name}" }
                                            td { class: "col-status-text",
                                                span { class: "status-badge {status_class}", "{status_text}" }
                                            }
                                            td { class: "col-ready", "{ready}" }
                                            td { class: "col-restarts", "{restarts}" }
                                            td { class: "col-node", "{node}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
