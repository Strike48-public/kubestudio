use super::cluster_selector::Context;
use dioxus::prelude::*;
use ks_kube::CrdInfo;
use lucide_dioxus::{
    Check, ChevronDown, ChevronRight, ChevronUp, Crosshair, Globe, HardDrive, LayoutList, Lock,
    Package, Server, Settings, Tag, Wrench,
};

/// Returns a flat list of all sidebar resource keys (for keyboard navigation)
/// This matches the order items appear in the sidebar when all categories are expanded
pub fn get_all_sidebar_items() -> Vec<String> {
    vec![
        // Workloads (0-5)
        "pods".to_string(),
        "deployments".to_string(),
        "statefulsets".to_string(),
        "daemonsets".to_string(),
        "jobs".to_string(),
        "cronjobs".to_string(),
        // Configuration (6-7)
        "configmaps".to_string(),
        "secrets".to_string(),
        // Network (8-10)
        "services".to_string(),
        "endpoints".to_string(),
        "ingresses".to_string(),
        // Storage (11-13)
        "persistentvolumes".to_string(),
        "persistentvolumeclaims".to_string(),
        "storageclasses".to_string(),
        // RBAC (14-17)
        "roles".to_string(),
        "clusterroles".to_string(),
        "rolebindings".to_string(),
        "clusterrolebindings".to_string(),
        // Cluster (18-19)
        "nodes".to_string(),
        "events".to_string(),
        // CRDs start at 20+ and are handled dynamically
    ]
}

/// Returns sidebar items including CRDs (for keyboard navigation with CRD support)
pub fn get_all_sidebar_items_with_crds(crds: &[CrdInfo]) -> Vec<String> {
    let mut items = get_all_sidebar_items();
    // Add CRDs sorted by kind (matching sidebar display order)
    let mut sorted_crds = crds.to_vec();
    sorted_crds.sort_by(|a, b| a.kind.cmp(&b.kind));
    for crd in sorted_crds {
        items.push(format!("crd:{}", crd.name));
    }
    items
}

/// Returns the global index (0-19) for a given built-in resource key
/// CRD keys (starting with "crd:") get indices starting at 20
pub fn get_global_index_for_key(key: &str) -> Option<usize> {
    match key {
        "pods" => Some(0),
        "deployments" => Some(1),
        "statefulsets" => Some(2),
        "daemonsets" => Some(3),
        "jobs" => Some(4),
        "cronjobs" => Some(5),
        "configmaps" => Some(6),
        "secrets" => Some(7),
        "services" => Some(8),
        "endpoints" => Some(9),
        "ingresses" => Some(10),
        "persistentvolumes" => Some(11),
        "persistentvolumeclaims" => Some(12),
        "storageclasses" => Some(13),
        "roles" => Some(14),
        "clusterroles" => Some(15),
        "rolebindings" => Some(16),
        "clusterrolebindings" => Some(17),
        "nodes" => Some(18),
        "events" => Some(19),
        // CRD keys are handled dynamically
        _ => None,
    }
}

#[derive(Clone, PartialEq)]
pub struct ResourceCategory {
    pub name: String,
    pub icon_key: String,
    pub items: Vec<ResourceType>,
}

/// Render the Lucide icon for a given category key.
fn render_category_icon(key: &str) -> Element {
    match key {
        "workloads" => rsx! { Package { size: 16 } },
        "config" => rsx! { Settings { size: 16 } },
        "network" => rsx! { Globe { size: 16 } },
        "storage" => rsx! { HardDrive { size: 16 } },
        "rbac" => rsx! { Lock { size: 16 } },
        "cluster" => rsx! { Server { size: 16 } },
        "custom" => rsx! { Wrench { size: 16 } },
        _ => rsx! { Package { size: 16 } },
    }
}

#[derive(Clone, PartialEq)]
pub struct ResourceType {
    pub name: String,
    pub key: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct SidebarProps {
    pub namespaces: Vec<String>,
    pub selected_namespace: Option<String>,
    pub current_view: String,
    pub on_namespace_select: EventHandler<String>,
    pub on_resource_select: EventHandler<String>,
    // Cluster selector props
    pub contexts: Vec<Context>,
    pub current_context: Option<String>,
    pub on_context_select: EventHandler<String>,
    pub is_connected: Option<bool>,
    // Keyboard navigation props
    #[props(default = None)]
    pub sidebar_selected_index: Option<Signal<Option<usize>>>,
    #[props(default = false)]
    pub is_sidebar_focused: bool,
    #[props(default = Signal::new(false))]
    pub namespace_selector_focused: Signal<bool>,
    // Custom Resource Definitions
    #[props(default = vec![])]
    pub crds: Vec<CrdInfo>,
}

#[component]
pub fn Sidebar(props: SidebarProps) -> Element {
    let mut expanded_categories = use_signal(|| vec!["Workloads".to_string()]);
    let mut cluster_expanded = use_signal(|| false);
    let mut ns_expanded = use_signal(|| false);
    let mut ns_selected_idx = use_signal(|| 0usize); // 0 = "All Namespaces"
    let mut sidebar_width = use_signal(|| 280);
    let mut is_resizing = use_signal(|| false);

    // Define min and max width constraints
    const MIN_WIDTH: i32 = 180;
    const MAX_WIDTH: i32 = 600;

    // Watch for namespace selector focus signal - toggle dropdown when 'n' is pressed
    let mut ns_focused = props.namespace_selector_focused;
    use_effect(move || {
        // Read to subscribe to changes
        if *ns_focused.read() {
            // Toggle the dropdown (peek to avoid nested subscription)
            let currently_expanded = *ns_expanded.peek();
            ns_expanded.set(!currently_expanded);
            // Reset asynchronously to avoid read+write warning in same reactive scope
            spawn(async move {
                ns_focused.set(false);
            });
        }
    });

    // Helper function to get category for a sidebar index
    fn get_category_for_index(idx: usize) -> Option<&'static str> {
        match idx {
            0..=5 => Some("Workloads"), // pods, deployments, statefulsets, daemonsets, jobs, cronjobs
            6..=7 => Some("Configuration"), // configmaps, secrets
            8..=10 => Some("Network"),  // services, endpoints, ingresses
            11..=13 => Some("Storage"), // persistentvolumes, persistentvolumeclaims, storageclasses
            14..=17 => Some("RBAC"),    // roles, clusterroles, rolebindings, clusterrolebindings
            18..=19 => Some("Cluster"), // nodes, events
            _ => Some("Custom Resources"), // CRDs are at index 20+
        }
    }

    // Auto-expand category when navigating through sidebar with keyboard
    // This effect subscribes to sidebar_selected_index changes and expands the appropriate category
    let sidebar_idx_signal = props.sidebar_selected_index;
    let has_crds = !props.crds.is_empty();
    use_effect(move || {
        // Read the signal first to establish subscription (this is key for reactivity)
        if let Some(sig) = sidebar_idx_signal {
            let idx_opt = *sig.read();
            if let Some(idx) = idx_opt {
                // Only expand "Custom Resources" if we actually have CRDs
                if idx >= 20 && !has_crds {
                    return;
                }
                if let Some(cat) = get_category_for_index(idx) {
                    // Use peek() to avoid read+write on same signal in reactive scope
                    let mut current = expanded_categories.peek().clone();
                    if !current.contains(&cat.to_string()) {
                        current.push(cat.to_string());
                        expanded_categories.set(current);
                    }
                }
            }
        }
    });

    // Auto-expand the category when navigating to a resource via hotkey
    let current_view_for_expand = props.current_view.clone();
    use_effect(move || {
        let view = current_view_for_expand.clone();
        let category: Option<String> = match view.as_str() {
            "pods" | "deployments" | "statefulsets" | "daemonsets" | "jobs" | "cronjobs" => {
                Some("Workloads".to_string())
            }
            "configmaps" | "secrets" => Some("Configuration".to_string()),
            "services" | "endpoints" | "ingresses" => Some("Network".to_string()),
            "persistentvolumes" | "persistentvolumeclaims" | "storageclasses" => {
                Some("Storage".to_string())
            }
            "roles" | "clusterroles" | "rolebindings" | "clusterrolebindings" => {
                Some("RBAC".to_string())
            }
            "nodes" | "events" => Some("Cluster".to_string()),
            v if v.starts_with("crd:") => {
                // All CRDs are under "Custom Resources" category
                Some("Custom Resources".to_string())
            }
            _ => None,
        };
        if let Some(cat) = category {
            // Use peek() to avoid read+write on same signal in reactive scope
            let mut current = expanded_categories.peek().clone();
            if !current.contains(&cat) {
                current.push(cat);
                expanded_categories.set(current);
            }
        }
    });

    // Scroll selected sidebar item into view when index changes
    // No focus check needed - if index is changing, keyboard nav is active
    use_effect(move || {
        // Read signal first to subscribe to changes
        if let Some(sig) = sidebar_idx_signal {
            let idx_opt = *sig.read();
            if let Some(idx) = idx_opt {
                let js = format!(
                    r#"
                    const item = document.querySelector('[data-sidebar-idx="{}"]');
                    const sidebar = document.querySelector('.sidebar');
                    if (item && sidebar) {{
                        // Special case: first item - scroll to absolute top to show headers
                        if ({} === 0) {{
                            sidebar.scrollTop = 0;
                            return;
                        }}

                        const itemRect = item.getBoundingClientRect();
                        const sidebarRect = sidebar.getBoundingClientRect();
                        const padding = 40; // Extra padding for visibility

                        // Check if item is below visible area
                        if (itemRect.bottom + padding > sidebarRect.bottom) {{
                            sidebar.scrollTop += (itemRect.bottom + padding - sidebarRect.bottom);
                        }}
                        // Check if item is above visible area (but don't scroll past top)
                        else if (itemRect.top - padding < sidebarRect.top) {{
                            const scrollAmount = sidebarRect.top - itemRect.top + padding;
                            sidebar.scrollTop = Math.max(0, sidebar.scrollTop - scrollAmount);
                        }}
                    }}
                    "#,
                    idx, idx
                );
                spawn(async move {
                    let _ = document::eval(&js).await;
                });
            }
        }
    });

    let mut toggle_category = move |category: String| {
        let mut current = expanded_categories.read().clone();
        if current.contains(&category) {
            current.retain(|c| c != &category);
        } else {
            current.push(category);
        }
        expanded_categories.set(current);
    };

    #[allow(clippy::useless_vec)]
    let categories = vec![
        ResourceCategory {
            name: "Workloads".to_string(),
            icon_key: "workloads".to_string(),
            items: vec![
                ResourceType {
                    name: "Pods".to_string(),
                    key: "pods".to_string(),
                },
                ResourceType {
                    name: "Deployments".to_string(),
                    key: "deployments".to_string(),
                },
                ResourceType {
                    name: "StatefulSets".to_string(),
                    key: "statefulsets".to_string(),
                },
                ResourceType {
                    name: "DaemonSets".to_string(),
                    key: "daemonsets".to_string(),
                },
                ResourceType {
                    name: "Jobs".to_string(),
                    key: "jobs".to_string(),
                },
                ResourceType {
                    name: "CronJobs".to_string(),
                    key: "cronjobs".to_string(),
                },
            ],
        },
        ResourceCategory {
            name: "Configuration".to_string(),
            icon_key: "config".to_string(),
            items: vec![
                ResourceType {
                    name: "ConfigMaps".to_string(),
                    key: "configmaps".to_string(),
                },
                ResourceType {
                    name: "Secrets".to_string(),
                    key: "secrets".to_string(),
                },
            ],
        },
        ResourceCategory {
            name: "Network".to_string(),
            icon_key: "network".to_string(),
            items: vec![
                ResourceType {
                    name: "Services".to_string(),
                    key: "services".to_string(),
                },
                ResourceType {
                    name: "Endpoints".to_string(),
                    key: "endpoints".to_string(),
                },
                ResourceType {
                    name: "Ingresses".to_string(),
                    key: "ingresses".to_string(),
                },
            ],
        },
        ResourceCategory {
            name: "Storage".to_string(),
            icon_key: "storage".to_string(),
            items: vec![
                ResourceType {
                    name: "PersistentVolumes".to_string(),
                    key: "persistentvolumes".to_string(),
                },
                ResourceType {
                    name: "PersistentVolumeClaims".to_string(),
                    key: "persistentvolumeclaims".to_string(),
                },
                ResourceType {
                    name: "StorageClasses".to_string(),
                    key: "storageclasses".to_string(),
                },
            ],
        },
        ResourceCategory {
            name: "RBAC".to_string(),
            icon_key: "rbac".to_string(),
            items: vec![
                ResourceType {
                    name: "Roles".to_string(),
                    key: "roles".to_string(),
                },
                ResourceType {
                    name: "ClusterRoles".to_string(),
                    key: "clusterroles".to_string(),
                },
                ResourceType {
                    name: "RoleBindings".to_string(),
                    key: "rolebindings".to_string(),
                },
                ResourceType {
                    name: "ClusterRoleBindings".to_string(),
                    key: "clusterrolebindings".to_string(),
                },
            ],
        },
        ResourceCategory {
            name: "Cluster".to_string(),
            icon_key: "cluster".to_string(),
            items: vec![
                ResourceType {
                    name: "Nodes".to_string(),
                    key: "nodes".to_string(),
                },
                ResourceType {
                    name: "Events".to_string(),
                    key: "events".to_string(),
                },
            ],
        },
    ];

    // Handle mouse events for resizing
    let handle_mousemove = move |evt: MouseEvent| {
        if is_resizing() {
            let new_width = evt.client_coordinates().x as i32;
            let clamped_width = new_width.clamp(MIN_WIDTH, MAX_WIDTH);
            sidebar_width.set(clamped_width);
        }
    };

    let handle_mouseup = move |_evt: MouseEvent| {
        if is_resizing() {
            tracing::info!("Sidebar resize ended at width: {}px", sidebar_width());
            is_resizing.set(false);
        }
    };

    let user_select = if is_resizing() { "none" } else { "auto" };
    let sidebar_style = format!(
        "width: {}px; user-select: {};",
        sidebar_width(),
        user_select
    );

    rsx! {
        // Global overlay when resizing to capture mouse events anywhere
        if is_resizing() {
            div {
                style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; z-index: 9999; cursor: col-resize; user-select: none;",
                onmousemove: handle_mousemove,
                onmouseup: handle_mouseup,
            }
        }

        div {
            class: "sidebar-wrapper",
            style: "{sidebar_style}",

        aside {
            class: "sidebar",

            // Cluster Selector Section (now at the top)
            div { class: "sidebar-section cluster-selector-section",
                div {
                    class: "cluster-selector-header",
                    onclick: move |_| cluster_expanded.set(!cluster_expanded()),
                    div { class: "cluster-info",
                        div { class: "cluster-icon", Crosshair { size: 16 } }
                        div { class: "cluster-details",
                            div { class: "cluster-label", "Cluster" }
                            div { class: "cluster-name",
                                if let Some(ctx) = &props.current_context {
                                    "{ctx}"
                                } else {
                                    span { class: "no-cluster", "No cluster selected" }
                                }
                            }
                        }
                    }
                    div { class: "status-indicator",
                        match props.is_connected {
                            Some(true) => rsx! { span { class: "status-dot connected", title: "Connected" } },
                            Some(false) => rsx! { span { class: "status-dot disconnected", title: "Disconnected" } },
                            None => rsx! { span { class: "status-dot pending", title: "Connecting..." } },
                        }
                    }
                    div { class: "expand-icon",
                        if cluster_expanded() {
                            ChevronUp { size: 14 }
                        } else {
                            ChevronDown { size: 14 }
                        }
                    }
                }

                if cluster_expanded() {
                    div { class: "cluster-dropdown",
                        if props.contexts.is_empty() {
                            div { class: "empty-contexts",
                                "No contexts available"
                            }
                        } else {
                            for context in props.contexts.iter() {
                                div {
                                    class: if props.current_context.as_ref() == Some(&context.name) {
                                        "context-item selected"
                                    } else {
                                        "context-item"
                                    },
                                    onclick: {
                                        let name = context.name.clone();
                                        move |_| {
                                            props.on_context_select.call(name.clone());
                                            cluster_expanded.set(false);
                                        }
                                    },
                                    div { class: "context-name", "{context.name}" }
                                    if props.current_context.as_ref() == Some(&context.name) {
                                        div { class: "current-badge", Check { size: 14 } }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Namespace Selector Section
            div { class: "sidebar-section namespace-selector-section",
                div {
                    class: "namespace-selector-header",
                    onclick: move |_| ns_expanded.set(!ns_expanded()),
                    div { class: "namespace-info",
                        div { class: "namespace-icon", Tag { size: 16 } }
                        div { class: "namespace-details",
                            div { class: "namespace-label", "Namespace (n)" }
                            div { class: "namespace-name",
                                if let Some(ns) = &props.selected_namespace {
                                    "{ns}"
                                } else {
                                    "All Namespaces"
                                }
                            }
                        }
                    }
                    div { class: "expand-icon",
                        if ns_expanded() {
                            ChevronUp { size: 14 }
                        } else {
                            ChevronDown { size: 14 }
                        }
                    }
                }

                if ns_expanded() {
                    {
                        // Total items: 1 ("All Namespaces") + number of namespaces
                        let total_ns_items = 1 + props.namespaces.len();
                        let namespaces_for_select = props.namespaces.clone();

                        rsx! {
                            div {
                                class: "namespace-dropdown",
                                tabindex: 0,
                                onmounted: move |e| {
                                    let data = e.data();
                                    spawn(async move {
                                        let _ = data.set_focus(true).await;
                                    });
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    let key = match &e.key() {
                                        Key::Character(c) if c == "j" => Key::ArrowDown,
                                        Key::Character(c) if c == "k" => Key::ArrowUp,
                                        Key::Character(c) if c == "[" && e.modifiers().ctrl() => Key::Escape,
                                        other => other.clone(),
                                    };

                                    // Helper to refocus app container after closing
                                    let refocus_app = || {
                                        spawn(async move {
                                            let _ = document::eval(
                                                r#"
                                                const app = document.querySelector('.app-container');
                                                if (app) app.focus();
                                                "#
                                            ).await;
                                        });
                                    };

                                    // Helper to scroll highlighted item into view
                                    let scroll_into_view = |idx: usize| {
                                        spawn(async move {
                                            let js = format!(
                                                r#"
                                                const items = document.querySelectorAll('.namespace-item');
                                                if (items[{}]) {{
                                                    items[{}].scrollIntoView({{ block: 'nearest', behavior: 'smooth' }});
                                                }}
                                                "#,
                                                idx, idx
                                            );
                                            let _ = document::eval(&js).await;
                                        });
                                    };

                                    match key {
                                        Key::ArrowDown => {
                                            let current = *ns_selected_idx.read();
                                            if current < total_ns_items - 1 {
                                                let new_idx = current + 1;
                                                ns_selected_idx.set(new_idx);
                                                scroll_into_view(new_idx);
                                            }
                                            e.stop_propagation();
                                            e.prevent_default();
                                        }
                                        Key::ArrowUp => {
                                            let current = *ns_selected_idx.read();
                                            if current > 0 {
                                                let new_idx = current - 1;
                                                ns_selected_idx.set(new_idx);
                                                scroll_into_view(new_idx);
                                            }
                                            e.stop_propagation();
                                            e.prevent_default();
                                        }
                                        Key::Enter => {
                                            let idx = *ns_selected_idx.read();
                                            if idx == 0 {
                                                props.on_namespace_select.call(String::new());
                                            } else if let Some(ns) = namespaces_for_select.get(idx - 1) {
                                                props.on_namespace_select.call(ns.clone());
                                            }
                                            ns_expanded.set(false);
                                            refocus_app();
                                            e.stop_propagation();
                                            e.prevent_default();
                                        }
                                        Key::Escape => {
                                            ns_expanded.set(false);
                                            refocus_app();
                                            e.stop_propagation();
                                            e.prevent_default();
                                        }
                                        Key::Character(ref c) if c == "n" => {
                                            ns_expanded.set(false);
                                            refocus_app();
                                            e.stop_propagation();
                                            e.prevent_default();
                                        }
                                        _ => {
                                            e.stop_propagation();
                                        }
                                    }
                                },
                                // "All Namespaces" option
                                div {
                                    class: if *ns_selected_idx.read() == 0 {
                                        "namespace-item highlighted"
                                    } else {
                                        "namespace-item"
                                    },
                                    onclick: move |_| {
                                        props.on_namespace_select.call(String::new());
                                        ns_expanded.set(false);
                                    },
                                    div { class: "namespace-item-name", "All Namespaces" }
                                    if props.selected_namespace.is_none() {
                                        div { class: "current-badge", Check { size: 14 } }
                                    }
                                }
                                // Individual namespaces
                                for (idx, ns) in props.namespaces.iter().enumerate() {
                                    {
                                        let ns_clone = ns.clone();
                                        let is_highlighted = *ns_selected_idx.read() == idx + 1;
                                        let is_current = props.selected_namespace.as_ref() == Some(ns);
                                        rsx! {
                                            div {
                                                class: if is_highlighted {
                                                    "namespace-item highlighted"
                                                } else {
                                                    "namespace-item"
                                                },
                                                onclick: move |_| {
                                                    props.on_namespace_select.call(ns_clone.clone());
                                                    ns_expanded.set(false);
                                                },
                                                div { class: "namespace-item-name", "{ns}" }
                                                if is_current {
                                                    div { class: "current-badge", Check { size: 14 } }
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

            // Resources Categories (always show, will list from all namespaces if none selected)
            div { class: "sidebar-section",
                div { class: "sidebar-section-header",
                    h3 { LayoutList { size: 16 } " Resources" }
                }
                {
                    // Use global indices (0-19) for consistent keyboard navigation
                    let selected_idx = props.sidebar_selected_index
                        .map(|s| *s.read())
                        .unwrap_or(None);

                    rsx! {
                        for category in categories.iter() {
                            div { class: "resource-category",
                                div {
                                    class: "category-header",
                                    onclick: {
                                        let cat_name = category.name.clone();
                                        move |_| toggle_category(cat_name.clone())
                                    },
                                    span { class: "category-icon", {render_category_icon(&category.icon_key)} }
                                    span { class: "category-name", "{category.name}" }
                                    span { class: "category-toggle",
                                        if expanded_categories.read().contains(&category.name) {
                                            ChevronDown { size: 14 }
                                        } else {
                                            ChevronRight { size: 14 }
                                        }
                                    }
                                }
                                if expanded_categories.read().contains(&category.name) {
                                    ul { class: "resource-list",
                                        for item in category.items.iter() {
                                            {
                                                // Use global index for consistent keyboard nav
                                                let global_idx = get_global_index_for_key(&item.key).unwrap_or(0);
                                                let is_active = props.current_view == item.key;
                                                let is_sidebar_selected = props.is_sidebar_focused && selected_idx == Some(global_idx);
                                                let class_name = match (is_active, is_sidebar_selected) {
                                                    (true, true) => "resource-item active sidebar-selected",
                                                    (true, false) => "resource-item active",
                                                    (false, true) => "resource-item sidebar-selected",
                                                    (false, false) => "resource-item",
                                                };
                                                rsx! {
                                                    li {
                                                        class: "{class_name}",
                                                        "data-sidebar-idx": "{global_idx}",
                                                        onclick: {
                                                            let key = item.key.clone();
                                                            move |_| props.on_resource_select.call(key.clone())
                                                        },
                                                        "{item.name}"
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

            // Custom Resources Section (only show if there are CRDs)
            // All CRDs are listed under a single collapsible "Custom Resources" category
            if !props.crds.is_empty() {
                {
                    // Sort CRDs by kind for display
                    let mut sorted_crds = props.crds.clone();
                    sorted_crds.sort_by(|a, b| a.kind.cmp(&b.kind));
                    // CRD indices start at 20
                    let crd_idx_offset = 20usize;
                    // Get selected index for highlighting
                    let selected_idx = props.sidebar_selected_index
                        .map(|s| *s.read())
                        .unwrap_or(None);
                    let category_key = "Custom Resources".to_string();
                    let is_expanded = expanded_categories.read().contains(&category_key);
                    let category_key_for_toggle = category_key.clone();

                    rsx! {
                        div { class: "sidebar-section custom-resources-section",
                            div { class: "resource-category",
                                div {
                                    class: "category-header",
                                    onclick: move |_| toggle_category(category_key_for_toggle.clone()),
                                    span { class: "category-icon", Wrench { size: 16 } }
                                    span { class: "category-name", "Custom Resources" }
                                    span { class: "category-toggle",
                                        if is_expanded {
                                            ChevronDown { size: 14 }
                                        } else {
                                            ChevronRight { size: 14 }
                                        }
                                    }
                                }
                                if is_expanded {
                                    ul { class: "resource-list",
                                        for (idx, crd) in sorted_crds.iter().enumerate() {
                                            {
                                                let crd_key = format!("crd:{}", crd.name);
                                                let global_idx = crd_idx_offset + idx;
                                                let is_active = props.current_view == crd_key;
                                                let is_sidebar_selected = props.is_sidebar_focused && selected_idx == Some(global_idx);
                                                let class_name = match (is_active, is_sidebar_selected) {
                                                    (true, true) => "resource-item active sidebar-selected",
                                                    (true, false) => "resource-item active",
                                                    (false, true) => "resource-item sidebar-selected",
                                                    (false, false) => "resource-item",
                                                };
                                                let crd_key_click = crd_key.clone();
                                                let display_name = crd.kind.clone();
                                                rsx! {
                                                    li {
                                                        class: "{class_name}",
                                                        "data-sidebar-idx": "{global_idx}",
                                                        title: "{crd.group}",
                                                        onclick: move |_| props.on_resource_select.call(crd_key_click.clone()),
                                                        "{display_name}"
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

        // Resize handle — on the wrapper so it's not clipped by sidebar scroll
        div {
            class: "sidebar-resize-handle",
            style: "cursor: col-resize;",
            onmousedown: move |evt| {
                tracing::info!("Sidebar resize started");
                is_resizing.set(true);
                evt.stop_propagation();
            }
        }
        }
    }
}
