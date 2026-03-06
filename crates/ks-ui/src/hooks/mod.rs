// Dioxus hooks for bridging state and Kubernetes client to UI

pub mod use_cluster;
pub mod use_crds;
pub mod use_focus;
pub mod use_keyboard;
pub mod use_metrics;
pub mod use_navigation;
pub mod use_portforward;
pub mod use_resources;
pub mod use_watch;

pub use use_cluster::ClusterContext;
pub use use_cluster::{use_cluster, use_connect_cluster};
pub use use_crds::{CrdState, use_watch_crd_instances, use_watch_crds};
pub use use_focus::{FocusZone, use_focus};
pub use use_metrics::{MetricsState, use_node_metrics};
pub use use_navigation::{NavigationState, ViewState, use_navigation};
pub use use_portforward::{ActivePortForward, PortForwardState, use_port_forwards};
pub use use_watch::{
    WatchConnectionState, WatchedResourceState, use_watch_clusterrolebindings,
    use_watch_clusterroles, use_watch_configmaps, use_watch_cronjobs, use_watch_daemonsets,
    use_watch_deployments, use_watch_endpoints, use_watch_events, use_watch_ingresses,
    use_watch_jobs, use_watch_nodes, use_watch_persistentvolumeclaims, use_watch_persistentvolumes,
    use_watch_pods, use_watch_rolebindings, use_watch_roles, use_watch_secrets, use_watch_services,
    use_watch_statefulsets, use_watch_storageclasses,
};
