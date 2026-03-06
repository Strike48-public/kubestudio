//! Drill-down component showing jobs triggered by a CronJob

use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use k8s_openapi::api::batch::v1::Job;

/// Props for the CronJobJobsDrillDown component
#[derive(Props, Clone, PartialEq)]
pub struct CronJobJobsDrillDownProps {
    /// Name of the CronJob
    pub cronjob_name: String,
    /// Namespace of the CronJob
    pub namespace: String,
    /// Cluster connection
    pub cluster: Signal<Option<ClusterContext>>,
    /// Currently selected job index
    pub selected_index: Signal<Option<usize>>,
    /// Callback when back is requested (Escape)
    pub on_back: EventHandler<()>,
    /// Callback when a job is selected (Enter/click) - provides (job_name, namespace)
    pub on_select_job: EventHandler<(String, String)>,
    /// Callback when selection changes - for tracking selected job in keyboard nav
    #[props(default)]
    pub on_selection_change: EventHandler<Option<(String, String)>>,
}

/// Get job status information - returns (status_text, status_class)
fn get_job_status(job: &Job) -> (&'static str, &'static str) {
    let status = job.status.as_ref();

    // Check conditions
    if let Some(conditions) = status.and_then(|s| s.conditions.as_ref()) {
        for condition in conditions {
            if condition.type_ == "Complete" && condition.status == "True" {
                return ("Complete", "status-success");
            }
            if condition.type_ == "Failed" && condition.status == "True" {
                return ("Failed", "status-error");
            }
        }
    }

    // Check active/succeeded/failed counts
    let active = status.and_then(|s| s.active).unwrap_or(0);
    let succeeded = status.and_then(|s| s.succeeded).unwrap_or(0);
    let failed = status.and_then(|s| s.failed).unwrap_or(0);

    if active > 0 {
        ("Running", "status-success")
    } else if succeeded > 0 && failed == 0 {
        ("Complete", "status-success")
    } else if failed > 0 {
        ("Failed", "status-error")
    } else {
        ("Pending", "status-warning")
    }
}

/// Get completions string (e.g., "1/1")
fn get_completions(job: &Job) -> String {
    let desired = job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1);
    let succeeded = job.status.as_ref().and_then(|s| s.succeeded).unwrap_or(0);
    format!("{}/{}", succeeded, desired)
}

/// Get duration string
fn get_duration(job: &Job) -> String {
    let status = job.status.as_ref();
    let start = status.and_then(|s| s.start_time.as_ref());
    let completion = status.and_then(|s| s.completion_time.as_ref());

    match (start, completion) {
        (Some(start), Some(end)) => {
            let duration = end.0.signed_duration_since(start.0);
            let secs = duration.num_seconds();
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m{}s", secs / 60, secs % 60)
            } else {
                format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
            }
        }
        (Some(start), None) => {
            // Still running
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(start.0);
            let secs = duration.num_seconds();
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m{}s", secs / 60, secs % 60)
            } else {
                format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
            }
        }
        _ => "-".to_string(),
    }
}

/// Get age string
fn get_age(job: &Job) -> String {
    let creation = job.metadata.creation_timestamp.as_ref();
    match creation {
        Some(ts) => {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(ts.0);
            let secs = duration.num_seconds();
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m", secs / 60)
            } else if secs < 86400 {
                format!("{}h", secs / 3600)
            } else {
                format!("{}d", secs / 86400)
            }
        }
        None => "-".to_string(),
    }
}

/// Component for drilling down into jobs triggered by a CronJob
#[component]
pub fn CronJobJobsDrillDown(mut props: CronJobJobsDrillDownProps) -> Element {
    let mut jobs = use_signal(Vec::<Job>::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);

    // Fetch jobs for the cronjob
    let cronjob_name = props.cronjob_name.clone();
    let namespace = props.namespace.clone();

    use_effect(move || {
        let cluster = props.cluster.read().clone();
        let cronjob_name = cronjob_name.clone();
        let namespace = namespace.clone();

        spawn(async move {
            loading.set(true);
            error.set(None);

            if let Some(ctx) = cluster {
                match ctx
                    .client
                    .list_jobs_for_cronjob(&cronjob_name, &namespace)
                    .await
                {
                    Ok(job_list) => {
                        jobs.set(job_list);
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to fetch jobs: {}", e)));
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
            let jobs_read = jobs.read();
            let selected_idx = *props.selected_index.read();

            let selected_job_info = selected_idx.and_then(|idx| {
                jobs_read.get(idx).and_then(|job| {
                    let name = job.metadata.name.clone()?;
                    let ns = job.metadata.namespace.clone()?;
                    Some((name, ns))
                })
            });

            on_selection_change.call(selected_job_info);
        });
    }

    let job_list = jobs.read();
    let selected_idx = *props.selected_index.read();
    let on_back = props.on_back;

    rsx! {
        div { class: "container-drilldown",
            // Header - matches other drill-down styles
            div { class: "drilldown-header",
                h3 { "CronJob: {props.cronjob_name}" }
                span { class: "drilldown-namespace", "in {props.namespace}" }
                button {
                    class: "back-btn",
                    onclick: move |_| on_back.call(()),
                    "Back (Esc)"
                }
            }

            // Hint text
            div { class: "drilldown-hint",
                "Use ↑↓ to navigate • Enter to view pods • Esc to go back"
            }

            // Content
            if *loading.read() {
                div { class: "empty-state",
                    "Loading jobs..."
                }
            } else if let Some(err) = error.read().as_ref() {
                div { class: "empty-state error",
                    "Error: {err}"
                }
            } else if job_list.is_empty() {
                div { class: "empty-state",
                    "No jobs found for this CronJob"
                }
            } else {
                // Job table
                div { class: "container-table-wrapper",
                    table { class: "container-table",
                        thead {
                            tr {
                                th { "" }  // Status dot column
                                th { "Job" }
                                th { "Status" }
                                th { "Completions" }
                                th { "Duration" }
                                th { "Age" }
                            }
                        }
                        tbody {
                            for (idx, job) in job_list.iter().enumerate() {
                                {
                                    let job_name = job.metadata.name.clone().unwrap_or_default();
                                    let job_ns = job.metadata.namespace.clone().unwrap_or_default();
                                    let (status_text, status_class) = get_job_status(job);
                                    let completions = get_completions(job);
                                    let duration = get_duration(job);
                                    let age = get_age(job);
                                    let is_selected = selected_idx == Some(idx);
                                    let row_class = if is_selected {
                                        "container-row selected"
                                    } else {
                                        "container-row"
                                    };

                                    rsx! {
                                        tr {
                                            key: "{job_name}",
                                            class: "{row_class}",
                                            "data-job-idx": "{idx}",
                                            onclick: {
                                                let job_name = job_name.clone();
                                                let job_ns = job_ns.clone();
                                                move |_| {
                                                    props.selected_index.set(Some(idx));
                                                    props.on_select_job.call((job_name.clone(), job_ns.clone()));
                                                }
                                            },
                                            td { class: "col-status",
                                                span { class: "status-dot {status_class}" }
                                            }
                                            td { class: "col-name", "{job_name}" }
                                            td { class: "col-status-text",
                                                span { class: "status-badge {status_class}", "{status_text}" }
                                            }
                                            td { class: "col-completions", "{completions}" }
                                            td { class: "col-duration", "{duration}" }
                                            td { class: "col-age", "{age}" }
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
