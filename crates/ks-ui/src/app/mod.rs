mod crd_items;
mod helpers;
mod hotkeys;
mod resource_items;

pub use crd_items::dynamic_objects_to_items;
pub use helpers::format_age;
pub use hotkeys::{get_commands, get_commands_with_tools, get_hotkeys};
pub use resource_items::*;

use crate::components::{
    ActionConfirmModal, ActionTarget, ActionType, ApplyManifest, ApplySource, ChatPanel,
    ClusterOverview, CommandPalette, ContainerDrillDown, Context, CreateResource,
    CronJobJobsDrillDown, DeleteModal, DeleteTarget, ExecViewer, HotkeysBar, LogViewer, NodeList,
    NodeSortState, PortForwardModal, PortForwardsList, PvcPodsDrillDown, ResourceItem,
    ResourceList, ServicePodsDrillDown, Sidebar, SortState, WorkloadPodsDrillDown, YamlViewer,
    get_all_sidebar_items_with_crds, get_overview_card_count, get_overview_card_target,
};
use crate::hooks::{
    ActivePortForward, FocusZone, ViewState, use_cluster, use_connect_cluster, use_focus,
    use_navigation, use_node_metrics, use_port_forwards, use_watch_clusterrolebindings,
    use_watch_clusterroles, use_watch_configmaps, use_watch_crd_instances, use_watch_crds,
    use_watch_cronjobs, use_watch_daemonsets, use_watch_deployments, use_watch_endpoints,
    use_watch_events, use_watch_ingresses, use_watch_jobs, use_watch_nodes,
    use_watch_persistentvolumeclaims, use_watch_persistentvolumes, use_watch_pods,
    use_watch_rolebindings, use_watch_roles, use_watch_secrets, use_watch_services,
    use_watch_statefulsets, use_watch_storageclasses,
};
use crate::utils::is_escape;
use dioxus::prelude::*;
use ks_kube::CrdInfo;
use ks_kube::PermissionMode;
use ks_kube::auth;
use ks_plugin::PluginConfig;
use ks_state::Store;
use lucide_dioxus::{ArrowRight, CircleX, CornerDownLeft, Loader, Moon, Sun, X};
use std::rc::Rc;
use std::sync::Arc;

// NOTE: The App component remains large due to Dioxus signal capture requirements.
// The following have been extracted to separate modules for clarity:
// - helpers.rs: format_age utility
// - hotkeys.rs: get_commands, get_hotkeys
// - resource_items.rs: all *_to_items conversion functions
//
// The keyboard handler, get_current_items, and RSX remain inline because they
// require mutable signal captures that don't work well with function extraction.

#[component]
pub fn App() -> Element {
    // Global state
    let _store = use_context_provider(|| Arc::new(Store::new()));

    // Cluster connection state
    let cluster = use_cluster();
    let connect_cluster = use_connect_cluster(cluster);

    // UI state
    let mut selected_namespace = use_signal(|| None::<String>);
    let mut namespaces = use_signal(Vec::<String>::new);
    let mut command_palette_open = use_signal(|| false);
    let mut current_view = use_signal(|| "overview".to_string());
    let mut previous_view = use_signal(|| "overview".to_string());
    let mut loading = use_signal(|| true);
    let mut error_message = use_signal(|| None::<String>);
    let mut plugin_error = use_signal(|| None::<String>);
    let mut available_contexts = use_signal(Vec::<Context>::new);
    let mut focus_search = use_signal(|| false);
    let mut search_is_focused = use_signal(|| false);
    let mut app_container_ref = use_signal(|| None::<Rc<MountedData>>);

    // Selection and action state
    let mut selected_index = use_signal(|| None::<usize>);
    let mut selected_resource = use_signal(|| None::<ResourceItem>);
    let mut nav = use_navigation();

    // Container drill-down state
    let mut selected_container_index = use_signal(|| None::<usize>);
    let mut selected_drilldown_pod = use_signal(|| None::<(String, String)>);

    // Delete modal state
    let mut delete_modal_open = use_signal(|| false);
    let mut delete_target = use_signal(|| None::<DeleteTarget>);
    let mut is_force_delete = use_signal(|| false);

    // Port-forward modal state
    let mut portforward_modal_open = use_signal(|| false);
    let mut portforward_target =
        use_signal(|| None::<(String, String, Vec<(u16, Option<String>, String)>)>);

    // Action confirm modal state
    let mut action_modal_open = use_signal(|| false);
    let mut action_modal_type = use_signal(|| ActionType::Restart);
    let mut action_target = use_signal(|| None::<ActionTarget>);

    // Help modal state
    let mut help_modal_open = use_signal(|| false);

    // Theme state — class-based dark/light toggle
    let mut is_dark = use_signal(|| true);

    // Sync is_dark with the actual DOM state on mount
    use_effect(move || {
        spawn(async move {
            if let Ok(val) =
                document::eval("return document.documentElement.classList.contains('dark')").await
                && let Some(dark) = val.as_bool()
            {
                is_dark.set(dark);
            }
        });
    });

    // Toggle theme closure
    let mut toggle_theme = move |_: ()| {
        let new_dark = !*is_dark.peek();
        is_dark.set(new_dark);
        spawn(async move {
            if new_dark {
                let _ = document::eval(
                    "document.documentElement.classList.add('dark'); localStorage.setItem('theme','dark');"
                ).await;
            } else {
                let _ = document::eval(
                    "document.documentElement.classList.remove('dark'); localStorage.setItem('theme','light');"
                ).await;
            }
        });
    };

    // AI feature flag: enabled when STRIKE48_API_URL is set and KUBESTUDIO_AI != "false"
    let matrix_api_url = use_signal(|| std::env::var("STRIKE48_API_URL").unwrap_or_default());
    let ai_enabled = use_signal(|| {
        let has_url = !std::env::var("STRIKE48_API_URL")
            .unwrap_or_default()
            .is_empty();
        let not_disabled = std::env::var("KUBESTUDIO_AI")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);
        has_url && not_disabled
    });

    // Store tenant_id in the session so ChatPanel can read it when seeding the agent.
    // The ks-connector binary also sets this when running in-process.
    {
        let tenant = std::env::var("STRIKE48_TENANT").unwrap_or_default();
        if !tenant.is_empty() {
            crate::session::set_tenant_id(&tenant);
        }
    }

    // Chat panel state (only used when ai_enabled)
    let mut chat_panel_open = use_signal(|| false);
    let mut pending_chat_message = use_signal(|| None::<String>);
    let mut matrix_auth_token = use_signal(|| {
        // Check synchronous sources: 1) session token (connector __st), 2) env var
        let session_token = crate::session::get_auth_token();
        if !session_token.is_empty() {
            tracing::info!(
                "Auth token from session store (length: {})",
                session_token.len()
            );
            return session_token;
        }
        let env_token = std::env::var("MATRIX_AUTH_TOKEN").unwrap_or_default();
        if !env_token.is_empty() {
            tracing::info!(
                "Auth token from MATRIX_AUTH_TOKEN env var (length: {})",
                env_token.len()
            );
        }
        env_token
    });

    // Pick up sandbox token from browser global injected by Matrix Studio.
    // Matrix injects window.__MATRIX_SESSION_TOKEN__ into connector iframes
    // and auto-refreshes it at ~70% TTL. We poll periodically to stay in sync.
    {
        use_effect(move || {
            let is_ai = ai_enabled();
            if !is_ai {
                return;
            }
            spawn(async move {
                loop {
                    if let Ok(val) =
                        document::eval("return window.__MATRIX_SESSION_TOKEN__ || ''").await
                        && let Some(token) = val.as_str()
                    {
                        let token = token.trim();
                        let current = crate::session::get_auth_token();
                        if !token.is_empty() && token != current {
                            tracing::info!(
                                "Picked up sandbox token from browser (length: {})",
                                token.len()
                            );
                            crate::session::set_auth_token(token);
                            matrix_auth_token.set(token.to_string());
                        }
                    }
                    // Poll every 30s to pick up tokens refreshed by Matrix injection JS
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            });
        });
    }

    // Command mode state (k9s-style ":" commands)
    let mut command_mode_open = use_signal(|| false);
    let mut command_input = use_signal(String::new);

    // Connection health state: None = pending (first check not done), Some(bool) = known
    let mut connection_healthy: Signal<Option<bool>> = use_signal(|| None);

    // Permission mode (read from KUBESTUDIO_MODE env var)
    // ReadOnly = disables write operations (shell, delete, scale, etc.)
    let permission_mode = use_signal(|| {
        std::env::var("KUBESTUDIO_MODE")
            .map(|s| PermissionMode::from_str(&s))
            .unwrap_or_default()
    });
    let is_read_only = move || permission_mode() == PermissionMode::ReadOnly;

    // Search state
    let search_term = use_signal(String::new);

    // Sort state for resource list and node list
    let sort_state = use_signal(SortState::default);
    let node_sort_state = use_signal(NodeSortState::default);

    // Focus zone tracking
    let mut focus = use_focus();
    let mut sidebar_selected_index = use_signal(|| None::<usize>);
    let mut namespace_selector_focused = use_signal(|| false);
    let mut keyboard_nav_active = use_signal(|| false);
    let mut sidebar_visible = use_signal(|| true);

    // Resources (watch-based for real-time updates)
    let pods = use_watch_pods(cluster, selected_namespace);
    let deployments = use_watch_deployments(cluster, selected_namespace);
    let statefulsets = use_watch_statefulsets(cluster, selected_namespace);
    let daemonsets = use_watch_daemonsets(cluster, selected_namespace);
    let jobs = use_watch_jobs(cluster, selected_namespace);
    let cronjobs = use_watch_cronjobs(cluster, selected_namespace);
    let configmaps = use_watch_configmaps(cluster, selected_namespace);
    let secrets = use_watch_secrets(cluster, selected_namespace);
    let services = use_watch_services(cluster, selected_namespace);
    let endpoints = use_watch_endpoints(cluster, selected_namespace);
    let persistentvolumes = use_watch_persistentvolumes(cluster);
    let persistentvolumeclaims = use_watch_persistentvolumeclaims(cluster, selected_namespace);
    let ingresses = use_watch_ingresses(cluster, selected_namespace);
    let storageclasses = use_watch_storageclasses(cluster);
    let nodes = use_watch_nodes(cluster);
    let events = use_watch_events(cluster, selected_namespace);

    // Metrics from metrics-server (optional - polls every 30s)
    let node_metrics = use_node_metrics(cluster);

    // RBAC resources
    let roles = use_watch_roles(cluster, selected_namespace);
    let clusterroles = use_watch_clusterroles(cluster);
    let rolebindings = use_watch_rolebindings(cluster, selected_namespace);
    let clusterrolebindings = use_watch_clusterrolebindings(cluster);

    // CRD support
    let crd_state = use_watch_crds(cluster);
    let mut selected_crd = use_signal(|| None::<CrdInfo>);
    let crd_instances = use_watch_crd_instances(cluster, selected_namespace, selected_crd);

    // Plugin config (loaded from ~/.config/kubestudio/config.yaml)
    let mut plugin_config = use_signal(PluginConfig::with_defaults);

    // Load plugin config on startup
    use_effect(move || match ks_plugin::load_config() {
        Ok(config) => {
            tracing::info!(
                "Loaded plugin config with {} aliases, {} hotkeys, {} tools",
                config.aliases.len(),
                config.hotkeys.len(),
                config.tools.len()
            );
            plugin_config.set(config);
        }
        Err(e) => {
            tracing::warn!("Failed to load plugin config, using defaults: {}", e);
        }
    });

    // Port-forward state management
    let mut port_forwards = use_port_forwards();

    // Initialize: Load kubeconfig and connect to current context
    use_effect(move || {
        spawn(async move {
            tracing::info!("Initializing app, loading kubeconfig...");
            match auth::load_kubeconfig(None).await {
                Ok(config) => {
                    let contexts = auth::list_contexts(&config);
                    let current = auth::current_context(&config);

                    tracing::info!("Found {} contexts", contexts.len());

                    let context_list: Vec<Context> = contexts
                        .into_iter()
                        .map(|name| Context {
                            is_current: current.as_ref() == Some(&name),
                            name,
                        })
                        .collect();
                    available_contexts.set(context_list);

                    if let Some(current_ctx) = current {
                        tracing::info!("Current context: {}", current_ctx);
                        connect_cluster(current_ctx);
                    } else if let Some(first_ctx) = available_contexts.read().first() {
                        tracing::info!("No current context, using first: {}", first_ctx.name);
                        connect_cluster(first_ctx.name.clone());
                    } else {
                        error_message.set(Some("No contexts found in kubeconfig".to_string()));
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to load kubeconfig: {}", e);
                    error_message.set(Some(format!(
                        "Failed to load kubeconfig: {}. Make sure kubectl is configured.",
                        e
                    )));
                }
            }
            loading.set(false);
        });
    });

    // Load namespaces when cluster is connected
    use_effect(move || {
        let cluster_ctx = cluster.read().clone();

        if let Some(ctx) = cluster_ctx {
            error_message.set(None);
            connection_healthy.set(None);

            spawn(async move {
                tracing::info!("Cluster connected, loading namespaces...");
                match ctx.client.list_namespaces().await {
                    Ok(ns_list) => {
                        tracing::info!("Successfully loaded {} namespaces", ns_list.len());
                        namespaces.set(ns_list);
                        error_message.set(None);
                        connection_healthy.set(Some(true));
                    }
                    Err(e) => {
                        tracing::error!("Failed to load namespaces: {}", e);
                        error_message.set(Some(format!("Failed to connect to cluster: {}", e)));
                        connection_healthy.set(Some(false));
                    }
                }
            });
        } else {
            connection_healthy.set(None);
            namespaces.set(Vec::new());
            selected_namespace.set(None);
        }
    });

    // Helper to get current items based on view (uses extracted resource_items module)
    // Note: This closure remains inline due to signal capture requirements
    let get_current_items = move || -> Vec<ResourceItem> {
        let term = search_term.read().to_lowercase();
        let view = current_view.read().clone();
        let items: Vec<ResourceItem> = match view.as_str() {
            "pods" => pods_to_items(&pods.read().items),
            "deployments" => deployments_to_items(&deployments.read().items),
            "statefulsets" => statefulsets_to_items(&statefulsets.read().items),
            "daemonsets" => daemonsets_to_items(&daemonsets.read().items),
            "jobs" => jobs_to_items(&jobs.read().items),
            "cronjobs" => cronjobs_to_items(&cronjobs.read().items),
            "configmaps" => configmaps_to_items(&configmaps.read().items),
            "secrets" => secrets_to_items(&secrets.read().items, &pods.read().items),
            "services" => services_to_items(&services.read().items, &endpoints.read().items),
            "endpoints" => endpoints_to_items(&endpoints.read().items),
            "persistentvolumes" => persistentvolumes_to_items(&persistentvolumes.read().items),
            "persistentvolumeclaims" => {
                persistentvolumeclaims_to_items(&persistentvolumeclaims.read().items)
            }
            "ingresses" => ingresses_to_items(&ingresses.read().items),
            "storageclasses" => storageclasses_to_items(&storageclasses.read().items),
            "events" => events_to_items(&events.read().items),
            "nodes" => nodes_to_items(&nodes.read().items),
            "roles" => roles_to_items(&roles.read().items),
            "clusterroles" => clusterroles_to_items(&clusterroles.read().items),
            "rolebindings" => rolebindings_to_items(&rolebindings.read().items),
            "clusterrolebindings" => {
                clusterrolebindings_to_items(&clusterrolebindings.read().items)
            }
            v if v.starts_with("crd:") => {
                // Handle CRD views
                if let Some(crd) = selected_crd.read().as_ref() {
                    dynamic_objects_to_items(&crd_instances.read().items, crd)
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };
        if term.is_empty() {
            items
        } else {
            items
                .into_iter()
                .filter(|item| item.name.to_lowercase().contains(&term))
                .collect()
        }
    };

    // Keyboard shortcuts - remains inline due to signal capture requirements
    let onkeydown = move |e: KeyboardEvent| {
        let is_focused = search_is_focused();

        // Map vim j/k to arrow keys, Ctrl+[ to Escape (VT100)
        let effective_key = match &e.key() {
            Key::Character(c) if c == "j" => Key::ArrowDown,
            Key::Character(c) if c == "k" => Key::ArrowUp,
            Key::Character(c) if c == "[" && e.modifiers().ctrl() => Key::Escape,
            other => other.clone(),
        };

        // Don't process hotkeys if a modal is open
        if delete_modal_open()
            || portforward_modal_open()
            || action_modal_open()
            || help_modal_open()
        {
            return;
        }

        // Special handling for Escape key when search is focused
        if effective_key == Key::Escape && is_focused {
            search_is_focused.set(false);
            e.stop_propagation();
            return;
        }

        // Don't process other hotkeys if user is typing in search input
        if is_focused {
            e.stop_propagation();
            return;
        }

        // Handle namespace selector escape
        if namespace_selector_focused() {
            if effective_key == Key::Escape {
                namespace_selector_focused.set(false);
                if let Some(app_ref) = app_container_ref.read().clone() {
                    spawn(async move {
                        let _ = app_ref.set_focus(true).await;
                    });
                }
                e.stop_propagation();
            }
            return;
        }

        if command_palette_open() {
            if effective_key == Key::Escape {
                command_palette_open.set(false);
                e.stop_propagation();
            }
            return;
        }

        // Handle command mode (k9s-style ":" commands for aliases)
        if command_mode_open() {
            match effective_key {
                Key::Escape => {
                    command_mode_open.set(false);
                    command_input.set(String::new());
                    // Refocus app container
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                    e.stop_propagation();
                    e.prevent_default();
                }
                Key::Enter => {
                    let input = command_input.read().clone();
                    command_mode_open.set(false);
                    command_input.set(String::new());

                    // Try to resolve as alias (or use input directly if it's a valid view)
                    let resolved = plugin_config.read().resolve_alias(&input).cloned();
                    let target = resolved.unwrap_or_else(|| input.clone());

                    // Check if this is a valid view by looking in sidebar items
                    let sidebar_items = get_all_sidebar_items_with_crds(&crd_state.read().crds);
                    if let Some(idx) = sidebar_items.iter().position(|k| k == &target) {
                        tracing::info!("Command mode: navigating to '{}' (index {})", target, idx);
                        previous_view.set(current_view.read().clone());
                        current_view.set(target.clone());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);

                        // Set sidebar index for seamless keyboard navigation
                        sidebar_selected_index.set(Some(idx));

                        // Handle CRD navigation
                        if let Some(crd_name) = target.strip_prefix("crd:") {
                            let crd = crd_state
                                .read()
                                .crds
                                .iter()
                                .find(|c| c.name == crd_name)
                                .cloned();
                            selected_crd.set(crd);
                        } else {
                            selected_crd.set(None);
                        }
                    } else {
                        tracing::warn!("Command mode: unknown command '{}'", input);
                    }

                    // Refocus app container
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                    e.stop_propagation();
                    e.prevent_default();
                }
                Key::Backspace => {
                    let mut current = command_input.read().clone();
                    current.pop();
                    command_input.set(current);
                    e.stop_propagation();
                    e.prevent_default();
                }
                Key::Character(ref c) => {
                    // Append character to command input
                    let mut current = command_input.read().clone();
                    current.push_str(c);
                    command_input.set(current);
                    e.stop_propagation();
                    e.prevent_default();
                }
                _ => {
                    e.stop_propagation();
                }
            }
            return;
        }

        // Get the current view's kind for actions
        let view = current_view.read().clone();
        let current_kind: String = match view.as_str() {
            "pods" => "Pod".to_string(),
            "deployments" => "Deployment".to_string(),
            "services" => "Service".to_string(),
            "statefulsets" => "StatefulSet".to_string(),
            "daemonsets" => "DaemonSet".to_string(),
            "jobs" => "Job".to_string(),
            "cronjobs" => "CronJob".to_string(),
            "configmaps" => "ConfigMap".to_string(),
            "secrets" => "Secret".to_string(),
            "endpoints" => "Endpoints".to_string(),
            "ingresses" => "Ingress".to_string(),
            "persistentvolumes" => "PersistentVolume".to_string(),
            "persistentvolumeclaims" => "PersistentVolumeClaim".to_string(),
            "storageclasses" => "StorageClass".to_string(),
            "events" => "Event".to_string(),
            "nodes" => "Node".to_string(),
            "roles" => "Role".to_string(),
            "clusterroles" => "ClusterRole".to_string(),
            "rolebindings" => "RoleBinding".to_string(),
            "clusterrolebindings" => "ClusterRoleBinding".to_string(),
            v if v.starts_with("crd:") => {
                // For CRD views, use the kind from the selected CRD
                selected_crd
                    .read()
                    .as_ref()
                    .map(|c| c.kind.clone())
                    .unwrap_or_default()
            }
            _ => String::new(),
        };

        // Check for custom hotkeys from plugin config
        if let Key::Character(ref key_char) = e.key() {
            let config = plugin_config.read();
            for hotkey in &config.hotkeys {
                let parsed = ks_plugin::ParsedHotkey::parse(&hotkey.key);
                if parsed.matches(
                    key_char,
                    e.modifiers().ctrl(),
                    e.modifiers().shift(),
                    e.modifiers().alt(),
                    e.modifiers().meta(),
                ) {
                    // Check if selection is required
                    if hotkey.requires_selection && selected_resource.read().is_none() {
                        continue;
                    }

                    // Build template context — prefer global ns, fall back to resource's ns
                    let ns = selected_namespace.read().clone().or_else(|| {
                        selected_resource
                            .read()
                            .as_ref()
                            .and_then(|r| r.namespace.clone())
                    });
                    let ctx = ks_plugin::TemplateContext {
                        namespace: ns,
                        name: selected_resource.read().as_ref().map(|r| r.name.clone()),
                        kind: Some(current_kind.clone()),
                        context: cluster.read().as_ref().map(|c| c.context_name.clone()),
                    };

                    // Execute the hotkey command
                    if let Err(err) = ks_plugin::execute_hotkey(hotkey, &ctx) {
                        tracing::error!("Failed to execute custom hotkey: {}", err);
                        plugin_error.set(Some(format!("Plugin hotkey failed: {}", err)));
                    }
                    e.stop_propagation();
                    e.prevent_default();
                    return;
                }
            }
        }

        match effective_key {
            // Arrow Left = Navigate left (sidebar or overview cards)
            Key::ArrowLeft => {
                keyboard_nav_active.set(true);
                let is_overview = current_view.read().as_str() == "overview";

                if focus.read().zone() == FocusZone::MainList {
                    if is_overview {
                        let current_idx = *selected_index.read();
                        match current_idx {
                            Some(0) | None => {
                                if sidebar_visible() {
                                    focus.write().set_zone(FocusZone::Sidebar);
                                    selected_index.set(None);
                                    if sidebar_selected_index.read().is_none() {
                                        sidebar_selected_index.set(Some(0));
                                    }
                                }
                            }
                            Some(idx) => {
                                selected_index.set(Some(idx - 1));
                            }
                        }
                    } else if sidebar_visible() {
                        focus.write().set_zone(FocusZone::Sidebar);
                        if sidebar_selected_index.read().is_none() {
                            let sidebar_items =
                                get_all_sidebar_items_with_crds(&crd_state.read().crds);
                            let current = current_view.read().clone();
                            if let Some(idx) = sidebar_items.iter().position(|k| *k == current) {
                                sidebar_selected_index.set(Some(idx));
                            } else {
                                sidebar_selected_index.set(Some(0));
                            }
                        }
                    }
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Arrow Right = Navigate right (to main content or through overview cards)
            Key::ArrowRight => {
                keyboard_nav_active.set(true);
                let is_overview = current_view.read().as_str() == "overview";

                if focus.read().zone() == FocusZone::Sidebar {
                    focus.write().set_zone(FocusZone::MainList);
                    if is_overview {
                        selected_index.set(Some(0));
                    }
                } else if focus.read().zone() == FocusZone::MainList && is_overview {
                    let card_count = get_overview_card_count();
                    let current_idx = selected_index.read().unwrap_or(0);
                    if current_idx < card_count - 1 {
                        selected_index.set(Some(current_idx + 1));
                    }
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Arrow key / vim j navigation (context-aware)
            Key::ArrowDown => {
                keyboard_nav_active.set(true);
                match focus.read().zone() {
                    FocusZone::Sidebar => {
                        let sidebar_items = get_all_sidebar_items_with_crds(&crd_state.read().crds);
                        if !sidebar_items.is_empty() {
                            let new_idx = match *sidebar_selected_index.read() {
                                None => 0,
                                Some(current) => (current + 1).min(sidebar_items.len() - 1),
                            };
                            sidebar_selected_index.set(Some(new_idx));
                        }
                    }
                    FocusZone::MainList => {
                        let current_state = nav.read().current().clone();
                        match current_state {
                            ViewState::ContainerDrillDown {
                                ref pod_name,
                                ref namespace,
                            } => {
                                if let Some(pod) = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(pod_name)
                                        && p.metadata.namespace.as_ref() == Some(namespace)
                                }) && let Some(spec) = &pod.spec
                                {
                                    let container_count = spec.containers.len();
                                    if container_count > 0 {
                                        let new_idx = match *selected_container_index.read() {
                                            None => 0,
                                            Some(current) => (current + 1).min(container_count - 1),
                                        };
                                        selected_container_index.set(Some(new_idx));
                                    }
                                }
                            }
                            ViewState::DeploymentPods { .. }
                            | ViewState::StatefulSetPods { .. }
                            | ViewState::DaemonSetPods { .. }
                            | ViewState::JobPods { .. }
                            | ViewState::CronJobJobs { .. }
                            | ViewState::ServiceEndpoints { .. }
                            | ViewState::PvcPods { .. } => {
                                let new_idx = match *selected_container_index.read() {
                                    None => 0,
                                    Some(current) => current + 1,
                                };
                                selected_container_index.set(Some(new_idx));
                            }
                            _ => {
                                let view = current_view.read().clone();
                                match view.as_str() {
                                    "overview" => {}
                                    "portforwards" => {
                                        let count = port_forwards.read().count();
                                        if count > 0 {
                                            let new_idx = match *selected_index.read() {
                                                None => 0,
                                                Some(current) => (current + 1).min(count - 1),
                                            };
                                            selected_index.set(Some(new_idx));
                                        }
                                    }
                                    _ => {
                                        let items = get_current_items();
                                        if !items.is_empty() {
                                            let new_idx = match *selected_index.read() {
                                                None => 0,
                                                Some(current) => (current + 1).min(items.len() - 1),
                                            };
                                            selected_index.set(Some(new_idx));
                                            selected_resource.set(Some(items[new_idx].clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                e.stop_propagation();
                e.prevent_default();
            }
            Key::ArrowUp => {
                keyboard_nav_active.set(true);
                match focus.read().zone() {
                    FocusZone::Sidebar => {
                        let sidebar_items = get_all_sidebar_items_with_crds(&crd_state.read().crds);
                        if !sidebar_items.is_empty() {
                            let current = sidebar_selected_index.read().unwrap_or(0);
                            let new_idx = current.saturating_sub(1);
                            sidebar_selected_index.set(Some(new_idx));
                        }
                    }
                    FocusZone::MainList => {
                        let current_state = nav.read().current().clone();
                        match current_state {
                            ViewState::ContainerDrillDown {
                                ref pod_name,
                                ref namespace,
                            } => {
                                if let Some(pod) = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(pod_name)
                                        && p.metadata.namespace.as_ref() == Some(namespace)
                                }) && pod.spec.is_some()
                                {
                                    let current = selected_container_index.read().unwrap_or(0);
                                    let new_idx = current.saturating_sub(1);
                                    selected_container_index.set(Some(new_idx));
                                }
                            }
                            ViewState::DeploymentPods { .. }
                            | ViewState::StatefulSetPods { .. }
                            | ViewState::DaemonSetPods { .. }
                            | ViewState::JobPods { .. }
                            | ViewState::CronJobJobs { .. }
                            | ViewState::ServiceEndpoints { .. }
                            | ViewState::PvcPods { .. } => {
                                let current = selected_container_index.read().unwrap_or(0);
                                let new_idx = current.saturating_sub(1);
                                selected_container_index.set(Some(new_idx));
                            }
                            _ => {
                                let view = current_view.read().clone();
                                match view.as_str() {
                                    "overview" => {}
                                    "portforwards" => {
                                        let count = port_forwards.read().count();
                                        if count > 0 {
                                            let current = selected_index.read().unwrap_or(0);
                                            let new_idx = current.saturating_sub(1);
                                            selected_index.set(Some(new_idx));
                                        }
                                    }
                                    _ => {
                                        let items = get_current_items();
                                        if !items.is_empty() {
                                            let current = selected_index.read().unwrap_or(0);
                                            let new_idx = current.saturating_sub(1);
                                            selected_index.set(Some(new_idx));
                                            selected_resource.set(Some(items[new_idx].clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Enter = Select overview card when in overview
            Key::Enter
                if current_view.read().as_str() == "overview"
                    && focus.read().zone() == FocusZone::MainList =>
            {
                let idx_opt = *selected_index.read();
                if let Some(idx) = idx_opt
                    && let Some(target) = get_overview_card_target(idx)
                {
                    current_view.set(target.to_string());
                    nav.write().reset(ViewState::ResourceList);
                    selected_index.set(None);
                    selected_resource.set(None);
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Enter = Select sidebar item when in sidebar focus
            Key::Enter if focus.read().zone() == FocusZone::Sidebar => {
                if let Some(idx) = *sidebar_selected_index.read() {
                    let sidebar_items = get_all_sidebar_items_with_crds(&crd_state.read().crds);
                    if let Some(key) = sidebar_items.get(idx) {
                        // Update selected_crd if this is a CRD view
                        if let Some(crd_name) = key.strip_prefix("crd:") {
                            let crd = crd_state
                                .read()
                                .crds
                                .iter()
                                .find(|c| c.name == crd_name)
                                .cloned();
                            selected_crd.set(crd);
                        } else {
                            selected_crd.set(None);
                        }
                        current_view.set(key.clone());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        focus.write().set_zone(FocusZone::MainList);
                    }
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Ctrl+D = Delete with confirmation
            Key::Character(ref c) if c == "d" && e.modifiers().ctrl() => {
                if is_read_only() {
                    tracing::warn!("Delete disabled in read-only mode (KUBESTUDIO_MODE=read)");
                    e.stop_propagation();
                    e.prevent_default();
                    return;
                }
                if current_view.read().as_str() == "portforwards" {
                    let idx_opt = *selected_index.read();
                    if let Some(idx) = idx_opt {
                        let forwards = port_forwards.read().list();
                        if let Some(forward) = forwards.get(idx) {
                            delete_target.set(Some(DeleteTarget {
                                name: forward.id.clone(),
                                namespace: Some(forward.namespace.clone()),
                                kind: "PortForward".to_string(),
                            }));
                            is_force_delete.set(false);
                            delete_modal_open.set(true);
                        }
                    }
                } else if let Some(resource) = selected_resource.read().clone() {
                    delete_target.set(Some(DeleteTarget {
                        name: resource.name.clone(),
                        namespace: resource.namespace.clone(),
                        kind: current_kind.to_string(),
                    }));
                    is_force_delete.set(false);
                    delete_modal_open.set(true);
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Ctrl+K = Kill (force delete)
            Key::Character(ref c) if c == "k" && e.modifiers().ctrl() => {
                if is_read_only() {
                    tracing::warn!(
                        "Force delete disabled in read-only mode (KUBESTUDIO_MODE=read)"
                    );
                    e.stop_propagation();
                    e.prevent_default();
                    return;
                }
                if let Some(resource) = selected_resource.read().clone() {
                    delete_target.set(Some(DeleteTarget {
                        name: resource.name.clone(),
                        namespace: resource.namespace.clone(),
                        kind: current_kind.to_string(),
                    }));
                    is_force_delete.set(true);
                    delete_modal_open.set(true);
                }
                e.stop_propagation();
                e.prevent_default();
            }
            // Ctrl+I = Command palette
            Key::Character(ref c) if c == "i" && e.modifiers().ctrl() => {
                command_palette_open.set(true);
                e.stop_propagation();
                e.prevent_default();
            }
            // Ctrl+O = Open file to apply (desktop only)
            #[cfg(feature = "desktop")]
            Key::Character(ref c) if c == "o" && e.modifiers().ctrl() => {
                if is_read_only() {
                    tracing::warn!(
                        "Apply manifest disabled in read-only mode (KUBESTUDIO_MODE=read)"
                    );
                    e.stop_propagation();
                    e.prevent_default();
                    return;
                }
                spawn(async move {
                    if let Some(path) = rfd::AsyncFileDialog::new()
                        .add_filter("YAML", &["yaml", "yml"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                        .await
                    {
                        let path_str = path.path().to_string_lossy().to_string();
                        nav.write().push(ViewState::ApplyFile { path: path_str });
                    }
                });
                e.stop_propagation();
                e.prevent_default();
            }
            // Ctrl+B = Toggle sidebar visibility
            Key::Character(ref c) if c == "b" && e.modifiers().ctrl() => {
                sidebar_visible.set(!sidebar_visible());
                e.stop_propagation();
                e.prevent_default();
            }
            Key::Character(ref c) if !e.modifiers().meta() && !e.modifiers().ctrl() => {
                match c.as_str() {
                    "o" | "0" => {
                        current_view.set("overview".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "1" | "p" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("pods".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "2" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("deployments".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "3" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("services".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "N" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("nodes".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "v" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("events".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        selected_index.set(None);
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "d" => {
                        if let Some(resource) = selected_resource.read().clone() {
                            let kind = current_kind.to_string();
                            let name = resource.name.clone();
                            let namespace = resource.namespace.clone();
                            nav.write().push(ViewState::YamlViewer {
                                kind,
                                name,
                                namespace,
                            });
                        }
                        e.stop_propagation();
                    }
                    "c" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Create resource disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                        } else {
                            nav.write().push(ViewState::CreateResource);
                        }
                        e.stop_propagation();
                    }
                    "l" => {
                        let current_state = nav.read().current().clone();
                        if let ViewState::ContainerDrillDown {
                            pod_name,
                            namespace,
                        } = current_state
                        {
                            if let Some(container_idx) = *selected_container_index.read()
                                && let Some(pod) = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name)
                                        && p.metadata.namespace.as_ref() == Some(&namespace)
                                })
                                && let Some(spec) = &pod.spec
                                && let Some(container) = spec.containers.get(container_idx)
                            {
                                nav.write().push(ViewState::LogViewer {
                                    pod_name: pod_name.clone(),
                                    namespace: namespace.clone(),
                                    container: Some(container.name.clone()),
                                });
                            }
                        } else if current_view.read().as_str() == "pods"
                            && let Some(resource) = selected_resource.read().clone()
                            && let Some(ns) = resource.namespace.clone()
                        {
                            nav.write().push(ViewState::LogViewer {
                                pod_name: resource.name.clone(),
                                namespace: ns,
                                container: None,
                            });
                        }
                        e.stop_propagation();
                    }
                    "s" => {
                        let current_state = nav.read().current().clone();
                        if let ViewState::ContainerDrillDown {
                            pod_name,
                            namespace,
                        } = current_state
                        {
                            // Exec into container - requires write permissions
                            if is_read_only() {
                                tracing::warn!(
                                    "Shell/exec disabled in read-only mode (KUBESTUDIO_MODE=read)"
                                );
                            } else if let Some(container_idx) = *selected_container_index.read()
                                && let Some(pod) = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name)
                                        && p.metadata.namespace.as_ref() == Some(&namespace)
                                })
                                && let Some(spec) = &pod.spec
                                && let Some(container) = spec.containers.get(container_idx)
                            {
                                nav.write().push(ViewState::ExecViewer {
                                    pod_name: pod_name.clone(),
                                    namespace: namespace.clone(),
                                    container: Some(container.name.clone()),
                                });
                            }
                        } else if current_view.read().as_str() == "pods" {
                            // Exec into pod - requires write permissions
                            if is_read_only() {
                                tracing::warn!(
                                    "Shell/exec disabled in read-only mode (KUBESTUDIO_MODE=read)"
                                );
                            } else if let Some(resource) = selected_resource.read().clone()
                                && let Some(ns) = resource.namespace.clone()
                            {
                                nav.write().push(ViewState::ExecViewer {
                                    pod_name: resource.name.clone(),
                                    namespace: ns,
                                    container: None,
                                });
                            }
                        } else {
                            // Navigate to services - always allowed
                            current_view.set("services".to_string());
                            nav.write().reset(ViewState::ResourceList);
                            selected_index.set(None);
                            selected_resource.set(None);
                        }
                        e.stop_propagation();
                    }
                    "f" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Port forward disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                            e.stop_propagation();
                            return;
                        }
                        if current_view.read().as_str() == "pods"
                            && let Some(resource) = selected_resource.read().clone()
                            && let Some(ns) = resource.namespace.clone()
                        {
                            let container_ports: Vec<(u16, Option<String>, String)> = pods
                                .read()
                                .items
                                .iter()
                                .find(|p| p.metadata.name.as_ref() == Some(&resource.name))
                                .and_then(|p| p.spec.as_ref())
                                .map(|spec| {
                                    spec.containers
                                        .iter()
                                        .flat_map(|c| {
                                            c.ports
                                                .as_ref()
                                                .map(|ports| {
                                                    ports
                                                        .iter()
                                                        .map(|p| {
                                                            (
                                                                p.container_port as u16,
                                                                p.name.clone(),
                                                                p.protocol.clone().unwrap_or_else(
                                                                    || "TCP".to_string(),
                                                                ),
                                                            )
                                                        })
                                                        .collect::<Vec<_>>()
                                                })
                                                .unwrap_or_default()
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            portforward_target.set(Some((
                                resource.name.clone(),
                                ns,
                                container_ports,
                            )));
                            portforward_modal_open.set(true);
                        }
                        e.stop_propagation();
                    }
                    "/" => {
                        focus_search.set(true);
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    ":" => {
                        // Open command mode (k9s-style alias input)
                        command_mode_open.set(true);
                        command_input.set(String::new());
                        e.stop_propagation();
                        e.prevent_default();
                    }
                    "F" => {
                        previous_view.set(current_view.read().clone());
                        current_view.set("portforwards".to_string());
                        nav.write().reset(ViewState::ResourceList);
                        if port_forwards.read().count() > 0 {
                            selected_index.set(Some(0));
                        } else {
                            selected_index.set(None);
                        }
                        selected_resource.set(None);
                        e.stop_propagation();
                    }
                    "n" => {
                        namespace_selector_focused.set(true);
                        e.stop_propagation();
                    }
                    "?" => {
                        help_modal_open.set(true);
                        e.stop_propagation();
                    }
                    "C" => {
                        // Shift+C toggles chat panel (only when AI is enabled and authenticated)
                        if ai_enabled() && !matrix_auth_token.read().is_empty() {
                            chat_panel_open.set(!chat_panel_open());
                        }
                        e.stop_propagation();
                    }
                    "+" | "=" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Scale up disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                            e.stop_propagation();
                            return;
                        }
                        if current_view.read().as_str() == "deployments"
                            && let Some(resource) = selected_resource.read().clone()
                            && let Some(ns) = resource.namespace.clone()
                            && let Some(ctx) = cluster.read().clone()
                        {
                            let name = resource.name.clone();
                            spawn(async move {
                                match ctx.client.get_deployment_replicas(&name, &ns).await {
                                    Ok(current) => {
                                        let new_replicas = current + 1;
                                        match ctx
                                            .client
                                            .scale_deployment(&name, &ns, new_replicas)
                                            .await
                                        {
                                            Ok(replicas) => {
                                                tracing::info!(
                                                    "Scaled {} to {} replicas",
                                                    name,
                                                    replicas
                                                );
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to scale {}: {}", name, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to get replicas for {}: {}",
                                            name,
                                            e
                                        );
                                    }
                                }
                            });
                        }
                        e.stop_propagation();
                    }
                    "-" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Scale down disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                            e.stop_propagation();
                            return;
                        }
                        if current_view.read().as_str() == "deployments"
                            && let Some(resource) = selected_resource.read().clone()
                            && let Some(ns) = resource.namespace.clone()
                            && let Some(ctx) = cluster.read().clone()
                        {
                            let name = resource.name.clone();
                            spawn(async move {
                                match ctx.client.get_deployment_replicas(&name, &ns).await {
                                    Ok(current) => {
                                        let new_replicas = (current - 1).max(0);
                                        match ctx
                                            .client
                                            .scale_deployment(&name, &ns, new_replicas)
                                            .await
                                        {
                                            Ok(replicas) => {
                                                tracing::info!(
                                                    "Scaled {} to {} replicas",
                                                    name,
                                                    replicas
                                                );
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to scale {}: {}", name, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to get replicas for {}: {}",
                                            name,
                                            e
                                        );
                                    }
                                }
                            });
                        }
                        e.stop_propagation();
                    }
                    "R" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Restart disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                            e.stop_propagation();
                            return;
                        }
                        let view = current_view.read().clone();
                        let kind = match view.as_str() {
                            "deployments" => Some("Deployment"),
                            "statefulsets" => Some("StatefulSet"),
                            "daemonsets" => Some("DaemonSet"),
                            _ => None,
                        };
                        if let Some(kind_str) = kind
                            && let Some(resource) = selected_resource.read().clone()
                        {
                            action_target.set(Some(ActionTarget {
                                name: resource.name.clone(),
                                namespace: resource.namespace.clone(),
                                kind: kind_str.to_string(),
                            }));
                            action_modal_type.set(ActionType::Restart);
                            action_modal_open.set(true);
                        }
                        e.stop_propagation();
                    }
                    "T" => {
                        if is_read_only() {
                            tracing::warn!(
                                "Trigger cronjob disabled in read-only mode (KUBESTUDIO_MODE=read)"
                            );
                            e.stop_propagation();
                            return;
                        }
                        if current_view.read().as_str() == "cronjobs"
                            && let Some(resource) = selected_resource.read().clone()
                        {
                            action_target.set(Some(ActionTarget {
                                name: resource.name.clone(),
                                namespace: resource.namespace.clone(),
                                kind: "CronJob".to_string(),
                            }));
                            action_modal_type.set(ActionType::Trigger);
                            action_modal_open.set(true);
                        }
                        e.stop_propagation();
                    }
                    // Check plugin aliases for resource navigation
                    alias => {
                        if let Some(target) = plugin_config.read().resolve_alias(alias) {
                            // Navigate to the aliased resource
                            previous_view.set(current_view.read().clone());
                            current_view.set(target.clone());
                            nav.write().reset(ViewState::ResourceList);
                            selected_index.set(None);
                            selected_resource.set(None);
                            // Update selected_crd if navigating to a CRD
                            if let Some(crd_name) = target.strip_prefix("crd:") {
                                let crd = crd_state
                                    .read()
                                    .crds
                                    .iter()
                                    .find(|c| c.name == crd_name)
                                    .cloned();
                                selected_crd.set(crd);
                            } else {
                                selected_crd.set(None);
                            }
                            e.stop_propagation();
                        }
                    }
                }
            }
            Key::Escape => {
                if nav.write().pop().is_some() {
                    if matches!(nav.read().current(), ViewState::ResourceList) {
                        selected_container_index.set(None);
                    }
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                } else if current_view.read().as_str() != "overview" {
                    let prev = previous_view.read().clone();
                    if prev.is_empty() || prev == current_view.read().as_str() {
                        current_view.set("overview".to_string());
                    } else {
                        current_view.set(prev);
                    }
                    selected_index.set(None);
                    selected_resource.set(None);
                }
                e.stop_propagation();
            }
            Key::Enter => {
                let current_state = nav.read().current().clone();
                match current_state {
                    ViewState::ContainerDrillDown {
                        pod_name,
                        namespace,
                    } => {
                        if let Some(container_idx) = *selected_container_index.read()
                            && let Some(pod) = pods.read().items.iter().find(|p| {
                                p.metadata.name.as_ref() == Some(&pod_name)
                                    && p.metadata.namespace.as_ref() == Some(&namespace)
                            })
                            && let Some(spec) = &pod.spec
                            && let Some(container) = spec.containers.get(container_idx)
                        {
                            nav.write().push(ViewState::LogViewer {
                                pod_name: pod_name.clone(),
                                namespace: namespace.clone(),
                                container: Some(container.name.clone()),
                            });
                        }
                    }
                    ViewState::ResourceList => {
                        if let Some(resource) = selected_resource.read().clone() {
                            let view = current_view.read().clone();
                            match view.as_str() {
                                "pods" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::ContainerDrillDown {
                                            pod_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "deployments" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::DeploymentPods {
                                            deployment_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "statefulsets" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::StatefulSetPods {
                                            statefulset_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "daemonsets" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::DaemonSetPods {
                                            daemonset_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "jobs" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::JobPods {
                                            job_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "cronjobs" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::CronJobJobs {
                                            cronjob_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "services" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::ServiceEndpoints {
                                            service_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                "persistentvolumeclaims" => {
                                    if let Some(ns) = &resource.namespace {
                                        nav.write().push(ViewState::PvcPods {
                                            pvc_name: resource.name.clone(),
                                            namespace: ns.clone(),
                                        });
                                        selected_container_index.set(Some(0));
                                    }
                                }
                                _ => {}
                            }
                            tracing::info!("Enter pressed on: {}", resource.name);
                        }
                    }
                    ViewState::DeploymentPods { .. }
                    | ViewState::StatefulSetPods { .. }
                    | ViewState::DaemonSetPods { .. }
                    | ViewState::JobPods { .. }
                    | ViewState::ServiceEndpoints { .. }
                    | ViewState::PvcPods { .. } => {
                        let pod_info = selected_drilldown_pod.read().clone();
                        if let Some((pod_name, ns)) = pod_info {
                            nav.write().push(ViewState::ContainerDrillDown {
                                pod_name,
                                namespace: ns,
                            });
                            selected_container_index.set(Some(0));
                            selected_drilldown_pod.set(None);
                        }
                    }
                    ViewState::CronJobJobs { .. } => {
                        let job_info = selected_drilldown_pod.read().clone();
                        if let Some((job_name, ns)) = job_info {
                            nav.write().push(ViewState::JobPods {
                                job_name,
                                namespace: ns,
                            });
                            selected_container_index.set(Some(0));
                            selected_drilldown_pod.set(None);
                        }
                    }
                    _ => {}
                }
                e.stop_propagation();
            }
            _ => {}
        }
    };

    // Commands and hotkeys (uses extracted hotkeys module)
    let commands = get_commands_with_tools(&plugin_config.read());
    let current_nav_state = nav.read().current().clone();
    let is_pods_view = current_view.read().as_str() == "pods";
    let is_services_view = current_view.read().as_str() == "services";
    let is_deployments_view = current_view.read().as_str() == "deployments";
    let is_statefulsets_view = current_view.read().as_str() == "statefulsets";
    let is_daemonsets_view = current_view.read().as_str() == "daemonsets";
    let is_cronjobs_view = current_view.read().as_str() == "cronjobs";

    let hotkeys = get_hotkeys(
        &current_nav_state,
        &current_view.read(),
        is_pods_view,
        is_services_view,
        is_deployments_view,
        is_statefulsets_view,
        is_daemonsets_view,
        is_cronjobs_view,
    );

    // Convert resources to items for display (uses extracted resource_items module)
    let pod_items = pods_to_items(&pods.read().items);
    let deployment_items = deployments_to_items(&deployments.read().items);
    let statefulset_items = statefulsets_to_items(&statefulsets.read().items);
    let daemonset_items = daemonsets_to_items(&daemonsets.read().items);
    let job_items = jobs_to_items(&jobs.read().items);
    let cronjob_items = cronjobs_to_items(&cronjobs.read().items);
    let configmap_items = configmaps_to_items(&configmaps.read().items);
    let secret_items = secrets_to_items(&secrets.read().items, &pods.read().items);
    let service_items = services_to_items(&services.read().items, &endpoints.read().items);
    let endpoint_items = endpoints_to_items(&endpoints.read().items);
    let persistentvolume_items = persistentvolumes_to_items(&persistentvolumes.read().items);
    let persistentvolumeclaim_items =
        persistentvolumeclaims_to_items(&persistentvolumeclaims.read().items);
    let ingress_items = ingresses_to_items(&ingresses.read().items);
    let storageclass_items = storageclasses_to_items(&storageclasses.read().items);
    let event_items = events_to_items(&events.read().items);

    let cluster_name = cluster.read().as_ref().map(|c| c.context_name.clone());
    let is_connected = if cluster.read().is_some() {
        *connection_healthy.read() // None = pending, Some(true) = connected, Some(false) = disconnected
    } else {
        Some(false)
    };
    let list_is_focused = focus.read().zone() == FocusZone::MainList;

    // RSX - the full rendering logic
    // NOTE: This section is too large to include inline in the module documentation
    // but cannot be extracted due to signal capture requirements
    rsx! {
        script { dangerous_inner_html: crate::theme::theme_init_script() }
        style { {crate::theme::theme_css()} }
        style { {include_str!("../styles/main.css")} }
        // Handle horizontal scrolling with arrow keys (vertical works naturally)
        script {
            dangerous_inner_html: {r#"
            document.addEventListener('keydown', function(event) {
                const target = document.activeElement;

                if (target && (target.classList.contains('describe-content') || target.classList.contains('log-content'))) {
                    if (event.key === 'ArrowLeft') {
                        target.scrollBy({ left: -80, behavior: 'smooth' });
                        event.preventDefault();
                        event.stopPropagation();
                    } else if (event.key === 'ArrowRight') {
                        target.scrollBy({ left: 80, behavior: 'smooth' });
                        event.preventDefault();
                        event.stopPropagation();
                    }
                }
            }, true);
            "#}
        }
        div {
            class: if *keyboard_nav_active.read() { "app-container keyboard-nav" } else { "app-container" },
            tabindex: 0,
            onkeydown: onkeydown,
            onmousemove: move |_| {
                if *keyboard_nav_active.read() {
                    keyboard_nav_active.set(false);
                }
            },
            onmounted: move |e| {
                tracing::info!("App container mounted");
                app_container_ref.set(Some(e.data()));
            },

            if sidebar_visible() {
                Sidebar {
                    // Cluster selector props (now integrated into sidebar)
                    contexts: available_contexts.read().clone(),
                    current_context: cluster_name.clone(),
                    is_connected: is_connected,
                    on_context_select: move |ctx: String| {
                        tracing::info!("Switching to context: {}", ctx);
                        connect_cluster(ctx);
                    },
                    // Namespace and resource props
                    namespaces: namespaces.read().clone(),
                    selected_namespace: selected_namespace.read().clone(),
                    current_view: current_view.read().clone(),
                    on_namespace_select: move |ns: String| {
                        let trimmed_ns = ns.trim();
                        if trimmed_ns.is_empty() {
                            // Empty string means "All Namespaces" was selected
                            selected_namespace.set(None);
                        } else {
                            selected_namespace.set(Some(trimmed_ns.to_string()));
                        }
                        // Reset selection when namespace changes
                        selected_index.set(None);
                        selected_resource.set(None);
                        // Refocus app container so hotkeys work
                        if let Some(app_ref) = app_container_ref.read().clone() {
                            spawn(async move {
                                let _ = app_ref.set_focus(true).await;
                            });
                        }
                    },
                    on_resource_select: move |resource_type: String| {
                        tracing::info!("Selected resource type: {}", resource_type);
                        // Update selected_crd if this is a CRD view
                        if let Some(crd_name) = resource_type.strip_prefix("crd:") {
                            let crd = crd_state.read().crds.iter()
                                .find(|c| c.name == crd_name)
                                .cloned();
                            selected_crd.set(crd);
                        } else {
                            selected_crd.set(None);
                        }
                        current_view.set(resource_type);
                        // Clear any active drill-down view
                        nav.write().reset(ViewState::ResourceList);
                        selected_container_index.set(None);
                        selected_index.set(None);
                        selected_resource.set(None);
                        // Switch focus to main content when resource selected
                        focus.write().set_zone(FocusZone::MainList);
                    },
                    // Keyboard navigation props
                    sidebar_selected_index: Some(sidebar_selected_index),
                    is_sidebar_focused: focus.read().zone() == FocusZone::Sidebar,
                    namespace_selector_focused: namespace_selector_focused,
                    // Custom Resource Definitions
                    crds: crd_state.read().crds.clone(),
                }
            }

            main { class: "main-content",
                // Plugin error toast (dismissible, shown above top bar)
                if let Some(err) = plugin_error.read().clone() {
                    div {
                        class: "plugin-error-toast",
                        onclick: move |_| plugin_error.set(None),
                        span { "{err}" }
                        span { class: "plugin-error-dismiss", "  ✕" }
                    }
                }
                // Top bar: hotkeys on left, action buttons on right
                div { class: "top-bar",
                    HotkeysBar {
                        hotkeys: hotkeys.clone(),
                    }
                    div { class: "top-bar-actions",
                        // Theme toggle
                        {
                            let theme_label = if is_dark() { "Light" } else { "Dark" };
                            let dark = is_dark();
                            rsx! {
                                button {
                                    class: "sidebar-theme-btn",
                                    title: "Toggle {theme_label} mode",
                                    onclick: move |_| toggle_theme(()),
                                    if dark { Moon { size: 16 } } else { Sun { size: 16 } }
                                }
                            }
                        }
                        // AI buttons — only when AI is enabled
                        if ai_enabled() {
                            {
                                let disabled = matrix_auth_token.read().is_empty();
                                let btn_class = if disabled { "sidebar-agent-btn disabled" } else { "sidebar-agent-btn" };
                                let title_suffix = if disabled { " (sign in to use)" } else { "" };
                                rsx! {
                                    button {
                                        class: "{btn_class}",
                                        disabled: disabled,
                                        title: "Agent Chat (Shift+C){title_suffix}",
                                        onclick: move |_| {
                                            if !disabled {
                                                chat_panel_open.set(!chat_panel_open());
                                            }
                                        },
                                        "AI"
                                    }
                                    button {
                                        class: "{btn_class}",
                                        disabled: disabled,
                                        title: "Generate Cluster Report{title_suffix}",
                                        onclick: move |_| {
                                            if !disabled {
                                                let cluster_name = cluster.read().as_ref()
                                                    .map(|c| c.context_name.clone())
                                                    .unwrap_or_else(|| "current".to_string());
                                                let msg = format!(
                                                    "Generate a comprehensive report of the current state of cluster '{}' using the kubestudio tools. Include node health, pod status, warnings, and resource utilization.",
                                                    cluster_name,
                                                );
                                                pending_chat_message.set(Some(msg));
                                                chat_panel_open.set(true);
                                            }
                                        },
                                        "Report"
                                    }
                                }
                            }
                        }
                    }
                }

                if loading() {
                    div { class: "loading",
                        h2 { Loader { size: 20 } " Loading..." }
                        p { "Connecting to Kubernetes cluster" }
                    }
                } else if let Some(err) = error_message.read().clone() {
                    div { class: "error",
                        h2 { CircleX { size: 20 } " Error" }
                        p { "{err}" }
                        p { style: "margin-top: 1rem; font-size: 0.875rem;",
                            "Troubleshooting:"
                        }
                        ul { style: "text-align: left; margin: 0.5rem auto; max-width: 500px;",
                            li { "Check if kubectl is installed: " code { "kubectl version" } }
                            li { "Verify kubeconfig exists: " code { "~/.kube/config" } }
                            li { "Test connection: " code { "kubectl get nodes" } }
                            li { "Start a local cluster: " code { "minikube start" } " or " code { "kind create cluster" } }
                        }
                    }
                } else if cluster.read().is_none() {
                    div { class: "no-cluster",
                        h2 { "🔌 No Cluster Connected" }
                        p { "Please configure your kubeconfig or start a Kubernetes cluster" }
                    }
                } else if let ViewState::YamlViewer { kind, name, namespace } = nav.read().current().clone() {
                    // Describe view - use extracted YamlViewer component
                    // Pass CRD info if viewing a custom resource
                    {
                        let crd_info_for_viewer = selected_crd.read().clone();
                        let is_read_only_mode = is_read_only();
                        rsx! {
                            YamlViewer {
                                kind: kind,
                                name: name,
                                namespace: namespace,
                                cluster: cluster,
                                crd_info: crd_info_for_viewer,
                                read_only: is_read_only_mode,
                                on_back: move |_| {
                                    // Pop the navigation stack
                                    nav.write().pop();
                                    // Refocus app container so hotkeys work
                                    if let Some(app_ref) = app_container_ref.read().clone() {
                                        spawn(async move {
                                            let _ = app_ref.set_focus(true).await;
                                        });
                                    }
                                },
                            }
                        }
                    }
                } else if matches!(nav.read().current(), ViewState::CreateResource) {
                    // Create resource view
                    CreateResource {
                        cluster: cluster,
                        namespace: selected_namespace.read().clone(),
                        on_close: move |_| {
                            // Pop the navigation stack
                            nav.write().pop();
                            // Refocus app container so hotkeys work
                            if let Some(app_ref) = app_container_ref.read().clone() {
                                spawn(async move {
                                    let _ = app_ref.set_focus(true).await;
                                });
                            }
                        },
                    }
                } else if let ViewState::ApplyFile { path } = nav.read().current().clone() {
                    // Apply manifest from file
                    ApplyManifest {
                        cluster: cluster,
                        source: ApplySource::File(path),
                        on_close: move |_| {
                            nav.write().pop();
                            if let Some(app_ref) = app_container_ref.read().clone() {
                                spawn(async move {
                                    let _ = app_ref.set_focus(true).await;
                                });
                            }
                        },
                    }
                } else {
                    match current_view.read().as_str() {
                        "overview" => {
                            let cluster_name = cluster.read().as_ref().map(|c| c.context_name.clone());
                            let nodes_list = nodes.read().items.clone();
                            let pods_list = pods.read().items.clone();
                            let deployment_count = deployments.read().items.len();
                            let statefulset_count = statefulsets.read().items.len();
                            let daemonset_count = daemonsets.read().items.len();
                            let service_count = services.read().items.len();
                            let events_list = events.read().items.clone();

                            rsx! {
                                ClusterOverview {
                                    cluster_name: cluster_name,
                                    nodes: nodes_list,
                                    pods: pods_list,
                                    deployment_count: deployment_count,
                                    statefulset_count: statefulset_count,
                                    daemonset_count: daemonset_count,
                                    service_count: service_count,
                                    events: events_list,
                                    selected_index: *selected_index.read(),
                                    is_focused: list_is_focused,
                                    keyboard_nav_active: *keyboard_nav_active.read(),
                                    has_matrix: ai_enabled(),
                                    ai_authenticated: !matrix_auth_token.read().is_empty(),
                                    on_navigate: move |view: String| {
                                        current_view.set(view);
                                        selected_index.set(None);
                                        focus.write().set_zone(FocusZone::MainList);
                                    },
                                    on_ask_agent: move |msg: String| {
                                        pending_chat_message.set(Some(msg));
                                        chat_panel_open.set(true);
                                    },
                                }
                            }
                        },
                        "portforwards" => {
                            // Port-forwards list view
                            let forwards_list: Vec<ActivePortForward> = port_forwards.read().list();
                            rsx! {
                                PortForwardsList {
                                    forwards: forwards_list,
                                    selected_index: *selected_index.read(),
                                    is_focused: list_is_focused,
                                    on_select: move |forward: ActivePortForward| {
                                        // Could navigate to the pod
                                        tracing::info!("Selected forward: {}", forward.id);
                                    },
                                    on_stop: move |forward_id: String| {
                                        port_forwards.write().remove(&forward_id);
                                        tracing::info!("Stopped forward: {}", forward_id);
                                    },
                                }
                            }
                        },
                            "pods" => {
                                // Check for container drill-down view
                                if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                    // Get container info for this pod
                                    let pod_opt = pods.read().items.iter().find(|p| {
                                        p.metadata.name.as_ref() == Some(&pod_name) &&
                                        p.metadata.namespace.as_ref() == Some(&namespace)
                                    }).cloned();

                                    // Clone for the on_select closure
                                    let pod_name_for_logs = pod_name.clone();
                                    let namespace_for_logs = namespace.clone();

                                    rsx! {
                                        ContainerDrillDown {
                                            pod_name: pod_name,
                                            namespace: namespace,
                                            pod: pod_opt,
                                            selected_index: selected_container_index,
                                            on_back: move |_| {
                                                nav.write().pop();
                                                selected_container_index.set(None);
                                            },
                                            on_select_container: move |(idx, container_name): (usize, String)| {
                                                selected_container_index.set(Some(idx));
                                                nav.write().push(ViewState::LogViewer {
                                                    pod_name: pod_name_for_logs.clone(),
                                                    namespace: namespace_for_logs.clone(),
                                                    container: Some(container_name),
                                                });
                                            },
                                        }
                                    }
                                } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                    // Log view - use extracted LogViewer component
                                    rsx! {
                                        LogViewer {
                                            pod_name: pod_name,
                                            namespace: namespace,
                                            container: container,
                                            cluster: cluster,
                                            on_back: move |_| {
                                                // Pop the navigation stack
                                                nav.write().pop();
                                                // Refocus app container so hotkeys work
                                                if let Some(app_ref) = app_container_ref.read().clone() {
                                                    spawn(async move {
                                                        let _ = app_ref.set_focus(true).await;
                                                    });
                                                }
                                            },
                                        }
                                    }
                                } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                    // Exec/shell view
                                    rsx! {
                                        ExecViewer {
                                            pod_name: pod_name,
                                            namespace: namespace,
                                            container: container,
                                            cluster: cluster,
                                            on_back: move |_| {
                                                nav.write().pop();
                                                if let Some(app_ref) = app_container_ref.read().clone() {
                                                    spawn(async move {
                                                        let _ = app_ref.set_focus(true).await;
                                                    });
                                                }
                                            },
                                        }
                                    }
                                } else {
                                    rsx! {
                                        ResourceList {
                                            kind: "Pods".to_string(),
                                            is_focused: list_is_focused,
                                            items: pod_items.clone(),
                                            namespace: selected_namespace.read().clone(),
                                            focus_search: focus_search,
                                            search_focused: search_is_focused,
                                            app_container_ref: Some(app_container_ref),
                                            selected_index: Some(selected_index),
                                            search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                            on_select: move |item: ResourceItem| {
                                                selected_resource.set(Some(item.clone()));
                                                // Drill down to container selection
                                                if let Some(ns) = &item.namespace {
                                                    nav.write().push(ViewState::ContainerDrillDown {
                                                        pod_name: item.name.clone(),
                                                        namespace: ns.clone(),
                                                    });
                                                    selected_container_index.set(Some(0));
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        "deployments" => {
                            // Check for workload drill-down view
                            if let ViewState::DeploymentPods { deployment_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    WorkloadPodsDrillDown {
                                        workload_kind: "Deployment".to_string(),
                                        workload_name: deployment_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                // Drilled into a pod from deployment
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "Deployments".to_string(),
                                        is_focused: list_is_focused,
                                        items: deployment_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to deployment's pods
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::DeploymentPods {
                                                    deployment_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "services" => {
                            if let ViewState::ServiceEndpoints { service_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    ServicePodsDrillDown {
                                        service_name: service_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "Services".to_string(),
                                        is_focused: list_is_focused,
                                        items: service_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to service endpoints
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::ServiceEndpoints {
                                                    service_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "statefulsets" => {
                            if let ViewState::StatefulSetPods { statefulset_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    WorkloadPodsDrillDown {
                                        workload_kind: "StatefulSet".to_string(),
                                        workload_name: statefulset_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "StatefulSets".to_string(),
                                        is_focused: list_is_focused,
                                        items: statefulset_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to statefulset's pods
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::StatefulSetPods {
                                                    statefulset_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "daemonsets" => {
                            if let ViewState::DaemonSetPods { daemonset_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    WorkloadPodsDrillDown {
                                        workload_kind: "DaemonSet".to_string(),
                                        workload_name: daemonset_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "DaemonSets".to_string(),
                                        is_focused: list_is_focused,
                                        items: daemonset_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to daemonset's pods
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::DaemonSetPods {
                                                    daemonset_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "jobs" => {
                            if let ViewState::JobPods { job_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    WorkloadPodsDrillDown {
                                        workload_kind: "Job".to_string(),
                                        workload_name: job_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "Jobs".to_string(),
                                        is_focused: list_is_focused,
                                        items: job_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to job's pods
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::JobPods {
                                                    job_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "cronjobs" => {
                            if let ViewState::CronJobJobs { cronjob_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    CronJobJobsDrillDown {
                                        cronjob_name: cronjob_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_job: move |(job_name, ns): (String, String)| {
                                            nav.write().push(ViewState::JobPods {
                                                job_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |job_info| {
                                            selected_drilldown_pod.set(job_info);
                                        },
                                    }
                                }
                            } else if let ViewState::JobPods { job_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    WorkloadPodsDrillDown {
                                        workload_kind: "Job".to_string(),
                                        workload_name: job_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "CronJobs".to_string(),
                                        is_focused: list_is_focused,
                                        items: cronjob_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to cronjob's jobs
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::CronJobJobs {
                                                    cronjob_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "configmaps" => rsx! {
                            ResourceList {
                                kind: "ConfigMaps".to_string(),
                                is_focused: list_is_focused,
                                items: configmap_items.clone(),
                                namespace: selected_namespace.read().clone(),
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "secrets" => rsx! {
                            ResourceList {
                                kind: "Secrets".to_string(),
                                is_focused: list_is_focused,
                                items: secret_items.clone(),
                                namespace: selected_namespace.read().clone(),
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "endpoints" => rsx! {
                            ResourceList {
                                kind: "Endpoints".to_string(),
                                is_focused: list_is_focused,
                                items: endpoint_items.clone(),
                                namespace: selected_namespace.read().clone(),
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "ingresses" => rsx! {
                            ResourceList {
                                kind: "Ingresses".to_string(),
                                is_focused: list_is_focused,
                                items: ingress_items.clone(),
                                namespace: selected_namespace.read().clone(),
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "persistentvolumes" => rsx! {
                            ResourceList {
                                kind: "PersistentVolumes".to_string(),
                                is_focused: list_is_focused,
                                items: persistentvolume_items.clone(),
                                namespace: None,
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "persistentvolumeclaims" => {
                            if let ViewState::PvcPods { pvc_name, namespace } = nav.read().current().clone() {
                                rsx! {
                                    PvcPodsDrillDown {
                                        pvc_name: pvc_name,
                                        namespace: namespace,
                                        cluster: cluster,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(None);
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_select_pod: move |(pod_name, ns): (String, String)| {
                                            nav.write().push(ViewState::ContainerDrillDown {
                                                pod_name,
                                                namespace: ns,
                                            });
                                            selected_container_index.set(Some(0));
                                            selected_drilldown_pod.set(None);
                                        },
                                        on_selection_change: move |pod_info| {
                                            selected_drilldown_pod.set(pod_info);
                                        },
                                    }
                                }
                            } else if let ViewState::ContainerDrillDown { pod_name, namespace } = nav.read().current().clone() {
                                let pod_opt = pods.read().items.iter().find(|p| {
                                    p.metadata.name.as_ref() == Some(&pod_name) &&
                                    p.metadata.namespace.as_ref() == Some(&namespace)
                                }).cloned();

                                // Clone for the on_select closure
                                let pod_name_for_logs = pod_name.clone();
                                let namespace_for_logs = namespace.clone();

                                rsx! {
                                    ContainerDrillDown {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        pod: pod_opt,
                                        selected_index: selected_container_index,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            selected_container_index.set(Some(0));
                                        },
                                        on_select_container: move |(idx, container_name): (usize, String)| {
                                            selected_container_index.set(Some(idx));
                                            nav.write().push(ViewState::LogViewer {
                                                pod_name: pod_name_for_logs.clone(),
                                                namespace: namespace_for_logs.clone(),
                                                container: Some(container_name),
                                            });
                                        },
                                    }
                                }
                            } else if let ViewState::LogViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    LogViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else if let ViewState::ExecViewer { pod_name, namespace, container } = nav.read().current().clone() {
                                rsx! {
                                    ExecViewer {
                                        pod_name: pod_name,
                                        namespace: namespace,
                                        container: container,
                                        cluster: cluster,
                                        on_back: move |_| {
                                            nav.write().pop();
                                            if let Some(app_ref) = app_container_ref.read().clone() {
                                                spawn(async move {
                                                    let _ = app_ref.set_focus(true).await;
                                                });
                                            }
                                        },
                                    }
                                }
                            } else {
                                rsx! {
                                    ResourceList {
                                        kind: "PersistentVolumeClaims".to_string(),
                                        is_focused: list_is_focused,
                                        items: persistentvolumeclaim_items.clone(),
                                        namespace: selected_namespace.read().clone(),
                                        focus_search: focus_search,
                                        search_focused: search_is_focused,
                                        app_container_ref: Some(app_container_ref),
                                        selected_index: Some(selected_index),
                                        search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                        on_select: move |item: ResourceItem| {
                                            selected_resource.set(Some(item.clone()));
                                            // Drill down to PVC's pods
                                            if let Some(ns) = &item.namespace {
                                                nav.write().push(ViewState::PvcPods {
                                                    pvc_name: item.name.clone(),
                                                    namespace: ns.clone(),
                                                });
                                                selected_container_index.set(Some(0));
                                            }
                                        },
                                    }
                                }
                            }
                        },
                        "storageclasses" => rsx! {
                            ResourceList {
                                kind: "StorageClasses".to_string(),
                                is_focused: list_is_focused,
                                items: storageclass_items.clone(),
                                namespace: None,
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "nodes" => {
                            let node_list: Vec<k8s_openapi::api::core::v1::Node> = nodes.read().items.clone();
                            let metrics_state = node_metrics.read();
                            let metrics_map = if metrics_state.available {
                                Some(metrics_state.nodes.clone())
                            } else {
                                None
                            };
                            let metrics_available = metrics_state.available;
                            rsx! {
                                NodeList {
                                    nodes: node_list,
                                    selected_index: *selected_index.read(),
                                    is_focused: list_is_focused,
                                    metrics: metrics_map,
                                    metrics_available: metrics_available,
                                    sort_state: Some(node_sort_state),
                                    on_select: move |node: k8s_openapi::api::core::v1::Node| {
                                        let name = node.metadata.name.clone().unwrap_or_default();
                                        selected_resource.set(Some(ResourceItem {
                                            name: name.clone(),
                                            namespace: None,
                                            status: "Node".to_string(),
                                            age: String::new(),
                                            age_seconds: None,
                                            ready: None,
                                            restarts: None,
                                        }));
                                        if let Some(idx) = nodes.read().items.iter().position(|n| n.metadata.name.as_ref() == Some(&name)) {
                                            selected_index.set(Some(idx));
                                        }
                                    },
                                }
                            }
                        },
                        "events" => rsx! {
                            ResourceList {
                                kind: "Events".to_string(),
                                is_focused: list_is_focused,
                                items: event_items.clone(),
                                namespace: selected_namespace.read().clone(),
                                focus_search: focus_search,
                                search_focused: search_is_focused,
                                app_container_ref: Some(app_container_ref),
                                selected_index: Some(selected_index),
                                search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                on_select: move |item: ResourceItem| {
                                    selected_resource.set(Some(item.clone()));
                                },
                            }
                        },
                        "roles" => {
                            let role_items: Vec<ResourceItem> = get_current_items();
                            rsx! {
                                ResourceList {
                                    kind: "Roles".to_string(),
                                    is_focused: list_is_focused,
                                    items: role_items,
                                    namespace: selected_namespace.read().clone(),
                                    focus_search: focus_search,
                                    search_focused: search_is_focused,
                                    app_container_ref: Some(app_container_ref),
                                    selected_index: Some(selected_index),
                                    search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                    on_select: move |item: ResourceItem| {
                                        selected_resource.set(Some(item.clone()));
                                    },
                                }
                            }
                        },
                        "clusterroles" => {
                            let clusterrole_items: Vec<ResourceItem> = get_current_items();
                            rsx! {
                                ResourceList {
                                    kind: "ClusterRoles".to_string(),
                                    is_focused: list_is_focused,
                                    items: clusterrole_items,
                                    namespace: None, // Cluster-scoped
                                    focus_search: focus_search,
                                    search_focused: search_is_focused,
                                    app_container_ref: Some(app_container_ref),
                                    selected_index: Some(selected_index),
                                    search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                    on_select: move |item: ResourceItem| {
                                        selected_resource.set(Some(item.clone()));
                                    },
                                }
                            }
                        },
                        "rolebindings" => {
                            let rolebinding_items: Vec<ResourceItem> = get_current_items();
                            rsx! {
                                ResourceList {
                                    kind: "RoleBindings".to_string(),
                                    is_focused: list_is_focused,
                                    items: rolebinding_items,
                                    namespace: selected_namespace.read().clone(),
                                    focus_search: focus_search,
                                    search_focused: search_is_focused,
                                    app_container_ref: Some(app_container_ref),
                                    selected_index: Some(selected_index),
                                    search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                    on_select: move |item: ResourceItem| {
                                        selected_resource.set(Some(item.clone()));
                                    },
                                }
                            }
                        },
                        "clusterrolebindings" => {
                            let clusterrolebinding_items: Vec<ResourceItem> = get_current_items();
                            rsx! {
                                ResourceList {
                                    kind: "ClusterRoleBindings".to_string(),
                                    is_focused: list_is_focused,
                                    items: clusterrolebinding_items,
                                    namespace: None, // Cluster-scoped
                                    focus_search: focus_search,
                                    search_focused: search_is_focused,
                                    app_container_ref: Some(app_container_ref),
                                    selected_index: Some(selected_index),
                                    search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                    on_select: move |item: ResourceItem| {
                                        selected_resource.set(Some(item.clone()));
                                    },
                                }
                            }
                        },
                        v if v.starts_with("crd:") => {
                            let crd_items: Vec<ResourceItem> = get_current_items();
                            let crd_kind = selected_crd.read()
                                .as_ref()
                                .map(|c| format!("{} ({})", c.kind, c.group))
                                .unwrap_or_else(|| "Custom Resource".to_string());
                            let crd_namespace = selected_namespace.read().clone();
                            rsx! {
                                ResourceList {
                                    kind: crd_kind,
                                    is_focused: list_is_focused,
                                    items: crd_items,
                                    namespace: crd_namespace,
                                    focus_search: focus_search,
                                    search_focused: search_is_focused,
                                    app_container_ref: Some(app_container_ref),
                                    selected_index: Some(selected_index),
                                    search_term: Some(search_term),
                                sort_state: Some(sort_state),
                                    on_select: move |item: ResourceItem| {
                                        selected_resource.set(Some(item.clone()));
                                    },
                                }
                            }
                        },
                            _ => rsx! {
                                div { class: "resource-list-container",
                                    div { class: "resource-list-header",
                                        div { class: "header-left",
                                            h2 { class: "resource-title", "{current_view}" }
                                        }
                                    }
                                    div { class: "empty-state",
                                        "🚧 This resource type is not yet implemented"
                                    }
                                }
                            },
                        }
                    }

            }

            // Chat panel (right side, only when AI is enabled)
            if ai_enabled() {
                ChatPanel {
                    visible: chat_panel_open(),
                    api_url: matrix_api_url.read().clone(),
                    auth_token: matrix_auth_token,
                    tenant_id: crate::session::get_tenant_id(),
                    on_close: move |_| chat_panel_open.set(false),
                    initial_message: pending_chat_message.read().clone(),
                    on_initial_message_consumed: move |_| {
                        pending_chat_message.set(None);
                    },
                }
            }

            // Command mode input bar (k9s-style ":" commands)
            if command_mode_open() {
                {
                    let input = command_input.read().clone();
                    let config = plugin_config.read();

                    // Find the best matching alias (first match that starts with input)
                    let prediction: Option<(String, String)> = if input.is_empty() {
                        None
                    } else {
                        config.aliases.iter()
                            .filter(|(alias, _)| alias.starts_with(&input))
                            .min_by_key(|(alias, _)| alias.len())
                            .map(|(a, t)| (a.clone(), t.clone()))
                    };

                    // Get a few other suggestions
                    let suggestions: Vec<(String, String)> = if input.is_empty() {
                        // Show common aliases when empty
                        vec![
                            ("dp".to_string(), "deployments".to_string()),
                            ("po".to_string(), "pods".to_string()),
                            ("svc".to_string(), "services".to_string()),
                            ("sec".to_string(), "secrets".to_string()),
                        ]
                    } else {
                        config.aliases.iter()
                            .filter(|(alias, _)| alias.starts_with(&input) || alias.contains(&input))
                            .take(4)
                            .map(|(a, t)| (a.clone(), t.clone()))
                            .collect()
                    };

                    rsx! {
                        div {
                            class: "command-mode-bar",
                            // Transparent input overlay to capture keyboard events
                            input {
                                class: "command-mode-input-capture",
                                r#type: "text",
                                autofocus: true,
                                onmounted: move |e| {
                                    let data = e.data();
                                    spawn(async move {
                                        let _ = data.set_focus(true).await;
                                    });
                                },
                                oninput: move |e| {
                                    command_input.set(e.value().clone());
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    if is_escape(&e) {
                                        command_mode_open.set(false);
                                        command_input.set(String::new());
                                        if let Some(app_ref) = app_container_ref.read().clone() {
                                            spawn(async move {
                                                let _ = app_ref.set_focus(true).await;
                                            });
                                        }
                                        e.stop_propagation();
                                        e.prevent_default();
                                    } else if e.key() == Key::Enter {
                                        let cmd = command_input.read().clone();
                                        command_mode_open.set(false);
                                        command_input.set(String::new());

                                        let resolved = plugin_config.read().resolve_alias(&cmd).cloned();
                                        let target = resolved.unwrap_or_else(|| cmd.clone());
                                        let sidebar_items = get_all_sidebar_items_with_crds(&crd_state.read().crds);

                                        if let Some(idx) = sidebar_items.iter().position(|k| k == &target) {
                                            tracing::info!("Command mode: navigating to '{}' (index {})", target, idx);
                                            previous_view.set(current_view.read().clone());
                                            current_view.set(target.clone());
                                            nav.write().reset(ViewState::ResourceList);
                                            selected_index.set(None);
                                            selected_resource.set(None);
                                            sidebar_selected_index.set(Some(idx));

                                            if let Some(crd_name) = target.strip_prefix("crd:") {
                                                let crd = crd_state.read().crds.iter()
                                                    .find(|c| c.name == crd_name)
                                                    .cloned();
                                                selected_crd.set(crd);
                                            } else {
                                                selected_crd.set(None);
                                            }
                                        }

                                        if let Some(app_ref) = app_container_ref.read().clone() {
                                            spawn(async move {
                                                let _ = app_ref.set_focus(true).await;
                                            });
                                        }
                                        e.stop_propagation();
                                        e.prevent_default();
                                    }
                                },
                            }
                            span { class: "command-mode-prompt", ":" }
                            span { class: "command-mode-input", "{input}" }
                            // Show prediction (remaining chars grayed out)
                            if let Some((alias, target)) = &prediction {
                                if alias.len() > input.len() {
                                    span { class: "command-mode-prediction",
                                        "{&alias[input.len()..]}"
                                    }
                                }
                                span { class: "command-mode-cursor", "_" }
                                span { class: "command-mode-arrow", ArrowRight { size: 14 } }
                                span { class: "command-mode-target", "{target}" }
                            } else {
                                span { class: "command-mode-cursor", "_" }
                                if !input.is_empty() {
                                    span { class: "command-mode-arrow", ArrowRight { size: 14 } }
                                    span { class: "command-mode-target",
                                        style: "opacity: 0.3;",
                                        "?"
                                    }
                                }
                            }
                            // Show other suggestions
                            if !suggestions.is_empty() {
                                div { class: "command-mode-suggestions",
                                    for (alias, target) in suggestions.iter().take(4) {
                                        span { class: "command-mode-suggestion",
                                            span { class: "command-mode-suggestion-key", "{alias}" }
                                            span { class: "command-mode-suggestion-target",
                                                ArrowRight { size: 14 }
                                                "{target}"
                                            }
                                        }
                                    }
                                }
                            }
                            span { class: "command-mode-hint",
                                "Enter " CornerDownLeft { size: 14 } "  Esc " X { size: 14 }
                            }
                        }
                    }
                }
            }

            CommandPalette {
                open: command_palette_open(),
                commands: commands,
                on_close: move |_| {
                    command_palette_open.set(false);
                    // Refocus the app container so keyboard shortcuts work immediately
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                },
                on_select: move |cmd_id: String| {
                    tracing::info!("Command selected: {}", cmd_id);

                    // Handle external tool commands
                    if let Some(tool_name) = cmd_id.strip_prefix("tool:") {
                        let config = plugin_config.read();
                        if let Some(tool) = config.tools.iter().find(|t| t.name == tool_name) {
                            let kind = match current_view.read().as_str() {
                                "pods" => Some("Pod".to_string()),
                                "deployments" => Some("Deployment".to_string()),
                                "services" => Some("Service".to_string()),
                                "statefulsets" => Some("StatefulSet".to_string()),
                                "daemonsets" => Some("DaemonSet".to_string()),
                                "jobs" => Some("Job".to_string()),
                                "cronjobs" => Some("CronJob".to_string()),
                                "configmaps" => Some("ConfigMap".to_string()),
                                "secrets" => Some("Secret".to_string()),
                                "endpoints" => Some("Endpoints".to_string()),
                                "ingresses" => Some("Ingress".to_string()),
                                "nodes" => Some("Node".to_string()),
                                "events" => Some("Event".to_string()),
                                v if v.starts_with("crd:") => selected_crd.read().as_ref().map(|c| c.kind.clone()),
                                _ => None,
                            };
                            // Use the global namespace filter, or fall back to the
                            // selected resource's own namespace (for "All Namespaces" mode)
                            let ns = selected_namespace.read().clone().or_else(|| {
                                selected_resource.read().as_ref().and_then(|r| r.namespace.clone())
                            });
                            let ctx = ks_plugin::TemplateContext {
                                namespace: ns,
                                name: selected_resource.read().as_ref().map(|r| r.name.clone()),
                                kind,
                                context: cluster.read().as_ref().map(|c| c.context_name.clone()),
                            };
                            if let Err(err) = ks_plugin::execute_tool(tool, &ctx) {
                                tracing::error!("Failed to launch tool {}: {}", tool_name, err);
                                plugin_error.set(Some(format!("Failed to launch {}: {}", tool_name, err)));
                            }
                        }
                        return;
                    }

                    // Handle view navigation commands
                    current_view.set(cmd_id);
                    nav.write().reset(ViewState::ResourceList);
                    selected_index.set(None);
                    selected_resource.set(None);
                },
            }

            // Delete confirmation modal
            DeleteModal {
                open: delete_modal_open(),
                target: delete_target.read().clone(),
                is_force: is_force_delete(),
                on_cancel: move |_| {
                    delete_modal_open.set(false);
                    delete_target.set(None);
                    // Refocus app container so hotkeys work
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                },
                on_confirm: move |target: DeleteTarget| {
                    tracing::info!("Delete confirmed for: {} {}", target.kind, target.name);
                    delete_modal_open.set(false);

                    // Handle PortForward deletion locally (not a K8s resource)
                    if target.kind == "PortForward" {
                        port_forwards.write().remove(&target.name);
                        tracing::info!("Stopped port-forward: {}", target.name);
                        // Adjust selection if needed
                        let new_count = port_forwards.read().count();
                        if new_count == 0 {
                            selected_index.set(None);
                        } else {
                            let idx = selected_index.read().unwrap_or(0);
                            if idx >= new_count {
                                selected_index.set(Some(new_count - 1));
                            }
                        }
                        delete_target.set(None);
                        // Refocus app container so hotkeys work
                        if let Some(app_ref) = app_container_ref.read().clone() {
                            spawn(async move {
                                let _ = app_ref.set_focus(true).await;
                            });
                        }
                        return;
                    }

                    // Perform the delete for K8s resources
                    if let Some(ctx) = cluster.read().clone() {
                        let name = target.name.clone();
                        let namespace = target.namespace.clone();
                        let kind = target.kind.clone();

                        spawn(async move {
                            let result = match kind.as_str() {
                                "Pod" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_pod(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Deployment" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_deployment(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "StatefulSet" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_statefulset(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "DaemonSet" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_daemonset(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Job" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_job(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "CronJob" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_cronjob(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "ConfigMap" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_configmap(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Secret" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_secret(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Service" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_service(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Endpoints" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_endpoint(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "Ingress" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_ingress(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "PersistentVolume" => {
                                    ctx.client.delete_persistentvolume(&name).await
                                }
                                "PersistentVolumeClaim" => {
                                    if let Some(ns) = namespace {
                                        ctx.client.delete_persistentvolumeclaim(&name, &ns).await
                                    } else {
                                        Err(ks_core::SkdError::ResourceNotFound {
                                            kind: kind.clone(),
                                            name: name.clone(),
                                            namespace: None,
                                        })
                                    }
                                }
                                "StorageClass" => {
                                    ctx.client.delete_storageclass(&name).await
                                }
                                _ => {
                                    // Check if this is a CRD kind
                                    let crd = selected_crd.read().clone();
                                    if let Some(crd_info) = crd {
                                        if crd_info.kind == kind {
                                            ctx.client.delete_crd_instance(&crd_info, &name, namespace.as_deref()).await
                                        } else {
                                            tracing::warn!("Delete not implemented for kind: {}", kind);
                                            Ok(())
                                        }
                                    } else {
                                        tracing::warn!("Delete not implemented for kind: {}", kind);
                                        Ok(())
                                    }
                                }
                            };

                            match result {
                                Ok(()) => {
                                    tracing::info!("Successfully deleted {} {}", kind, name);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to delete {} {}: {}", kind, name, e);
                                }
                            }
                        });
                    }

                    delete_target.set(None);
                    selected_resource.set(None);
                    selected_index.set(None);
                    // Refocus app container so hotkeys work
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                },
            }

            // Port-forward modal
            if portforward_modal_open() {
                if let Some((pod_name, namespace, container_ports)) = portforward_target.read().clone() {
                    PortForwardModal {
                        pod_name: pod_name.clone(),
                        namespace: namespace.clone(),
                        container_ports: container_ports,
                        on_cancel: move |_| {
                            portforward_modal_open.set(false);
                            portforward_target.set(None);
                            // Refocus app container so hotkeys work
                            if let Some(app_ref) = app_container_ref.read().clone() {
                                spawn(async move {
                                    let _ = app_ref.set_focus(true).await;
                                });
                            }
                        },
                        on_confirm: move |(local_port, remote_port): (u16, u16)| {
                            portforward_modal_open.set(false);
                            let target = portforward_target.read().clone();
                            portforward_target.set(None);

                            if let Some((pod_name, namespace, _)) = target {
                                // Check if port is already forwarded
                                if port_forwards.read().has_port(local_port) {
                                    tracing::info!("Port {} already forwarded", local_port);
                                } else if let Some(ctx) = cluster.read().clone() {
                                    spawn(async move {
                                        match ctx.client.port_forward(&pod_name, &namespace, local_port, remote_port).await {
                                            Ok(handle) => {
                                                tracing::info!(
                                                    "Port-forward started: localhost:{} -> {}:{}",
                                                    local_port, pod_name, remote_port
                                                );
                                                port_forwards.write().add(handle);
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to start port-forward: {}", e);
                                            }
                                        }
                                    });
                                }
                            }

                            // Refocus app container
                            if let Some(app_ref) = app_container_ref.read().clone() {
                                spawn(async move {
                                    let _ = app_ref.set_focus(true).await;
                                });
                            }
                        },
                    }
                }
            }

            // Action confirmation modal (restart/trigger)
            ActionConfirmModal {
                open: action_modal_open(),
                action_type: action_modal_type.read().clone(),
                target: action_target.read().clone(),
                on_cancel: move |_| {
                    action_modal_open.set(false);
                    action_target.set(None);
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                },
                on_confirm: move |target: ActionTarget| {
                    action_modal_open.set(false);
                    let action_type = action_modal_type.read().clone();

                    if let Some(ctx) = cluster.read().clone() {
                        let name = target.name.clone();
                        let namespace = target.namespace.clone();
                        let kind = target.kind.clone();

                        match action_type {
                            ActionType::Restart => {
                                if let Some(ns) = namespace {
                                    spawn(async move {
                                        let result = match kind.as_str() {
                                            "Deployment" => ctx.client.restart_deployment(&name, &ns).await,
                                            "StatefulSet" => ctx.client.restart_statefulset(&name, &ns).await,
                                            "DaemonSet" => ctx.client.restart_daemonset(&name, &ns).await,
                                            _ => Ok(()),
                                        };
                                        match result {
                                            Ok(()) => tracing::info!("Restarted {} {}", kind, name),
                                            Err(e) => tracing::error!("Failed to restart {} {}: {}", kind, name, e),
                                        }
                                    });
                                }
                            }
                            ActionType::Trigger => {
                                if let Some(ns) = namespace {
                                    spawn(async move {
                                        match ctx.client.trigger_cronjob(&name, &ns).await {
                                            Ok(job_name) => tracing::info!("Triggered job {} from cronjob {}", job_name, name),
                                            Err(e) => tracing::error!("Failed to trigger job from {}: {}", name, e),
                                        }
                                    });
                                }
                            }
                        }
                    }

                    action_target.set(None);
                    if let Some(app_ref) = app_container_ref.read().clone() {
                        spawn(async move {
                            let _ = app_ref.set_focus(true).await;
                        });
                    }
                },
            }

            // Help modal
            if help_modal_open() {
                div {
                    class: "modal-overlay",
                    tabindex: 0,
                    onclick: move |_| help_modal_open.set(false),
                    onmounted: move |e| {
                        let data = e.data();
                        spawn(async move {
                            let _ = data.set_focus(true).await;
                        });
                    },
                    onkeydown: move |e: KeyboardEvent| {
                        if is_escape(&e) {
                            help_modal_open.set(false);
                            // Refocus app container so hotkeys work
                            if let Some(app_ref) = app_container_ref.read().clone() {
                                spawn(async move {
                                    let _ = app_ref.set_focus(true).await;
                                });
                            }
                            e.stop_propagation();
                            e.prevent_default();
                        }
                    },
                    div {
                        class: "help-modal",
                        onclick: move |e| e.stop_propagation(),
                        h2 { "Keyboard Shortcuts" }

                        div { class: "help-columns",
                            div { class: "help-column",
                                div { class: "help-section",
                                    h3 { "Navigation" }
                                    div { class: "help-row", kbd { "o" } span { "Overview" } }
                                    div { class: "help-row", kbd { "p" } span { "Pods" } }
                                    div { class: "help-row", kbd { "d" } span { "Deployments" } }
                                    div { class: "help-row", kbd { "s" } span { "Services" } }
                                    div { class: "help-row", kbd { "v" } span { "Events" } }
                                    div { class: "help-row", kbd { "N" } span { "Nodes" } }
                                }

                                div { class: "help-section",
                                    h3 { "Movement" }
                                    div { class: "help-row", kbd { "↑↓" } span { "Navigate list" } }
                                    div { class: "help-row", kbd { "j/k" } span { "Navigate (vim)" } }
                                    div { class: "help-row", kbd { "←→" } span { "Sidebar / Main" } }
                                    div { class: "help-row", kbd { "Enter" } span { "Select / Drill down" } }
                                    div { class: "help-row", kbd { "Esc" } span { "Back / Close" } }
                                }

                                div { class: "help-section",
                                    h3 { "General" }
                                    div { class: "help-row", kbd { "/" } span { "Search" } }
                                    div { class: "help-row", kbd { "n" } span { "Namespace selector" } }
                                    div { class: "help-row", kbd { "^b" } span { "Toggle sidebar" } }
                                    div { class: "help-row", kbd { "^i" } span { "Command palette" } }
                                    div { class: "help-row", kbd { "?" } span { "This help" } }
                                }
                            }

                            div { class: "help-column",
                                div { class: "help-section",
                                    h3 { "Resources" }
                                    div { class: "help-row", kbd { "y" } span { "View YAML" } }
                                    div { class: "help-row", kbd { "c" } span { "Create resource" } }
                                    div { class: "help-row", kbd { "^o" } span { "Apply manifest" } }
                                    div { class: "help-row", kbd { "^d" } span { "Delete" } }
                                    div { class: "help-row", kbd { "^k" } span { "Force delete" } }
                                }

                                div { class: "help-section",
                                    h3 { "Pods" }
                                    div { class: "help-row", kbd { "l" } span { "View logs" } }
                                    div { class: "help-row", kbd { "s" } span { "Shell / Exec" } }
                                    div { class: "help-row", kbd { "f" } span { "Port forward" } }
                                    div { class: "help-row", kbd { "F" } span { "View port forwards" } }
                                    div { class: "help-row", kbd { "w" } span { "Toggle line wrap" } }
                                    div { class: "help-row", kbd { "r" } span { "Reveal secrets" } }
                                }

                                div { class: "help-section",
                                    h3 { "Workloads" }
                                    div { class: "help-row", kbd { "+/-" } span { "Scale replicas" } }
                                    div { class: "help-row", kbd { "R" } span { "Restart rollout" } }
                                    div { class: "help-row", kbd { "T" } span { "Trigger job" } }
                                }
                            }
                        }

                        div { class: "help-footer",
                            "Press Esc or click outside to close"
                        }
                    }
                }
            }
        }
    }
}
