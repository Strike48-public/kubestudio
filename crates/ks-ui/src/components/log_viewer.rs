use crate::hooks::ClusterContext;
use dioxus::prelude::*;
use lucide_dioxus::Circle;
use tokio::io::AsyncBufReadExt;

/// Maximum number of log lines to keep in buffer
const MAX_LOG_LINES: usize = 10000;

#[derive(Props, Clone, PartialEq)]
pub struct LogViewerProps {
    /// Pod name to fetch logs from
    pub pod_name: String,
    /// Namespace of the pod
    pub namespace: String,
    /// Optional container name (if None, fetches from first/default container)
    pub container: Option<String>,
    /// Cluster connection for fetching logs
    pub cluster: Signal<Option<ClusterContext>>,
    /// Called when user presses Escape or Back button
    pub on_back: EventHandler<()>,
}

#[component]
pub fn LogViewer(props: LogViewerProps) -> Element {
    // Internal state
    let mut log_content = use_signal(Vec::<String>::new);
    let mut loading = use_signal(|| true);
    let mut text_wrap = use_signal(|| true);
    let mut following = use_signal(|| true); // Start in follow mode
    let mut show_timestamps = use_signal(|| false);
    let mut stream_active = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    let mut should_scroll = use_signal(|| false);

    // Clone props for effects
    let pod_name = props.pod_name.clone();
    let namespace = props.namespace.clone();
    let container = props.container.clone();
    let cluster = props.cluster;

    // Initial log fetch - runs once on mount
    use_effect(move || {
        let pod_name = pod_name.clone();
        let namespace = namespace.clone();
        let container = container.clone();

        spawn(async move {
            loading.set(true);
            log_content.set(vec![]);
            error_message.set(None);

            if let Some(ctx) = cluster.read().clone() {
                // Get recent logs (non-streaming)
                match ctx
                    .client
                    .get_pod_logs(&pod_name, &namespace, container.as_deref())
                    .await
                {
                    Ok(logs) => {
                        let lines: Vec<String> = logs.lines().map(|s| s.to_string()).collect();
                        // Keep only last MAX_LOG_LINES
                        let start = lines.len().saturating_sub(MAX_LOG_LINES);
                        log_content.set(lines[start..].to_vec());
                        // Trigger scroll to bottom after initial load
                        should_scroll.set(true);
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Error fetching logs: {}", e)));
                    }
                }
            }
            loading.set(false);
        });
    });

    // Streaming coroutine - runs continuously in background
    let pod_name_stream = props.pod_name.clone();
    let namespace_stream = props.namespace.clone();
    let container_stream = props.container.clone();

    let _stream_task = use_coroutine(move |_rx: UnboundedReceiver<()>| {
        let pod_name = pod_name_stream.clone();
        let namespace = namespace_stream.clone();
        let container = container_stream.clone();

        async move {
            // Wait for initial load to complete
            loop {
                if !*loading.read() {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            // Now start streaming
            if let Some(ctx) = cluster.read().clone() {
                let timestamps = *show_timestamps.read();
                match ctx
                    .client
                    .stream_pod_logs(
                        &pod_name,
                        &namespace,
                        container.as_deref(),
                        Some(0),
                        timestamps,
                    )
                    .await
                {
                    Ok(stream) => {
                        stream_active.set(true);
                        let mut lines = stream.lines();

                        loop {
                            let is_following = *following.read();
                            stream_active.set(is_following);

                            match lines.next_line().await {
                                Ok(Some(line)) => {
                                    // Only update display if following
                                    if is_following && !line.is_empty() {
                                        let mut current = log_content.read().clone();
                                        current.push(line);
                                        // Trim to max lines
                                        if current.len() > MAX_LOG_LINES {
                                            let start = current.len() - MAX_LOG_LINES;
                                            current = current[start..].to_vec();
                                        }
                                        log_content.set(current);

                                        // Trigger scroll
                                        should_scroll.set(true);
                                    }
                                    // When paused, we read but discard the line
                                }
                                Ok(None) => {
                                    // Stream ended (pod terminated?)
                                    tracing::info!("Log stream ended");
                                    break;
                                }
                                Err(e) => {
                                    tracing::warn!("Log stream error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to start log stream: {}", e);
                    }
                }
            }
            stream_active.set(false);
        }
    });

    // Scroll effect - triggers when should_scroll is set
    // Use peek() to avoid read+write warning - we check once and reset
    use_effect(move || {
        let do_scroll = *should_scroll.peek();
        if do_scroll {
            // Reset the flag first
            should_scroll.set(false);
            // Then scroll
            spawn(async move {
                // Longer delay to ensure DOM is fully updated
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                let _ = document::eval(
                    r#"
                    const logContent = document.querySelector('.log-content');
                    if (logContent) {
                        logContent.scrollTop = logContent.scrollHeight;
                    }
                    "#,
                )
                .await;
            });
        }
    });

    let on_back = props.on_back;
    let on_back_key = props.on_back;
    let pod_name_display = props.pod_name.clone();
    let namespace_display = props.namespace.clone();
    let container_display = props.container.clone();

    rsx! {
        div { class: "log-viewer-container",
            div { class: "log-viewer-header",
                h3 { "Logs: {pod_name_display}" }
                if let Some(c) = &container_display {
                    span { class: "container-name", "({c})" }
                }
                span { class: "container-name", "in {namespace_display}" }

                // Status indicators
                div { class: "log-status",
                    if *following.read() {
                        span { class: "status-badge status-success", "Following" }
                    } else {
                        span { class: "status-badge status-warning", "Paused" }
                    }
                    if *show_timestamps.read() {
                        span { class: "status-badge", "Timestamps" }
                    }
                }

                button {
                    class: "back-btn",
                    onclick: move |_| on_back.call(()),
                    "Back (Esc)"
                }
            }

            // Hint bar
            div { class: "log-hints",
                "f: Toggle follow • t: Timestamps • w: Wrap • ↑↓: Scroll • Esc: Back"
            }

            div { class: "log-viewer",
                if loading() {
                    div { class: "log-loading", "Loading logs..." }
                } else if let Some(err) = error_message.read().as_ref() {
                    div { class: "log-error", "{err}" }
                } else if log_content.read().is_empty() {
                    div { class: "log-empty", "No logs available" }
                } else {
                    pre {
                        class: if text_wrap() { "log-content" } else { "log-content nowrap" },
                        tabindex: 0,
                        onmounted: move |e| {
                            let data = e.data();
                            spawn(async move {
                                let _ = data.set_focus(true).await;
                                // Scroll to bottom after mount
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                let _ = document::eval(
                                    r#"
                                    const logContent = document.querySelector('.log-content');
                                    if (logContent) {
                                        logContent.scrollTop = logContent.scrollHeight;
                                    }
                                    "#,
                                )
                                .await;
                            });
                        },
                        onkeydown: move |e: KeyboardEvent| {
                            if crate::utils::is_escape(&e) {
                                on_back_key.call(());
                                e.stop_propagation();
                            } else {
                                match e.key() {
                                    Key::Character(ref c) if !e.modifiers().ctrl() && !e.modifiers().meta() => {
                                        match c.as_str() {
                                            "w" => {
                                                let new_wrap = !*text_wrap.read();
                                                text_wrap.set(new_wrap);
                                                e.stop_propagation();
                                                e.prevent_default();
                                            }
                                            "f" => {
                                                // Toggle follow mode
                                                let new_following = !*following.read();
                                                following.set(new_following);
                                                if new_following {
                                                    should_scroll.set(true);
                                                }
                                                e.stop_propagation();
                                                e.prevent_default();
                                            }
                                            "t" => {
                                                let new_timestamps = !*show_timestamps.read();
                                                show_timestamps.set(new_timestamps);
                                                e.stop_propagation();
                                                e.prevent_default();
                                            }
                                            _ => {
                                                e.stop_propagation();
                                            }
                                        }
                                    }
                                    Key::ArrowUp | Key::PageUp => {
                                        if *following.read() {
                                            following.set(false);
                                        }
                                        e.stop_propagation();
                                    }
                                    Key::ArrowDown | Key::PageDown | Key::End => {
                                        e.stop_propagation();
                                    }
                                    Key::ArrowLeft | Key::ArrowRight | Key::Home => {
                                        e.stop_propagation();
                                    }
                                    _ => {}
                                }
                            }
                        },
                        onscroll: move |_| {
                            // Could detect if user scrolled away from bottom to pause
                            // For now, only arrow keys pause
                        },
                        div {
                            for line in log_content.read().iter() {
                                {
                                    let log_class = get_log_line_class(line);
                                    rsx! {
                                        div { class: "{log_class}", "{line}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Line count indicator
            div { class: "log-footer",
                span { class: "line-count", "{log_content.read().len()} lines" }
                if *following.read() && *stream_active.read() {
                    span { class: "streaming-indicator", Circle { size: 14 } " Live" }
                }
            }
        }
    }
}

/// Determine CSS class for a log line based on log level
fn get_log_line_class(line: &str) -> &'static str {
    if line.contains("ERROR")
        || line.contains("error")
        || line.contains("Error")
        || line.contains("FATAL")
    {
        "log-line log-error"
    } else if line.contains("WARN") || line.contains("warn") || line.contains("Warning") {
        "log-line log-warn"
    } else if line.contains("INFO") || line.contains("info") {
        "log-line log-info"
    } else {
        "log-line"
    }
}
