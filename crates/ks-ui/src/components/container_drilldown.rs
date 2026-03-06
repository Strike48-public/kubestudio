use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Pod;

#[derive(Props, Clone, PartialEq)]
pub struct ContainerDrillDownProps {
    /// Pod name
    pub pod_name: String,
    /// Namespace
    pub namespace: String,
    /// The pod data (optional - shows "not found" if None)
    pub pod: Option<Pod>,
    /// Currently selected container index
    pub selected_index: Signal<Option<usize>>,
    /// Called when user presses Escape or clicks Back
    pub on_back: EventHandler<()>,
    /// Called when user selects a container (e.g., to view logs)
    pub on_select_container: EventHandler<(usize, String)>,
}

#[component]
pub fn ContainerDrillDown(props: ContainerDrillDownProps) -> Element {
    let pod_name = props.pod_name.clone();
    let namespace = props.namespace.clone();
    let on_back = props.on_back;

    rsx! {
        div { class: "container-drilldown",
            div { class: "drilldown-header",
                h3 { "Pod: {pod_name}" }
                span { class: "drilldown-namespace", "in {namespace}" }
                button {
                    class: "back-btn",
                    onclick: move |_| on_back.call(()),
                    "Back (Esc)"
                }
            }
            div { class: "drilldown-hint",
                "Use ↑↓ to navigate • Enter to view logs • Esc to go back"
            }
            div { class: "container-table-wrapper",
                table { class: "container-table",
                    thead {
                        tr {
                            th { "" }
                            th { "Container" }
                            th { "Image" }
                            th { "Status" }
                            th { "Ready" }
                            th { "Restarts" }
                            th { "CPU Req/Lim" }
                            th { "Mem Req/Lim" }
                        }
                    }
                    tbody {
                        if let Some(pod) = &props.pod {
                            if let Some(spec) = &pod.spec {
                                for (idx, container) in spec.containers.iter().enumerate() {
                                    {
                                        let is_selected = *props.selected_index.read() == Some(idx);
                                        let row_class = if is_selected { "container-row selected" } else { "container-row" };
                                        let container_name = container.name.clone();
                                        let on_select = props.on_select_container;

                                        // Get container status
                                        let status_info = pod.status.as_ref().and_then(|s| {
                                            s.container_statuses.as_ref().and_then(|statuses| {
                                                statuses.iter().find(|cs| cs.name == container.name).cloned()
                                            })
                                        });

                                        let (status_text, status_class) = get_container_status(&status_info);
                                        let ready = status_info.as_ref().map(|cs| cs.ready).unwrap_or(false);
                                        let restarts = status_info.as_ref().map(|cs| cs.restart_count).unwrap_or(0);

                                        // Get resource requests/limits
                                        let (cpu_req, cpu_lim, mem_req, mem_lim) = get_resource_info(container);

                                        // Truncate image name for display
                                        let image = container.image.clone().unwrap_or_default();
                                        let image_short = if image.len() > 50 {
                                            format!("...{}", &image[image.len()-47..])
                                        } else {
                                            image.clone()
                                        };

                                        rsx! {
                                            tr {
                                                class: "{row_class}",
                                                "data-container-idx": "{idx}",
                                                onclick: move |_| {
                                                    on_select.call((idx, container_name.clone()));
                                                },
                                                td { class: "col-status",
                                                    span { class: "status-dot {status_class}" }
                                                }
                                                td { class: "col-name", "{container.name}" }
                                                td { class: "col-image", title: "{image}", "{image_short}" }
                                                td { class: "col-status-text",
                                                    span { class: "status-badge {status_class}", "{status_text}" }
                                                }
                                                td { class: "col-ready",
                                                    if ready { "Yes" } else { "No" }
                                                }
                                                td { class: "col-restarts", "{restarts}" }
                                                td { class: "col-resources", "{cpu_req}/{cpu_lim}" }
                                                td { class: "col-resources", "{mem_req}/{mem_lim}" }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            tr {
                                td { colspan: "8",
                                    div { class: "empty-state", "Pod not found" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Get container status text and CSS class
fn get_container_status(
    status_info: &Option<k8s_openapi::api::core::v1::ContainerStatus>,
) -> (String, &'static str) {
    if let Some(cs) = status_info {
        if let Some(state) = &cs.state {
            if state.running.is_some() {
                ("Running".to_string(), "status-success")
            } else if let Some(waiting) = &state.waiting {
                (
                    waiting.reason.clone().unwrap_or("Waiting".to_string()),
                    "status-warning",
                )
            } else if let Some(terminated) = &state.terminated {
                (
                    terminated
                        .reason
                        .clone()
                        .unwrap_or("Terminated".to_string()),
                    "status-error",
                )
            } else {
                ("Unknown".to_string(), "status-unknown")
            }
        } else {
            ("Unknown".to_string(), "status-unknown")
        }
    } else {
        ("Unknown".to_string(), "status-unknown")
    }
}

/// Get resource requests/limits as formatted strings
fn get_resource_info(
    container: &k8s_openapi::api::core::v1::Container,
) -> (String, String, String, String) {
    let resources = container.resources.as_ref();

    let cpu_req = resources
        .and_then(|r| r.requests.as_ref())
        .and_then(|req| req.get("cpu"))
        .map(|q| q.0.clone())
        .unwrap_or("-".to_string());

    let cpu_lim = resources
        .and_then(|r| r.limits.as_ref())
        .and_then(|lim| lim.get("cpu"))
        .map(|q| q.0.clone())
        .unwrap_or("-".to_string());

    let mem_req = resources
        .and_then(|r| r.requests.as_ref())
        .and_then(|req| req.get("memory"))
        .map(|q| q.0.clone())
        .unwrap_or("-".to_string());

    let mem_lim = resources
        .and_then(|r| r.limits.as_ref())
        .and_then(|lim| lim.get("memory"))
        .map(|q| q.0.clone())
        .unwrap_or("-".to_string());

    (cpu_req, cpu_lim, mem_req, mem_lim)
}
