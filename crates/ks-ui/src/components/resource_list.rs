use dioxus::prelude::*;
use std::rc::Rc;

// Virtual scrolling constants
const ROW_HEIGHT: f64 = 36.0;
const BUFFER_ROWS: usize = 20; // Large buffer to keep selection in range during navigation
// Only enable virtual scrolling for lists larger than this threshold
const VIRTUAL_SCROLL_THRESHOLD: usize = 100;

#[derive(Clone, PartialEq)]
pub struct ResourceItem {
    pub name: String,
    pub namespace: Option<String>,
    pub status: String,
    pub age: String,
    pub ready: Option<String>,
    pub restarts: Option<u32>,
}

#[derive(Props, Clone, PartialEq)]
pub struct ResourceListProps {
    pub kind: String,
    pub items: Vec<ResourceItem>,
    pub on_select: EventHandler<ResourceItem>,
    #[props(default = None)]
    pub namespace: Option<String>,
    #[props(default = Signal::new(false))]
    pub focus_search: Signal<bool>,
    #[props(default = Signal::new(false))]
    pub search_focused: Signal<bool>,
    #[props(default)]
    pub app_container_ref: Option<Signal<Option<Rc<MountedData>>>>,
    /// Currently selected index (managed by parent for cross-component state)
    #[props(default = None)]
    pub selected_index: Option<Signal<Option<usize>>>,
    /// External search term (managed by parent to persist across view changes)
    #[props(default = None)]
    pub search_term: Option<Signal<String>>,
    /// Whether this list has keyboard focus (controls selection highlight visibility)
    #[props(default = true)]
    pub is_focused: bool,
}

fn get_status_class(status: &str) -> &str {
    match status.to_lowercase().as_str() {
        "running" | "active" | "ready" | "bound" | "available" | "complete" | "succeeded"
        | "in use" => "status-success",
        "pending" | "creating" | "updating" | "progressing" | "suspended" | "released"
        | "containercreating" => "status-warning",
        "failed" | "error" | "crashloopbackoff" | "imagepullbackoff" | "notready" | "lost"
        | "errimagepull" | "invalidimagenam" | "oomkilled" | "completed" | "stalled" => {
            "status-error"
        }
        "terminating" | "deleting" => "status-terminating",
        "unused" => "status-unknown",
        _ => "status-unknown",
    }
}

#[component]
pub fn ResourceList(mut props: ResourceListProps) -> Element {
    // Use external search state if provided, otherwise use local
    let local_search = use_signal(String::new);
    let mut search = props.search_term.unwrap_or(local_search);
    let mut search_input_ref = use_signal(|| None::<Rc<MountedData>>);

    // Local selection state if not provided by parent
    let mut local_selected = use_signal(|| None::<usize>);
    let selected_index = props.selected_index.unwrap_or(local_selected);

    // Virtual scrolling state
    let mut scroll_top = use_signal(|| 0.0f64);
    let mut container_height = use_signal(|| 600.0f64);

    // Watch for focus signal from parent - this effect will re-run when signal changes
    let mut focus_sig = props.focus_search;
    let input_ref = search_input_ref;
    use_effect(move || {
        // Use read() to subscribe to changes so effect triggers on '/' keypress
        if *focus_sig.read() {
            tracing::info!("Focus effect triggered - focus_search is TRUE");
            if let Some(mounted) = input_ref.peek().clone() {
                tracing::info!("Calling set_focus() on search input");
                spawn(async move {
                    // Ignore the error - set_focus works but returns a spurious error on desktop
                    let _ = mounted.set_focus(true).await;
                    tracing::info!("Focus command sent to input");
                });
            } else {
                tracing::warn!("Input ref is None - element not mounted yet?");
            }
            // Reset asynchronously to avoid read+write warning in same reactive scope
            spawn(async move {
                focus_sig.set(false);
            });
        }
    });

    let filtered_items: Vec<ResourceItem> = props
        .items
        .iter()
        .filter(|item| {
            if search.read().is_empty() {
                true
            } else {
                item.name
                    .to_lowercase()
                    .contains(&search.read().to_lowercase())
            }
        })
        .cloned()
        .collect();

    let item_count = filtered_items.len();
    let use_virtual_scroll = item_count >= VIRTUAL_SCROLL_THRESHOLD;

    // Watch for selection changes and scroll into view
    // Works for both virtual and non-virtual since selected row is always rendered
    use_effect(move || {
        if let Some(idx) = *selected_index.read() {
            spawn(async move {
                // Small delay to ensure row is rendered
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                let js = format!(
                    r#"
                    const row = document.querySelector('[data-row-idx="{}"]');
                    if (row) {{
                        row.scrollIntoView({{ block: 'nearest' }});
                    }}
                    "#,
                    idx
                );
                let _ = document::eval(&js).await;
            });
        }
    });

    // Helper to update selection
    let mut update_selection = move |new_index: Option<usize>| {
        if let Some(mut sel) = props.selected_index {
            sel.set(new_index);
        } else {
            local_selected.set(new_index);
        }
    };

    // Check column visibility once
    let has_namespace = props.items.iter().any(|i| i.namespace.is_some());
    let has_ready = props.items.iter().any(|i| i.ready.is_some());
    let has_restarts = props.items.iter().any(|i| i.restarts.is_some());

    rsx! {
        div {
            class: "resource-list-container",
            div { class: "resource-list-header",
                div { class: "header-left",
                    h2 { class: "resource-title", "{props.kind}" }
                    if let Some(ns) = &props.namespace {
                        span { class: "namespace-badge", "namespace: {ns}" }
                    } else {
                        span { class: "namespace-badge all-namespaces", "all namespaces" }
                    }
                    span { class: "resource-count", "({item_count} items)" }
                }
                div { class: "header-right",
                    input {
                        class: "search-input",
                        r#type: "text",
                        placeholder: "Search {props.kind.to_lowercase()}... (press / to focus)",
                        oninput: move |e| {
                            tracing::debug!("Search input changed: {}", e.value());
                            search.set(e.value());
                            // Reset selection when search changes
                            update_selection(None);
                        },
                        onfocus: move |_| {
                            tracing::info!("Search input FOCUSED");
                            props.search_focused.set(true);
                        },
                        onblur: move |_| {
                            tracing::info!("Search input BLURRED");
                            props.search_focused.set(false);
                        },
                        onkeydown: move |e: KeyboardEvent| {
                            // Only handle keys if we're actually focused
                            if !*props.search_focused.read() {
                                return;
                            }

                            match e.key() {
                                Key::Escape => {
                                    search.set(String::new());
                                    let _ = document::eval("document.querySelector('.search-input').value = ''");
                                    props.search_focused.set(false);
                                    update_selection(None);

                                    // Refocus the app container so hotkeys continue to work
                                    if let Some(app_container_signal) = &props.app_container_ref
                                        && let Some(app_ref) = app_container_signal.read().clone() {
                                        spawn(async move {
                                            let _ = app_ref.set_focus(true).await;
                                        });
                                    }

                                    e.prevent_default();
                                    e.stop_propagation();
                                }
                                Key::Enter => {
                                    // Apply filter and blur - don't select resource
                                    props.search_focused.set(false);

                                    if let Some(app_container_signal) = &props.app_container_ref
                                        && let Some(app_ref) = app_container_signal.read().clone() {
                                        spawn(async move {
                                            let _ = app_ref.set_focus(true).await;
                                        });
                                    }

                                    e.prevent_default();
                                    e.stop_propagation();
                                }
                                Key::ArrowDown => {
                                    // Navigate down in filtered list
                                    let current = selected_index.read().unwrap_or(0);
                                    if !filtered_items.is_empty() {
                                        let new_idx = (current + 1).min(filtered_items.len() - 1);
                                        update_selection(Some(new_idx));
                                    }
                                    e.prevent_default();
                                    e.stop_propagation();
                                }
                                Key::ArrowUp => {
                                    // Navigate up in filtered list
                                    let current = selected_index.read().unwrap_or(0);
                                    let new_idx = current.saturating_sub(1);
                                    update_selection(Some(new_idx));
                                    e.prevent_default();
                                    e.stop_propagation();
                                }
                                _ => {
                                    // Stop propagation for all other keys to prevent hotkeys while typing
                                    e.stop_propagation();
                                }
                            }
                        },
                        onmounted: move |e| {
                            tracing::info!("Search input mounted, capturing reference");
                            search_input_ref.set(Some(e.data()));
                        },
                    }
                }
            }

            if use_virtual_scroll {
                // Virtual scrolling for large lists - uses same table structure with spacer rows
                {
                    let total_items = filtered_items.len();
                    let current_scroll = scroll_top();
                    let current_height = container_height();

                    // Calculate visible range with large buffer
                    let start_idx = ((current_scroll / ROW_HEIGHT).floor() as usize).saturating_sub(BUFFER_ROWS);
                    let visible_count = (current_height / ROW_HEIGHT).ceil() as usize + BUFFER_ROWS * 2;
                    let end_idx = (start_idx + visible_count).min(total_items);

                    // Calculate spacer heights to maintain total scroll height
                    let top_spacer_height = start_idx as f64 * ROW_HEIGHT;
                    let bottom_spacer_height = (total_items.saturating_sub(end_idx)) as f64 * ROW_HEIGHT;

                    // Get visible items with their original indices
                    let visible_items: Vec<(usize, ResourceItem)> = filtered_items
                        .iter()
                        .enumerate()
                        .skip(start_idx)
                        .take(end_idx.saturating_sub(start_idx))
                        .map(|(idx, item)| (idx, item.clone()))
                        .collect();

                    rsx! {
                        div {
                            class: "table-container",
                            onscroll: move |_e| {
                                spawn(async move {
                                    let js = r#"
                                        const container = document.querySelector('.table-container');
                                        if (container) {
                                            window.__scrollTop = container.scrollTop;
                                            window.__containerHeight = container.clientHeight;
                                        }
                                    "#;
                                    let _ = document::eval(js).await;

                                    let get_scroll = r#"window.__scrollTop || 0"#;
                                    if let Ok(val) = document::eval(get_scroll).await
                                        && let Some(scroll_val) = val.as_f64()
                                    {
                                        scroll_top.set(scroll_val);
                                    }

                                    let get_height = r#"window.__containerHeight || 600"#;
                                    if let Ok(val) = document::eval(get_height).await
                                        && let Some(height_val) = val.as_f64()
                                    {
                                        container_height.set(height_val);
                                    }
                                });
                            },
                            onmounted: move |_e| {
                                spawn(async move {
                                    let js = r#"
                                        const container = document.querySelector('.table-container');
                                        container ? container.clientHeight : 600
                                    "#;
                                    if let Ok(val) = document::eval(js).await
                                        && let Some(height) = val.as_f64()
                                    {
                                        container_height.set(height);
                                    }
                                });
                            },
                            table { class: "resource-table",
                                thead {
                                    tr {
                                        th { class: "col-status", "" }
                                        th { class: "col-name", "Name" }
                                        if has_namespace {
                                            th { class: "col-namespace", "Namespace" }
                                        }
                                        if has_ready {
                                            th { class: "col-ready", "Ready" }
                                        }
                                        th { class: "col-status-text", "Status" }
                                        if has_restarts {
                                            th { class: "col-restarts", "Restarts" }
                                        }
                                        th { class: "col-age", "Age" }
                                    }
                                }
                                tbody {
                                    // Top spacer row
                                    if top_spacer_height > 0.0 {
                                        tr { class: "spacer-row",
                                            td {
                                                colspan: "7",
                                                style: "height: {top_spacer_height}px; padding: 0; border: none;"
                                            }
                                        }
                                    }

                                    if filtered_items.is_empty() {
                                        tr { class: "empty-row",
                                            td { colspan: "7",
                                                div { class: "empty-state",
                                                    if search.read().is_empty() {
                                                        "No resources found"
                                                    } else {
                                                        "No resources match your search"
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        for (idx, item) in visible_items.iter() {
                                            {
                                                let idx = *idx;
                                                let is_selected = *selected_index.read() == Some(idx);
                                                let row_class = if is_selected && props.is_focused {
                                                    "resource-row selected"
                                                } else {
                                                    "resource-row"
                                                };

                                                rsx! {
                                                    tr {
                                                        class: "{row_class}",
                                                        "data-row-idx": "{idx}",
                                                        onclick: {
                                                            let item = item.clone();
                                                            move |_| {
                                                                update_selection(Some(idx));
                                                                props.on_select.call(item.clone());
                                                            }
                                                        },
                                                        td { class: "col-status",
                                                            span {
                                                                class: "status-dot {get_status_class(&item.status)}",
                                                                title: "{item.status}"
                                                            }
                                                        }
                                                        td { class: "col-name",
                                                            span { class: "resource-name", "{item.name}" }
                                                        }
                                                        if has_namespace {
                                                            td { class: "col-namespace",
                                                                if let Some(ns) = &item.namespace {
                                                                    span { class: "namespace-badge-small", "{ns}" }
                                                                }
                                                            }
                                                        }
                                                        if has_ready {
                                                            td { class: "col-ready",
                                                                if let Some(ready) = &item.ready {
                                                                    span { class: "ready-text", "{ready}" }
                                                                }
                                                            }
                                                        }
                                                        td { class: "col-status-text",
                                                            span { class: "status-badge {get_status_class(&item.status)}", "{item.status}" }
                                                        }
                                                        if has_restarts {
                                                            td { class: "col-restarts",
                                                                if let Some(restarts) = item.restarts {
                                                                    span {
                                                                        class: if restarts > 5 { "restart-count high" } else { "restart-count" },
                                                                        "{restarts}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        td { class: "col-age",
                                                            span { class: "age-text", "{item.age}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Bottom spacer row
                                    if bottom_spacer_height > 0.0 {
                                        tr { class: "spacer-row",
                                            td {
                                                colspan: "7",
                                                style: "height: {bottom_spacer_height}px; padding: 0; border: none;"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal table rendering for smaller lists
                div { class: "table-container",
                    table { class: "resource-table",
                        thead {
                            tr {
                                th { class: "col-status", "" }
                                th { class: "col-name", "Name" }
                                if has_namespace {
                                    th { class: "col-namespace", "Namespace" }
                                }
                                if has_ready {
                                    th { class: "col-ready", "Ready" }
                                }
                                th { class: "col-status-text", "Status" }
                                if has_restarts {
                                    th { class: "col-restarts", "Restarts" }
                                }
                                th { class: "col-age", "Age" }
                            }
                        }
                        tbody {
                            if filtered_items.is_empty() {
                                tr { class: "empty-row",
                                    td { colspan: "7",
                                        div { class: "empty-state",
                                            if search.read().is_empty() {
                                                "No resources found"
                                            } else {
                                                "No resources match your search"
                                            }
                                        }
                                    }
                                }
                            } else {
                                for (idx, item) in filtered_items.iter().enumerate() {
                                    {
                                        let is_selected = *selected_index.read() == Some(idx);
                                        let row_class = if is_selected && props.is_focused {
                                            "resource-row selected"
                                        } else {
                                            "resource-row"
                                        };

                                        rsx! {
                                            tr {
                                                class: "{row_class}",
                                                "data-row-idx": "{idx}",
                                                onclick: {
                                                    let item = item.clone();
                                                    move |_| {
                                                        update_selection(Some(idx));
                                                        props.on_select.call(item.clone());
                                                    }
                                                },
                                                td { class: "col-status",
                                                    span {
                                                        class: "status-dot {get_status_class(&item.status)}",
                                                        title: "{item.status}"
                                                    }
                                                }
                                                td { class: "col-name",
                                                    span { class: "resource-name", "{item.name}" }
                                                }
                                                if has_namespace {
                                                    td { class: "col-namespace",
                                                        if let Some(ns) = &item.namespace {
                                                            span { class: "namespace-badge-small", "{ns}" }
                                                        }
                                                    }
                                                }
                                                if has_ready {
                                                    td { class: "col-ready",
                                                        if let Some(ready) = &item.ready {
                                                            span { class: "ready-text", "{ready}" }
                                                        }
                                                    }
                                                }
                                                td { class: "col-status-text",
                                                    span { class: "status-badge {get_status_class(&item.status)}", "{item.status}" }
                                                }
                                                if has_restarts {
                                                    td { class: "col-restarts",
                                                        if let Some(restarts) = item.restarts {
                                                            span {
                                                                class: if restarts > 5 { "restart-count high" } else { "restart-count" },
                                                                "{restarts}"
                                                            }
                                                        }
                                                    }
                                                }
                                                td { class: "col-age",
                                                    span { class: "age-text", "{item.age}" }
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
    }
}
