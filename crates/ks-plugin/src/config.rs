//! Plugin configuration types and loading

use crate::ParsedHotkey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Plugin configuration error
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config YAML: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Config directory not found")]
    ConfigDirNotFound,
}

/// Main plugin configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    /// Resource aliases (e.g., "dp" → "deployments")
    pub aliases: HashMap<String, String>,

    /// Custom hotkey bindings
    pub hotkeys: Vec<CustomHotkey>,

    /// External tool launchers
    pub tools: Vec<ExternalTool>,

    /// Remappable keybindings for built-in actions
    pub keybindings: KeyBindings,
}

impl PluginConfig {
    /// Load configuration from the default location (~/.config/kubestudio/config.yaml)
    /// If no config file exists, returns the built-in defaults with aliases and tools.
    /// If a config file exists, merges it with defaults (user config takes priority).
    pub fn load() -> Result<Self, PluginError> {
        let config_path = Self::config_path().ok_or(PluginError::ConfigDirNotFound)?;
        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific file path.
    /// If the file doesn't exist, returns built-in defaults.
    /// If it exists, parses it and merges with defaults (user config takes priority).
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, PluginError> {
        if !path.exists() {
            tracing::debug!(
                "No config file found at {:?}, using built-in defaults",
                path
            );
            return Ok(Self::with_defaults());
        }

        let content = std::fs::read_to_string(path)?;
        let user_config: PluginConfig = serde_yaml::from_str(&content)?;

        // Start with defaults and merge user config on top
        let mut config = Self::with_defaults();

        // User aliases override defaults
        for (alias, target) in user_config.aliases {
            config.aliases.insert(alias, target);
        }

        // User hotkeys are added (these don't have defaults)
        config.hotkeys = user_config.hotkeys;

        // User keybindings override defaults (serde(default) fills unset fields)
        config.keybindings = user_config.keybindings;

        // User tools override defaults by name, or add new ones
        for user_tool in user_config.tools {
            if let Some(pos) = config.tools.iter().position(|t| t.name == user_tool.name) {
                config.tools[pos] = user_tool;
            } else {
                config.tools.push(user_tool);
            }
        }

        tracing::info!("Loaded plugin config from {:?}", path);
        Ok(config)
    }

    /// Get the default config file path
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("kubestudio").join("config.yaml"))
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<(), PluginError> {
        let config_path = Self::config_path().ok_or(PluginError::ConfigDirNotFound)?;

        // Create directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)?;
        std::fs::write(&config_path, content)?;
        tracing::info!("Saved plugin config to {:?}", config_path);
        Ok(())
    }

    /// Resolve an alias to a resource key
    /// Returns the target resource key if alias exists, otherwise None
    pub fn resolve_alias(&self, alias: &str) -> Option<&String> {
        self.aliases.get(alias)
    }

    /// Get the default configuration with common aliases
    pub fn with_defaults() -> Self {
        let mut aliases = HashMap::new();
        // Common k9s-style aliases
        aliases.insert("dp".to_string(), "deployments".to_string());
        aliases.insert("deploy".to_string(), "deployments".to_string());
        aliases.insert("po".to_string(), "pods".to_string());
        aliases.insert("svc".to_string(), "services".to_string());
        aliases.insert("sec".to_string(), "secrets".to_string());
        aliases.insert("cm".to_string(), "configmaps".to_string());
        aliases.insert("ns".to_string(), "namespaces".to_string());
        aliases.insert("no".to_string(), "nodes".to_string());
        aliases.insert("ing".to_string(), "ingresses".to_string());
        aliases.insert("pv".to_string(), "persistentvolumes".to_string());
        aliases.insert("pvc".to_string(), "persistentvolumeclaims".to_string());
        aliases.insert("sc".to_string(), "storageclasses".to_string());
        aliases.insert("sts".to_string(), "statefulsets".to_string());
        aliases.insert("ds".to_string(), "daemonsets".to_string());
        aliases.insert("rs".to_string(), "replicasets".to_string());
        aliases.insert("cj".to_string(), "cronjobs".to_string());
        aliases.insert("rb".to_string(), "rolebindings".to_string());
        aliases.insert("crb".to_string(), "clusterrolebindings".to_string());
        aliases.insert("ep".to_string(), "endpoints".to_string());
        aliases.insert("ev".to_string(), "events".to_string());

        Self {
            aliases,
            hotkeys: Vec::new(),
            keybindings: KeyBindings::default(),
            tools: vec![
                ExternalTool {
                    name: "echo-test".to_string(),
                    command: "echo".to_string(),
                    args: vec![
                        "Plugin test: namespace={{namespace}} name={{name}} kind={{kind}} context={{context}}".to_string(),
                    ],
                    description: Some("Test plugin pipeline (no external tools needed)".to_string()),
                },
                ExternalTool {
                    name: "kubectl-get".to_string(),
                    command: "kubectl".to_string(),
                    args: vec![
                        "get".to_string(),
                        "{{kind}}".to_string(),
                        "{{name}}".to_string(),
                        "--context".to_string(),
                        "{{context}}".to_string(),
                        "--namespace".to_string(),
                        "{{namespace}}".to_string(),
                    ],
                    description: Some("kubectl get for selected resource".to_string()),
                },
            ],
        }
    }
}

/// A resource alias mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    /// The alias (e.g., "dp")
    pub alias: String,
    /// The target resource key (e.g., "deployments")
    pub target: String,
}

/// A custom hotkey binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomHotkey {
    /// The key combination (e.g., "Ctrl+Shift+L", "Alt+K")
    pub key: String,

    /// The shell command to execute
    /// Supports template variables: {{namespace}}, {{name}}, {{kind}}, {{context}}
    pub command: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Whether this hotkey requires a resource to be selected
    #[serde(default)]
    pub requires_selection: bool,

    /// Whether to run in a new terminal window
    #[serde(default)]
    pub open_terminal: bool,
}

impl CustomHotkey {
    /// Expand the command template with the given context
    pub fn expand_command(&self, ctx: &TemplateContext) -> String {
        let mut cmd = self.command.clone();
        cmd = cmd.replace("{{namespace}}", &ctx.namespace.clone().unwrap_or_default());
        cmd = cmd.replace("{{name}}", &ctx.name.clone().unwrap_or_default());
        cmd = cmd.replace("{{kind}}", &ctx.kind.clone().unwrap_or_default());
        cmd = cmd.replace("{{context}}", &ctx.context.clone().unwrap_or_default());
        cmd
    }
}

/// An external tool launcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    /// Tool name (e.g., "k9s")
    pub name: String,

    /// The executable command
    pub command: String,

    /// Command line arguments
    /// Supports template variables: {{namespace}}, {{name}}, {{kind}}, {{context}}
    #[serde(default)]
    pub args: Vec<String>,

    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
}

impl ExternalTool {
    /// Expand the arguments with the given context.
    ///
    /// Template variables that resolve to None/empty are handled:
    /// - A standalone arg that expands to empty is dropped
    /// - A `--flag value` pair where the value is empty drops both args
    pub fn expand_args(&self, ctx: &TemplateContext) -> Vec<String> {
        let expanded: Vec<String> = self
            .args
            .iter()
            .map(|arg| {
                let mut a = arg.clone();
                a = a.replace("{{namespace}}", &ctx.namespace.clone().unwrap_or_default());
                a = a.replace("{{name}}", &ctx.name.clone().unwrap_or_default());
                a = a.replace("{{kind}}", &ctx.kind.clone().unwrap_or_default());
                a = a.replace("{{context}}", &ctx.context.clone().unwrap_or_default());
                a
            })
            .collect();

        // Filter out empty args and their preceding flags.
        // e.g., ["--namespace", ""] → dropped entirely.
        let mut result = Vec::with_capacity(expanded.len());
        let mut skip_next = false;
        for (i, arg) in expanded.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }
            if arg.is_empty() {
                // Remove the preceding flag if it was a --flag
                if result
                    .last()
                    .map(|s: &String| s.starts_with('-'))
                    .unwrap_or(false)
                {
                    result.pop();
                }
                continue;
            }
            // If this is a flag and the next arg is empty, skip both
            if arg.starts_with('-')
                && let Some(next) = expanded.get(i + 1)
                && next.is_empty()
            {
                skip_next = true;
                continue;
            }
            result.push(arg.clone());
        }
        result
    }
}

/// Context for template expansion
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    /// Current namespace (or None for all namespaces)
    pub namespace: Option<String>,
    /// Selected resource name
    pub name: Option<String>,
    /// Selected resource kind (e.g., "Pod", "Deployment")
    pub kind: Option<String>,
    /// Current Kubernetes context name
    pub context: Option<String>,
}

/// Remappable keybindings for all application actions.
///
/// Each field holds a hotkey string (e.g. `"d"`, `"Ctrl+D"`, `"Shift+C"`).
/// Parsing and matching is delegated to [`ParsedHotkey`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct KeyBindings {
    // --- Navigation ---
    pub overview: String,
    pub pods: String,
    pub deployments: String,
    pub services: String,
    pub events: String,
    pub nodes: String,
    pub port_forwards: String,

    // --- General ---
    pub search: String,
    pub namespace: String,
    pub command_mode: String,
    pub help: String,
    pub toggle_sidebar: String,
    pub command_palette: String,
    pub toggle_chat: String,

    // --- Resource actions ---
    pub describe: String,
    pub create_resource: String,
    pub apply_manifest: String,
    pub delete: String,
    pub force_delete: String,

    // --- Pod actions ---
    pub logs: String,
    pub shell: String,
    pub port_forward: String,

    // --- Viewer actions ---
    pub toggle_wrap: String,
    pub copy: String,
    pub toggle_view: String,
    pub toggle_managed_fields: String,
    pub edit: String,
    pub reveal_secrets: String,
    pub apply_edit: String,

    // --- Log viewer ---
    pub toggle_follow: String,
    pub toggle_timestamps: String,

    // --- Deployment actions ---
    pub scale_up: String,
    pub scale_down: String,
    pub restart: String,
    pub trigger: String,

    // --- Apply manifest ---
    pub apply_manifest_confirm: String,

    // --- Settings ---
    pub settings: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            // Navigation
            overview: "o".into(),
            pods: "p".into(),
            deployments: "2".into(),
            services: "3".into(),
            events: "v".into(),
            nodes: "Shift+N".into(),
            port_forwards: "Shift+F".into(),

            // General
            search: "/".into(),
            namespace: "n".into(),
            command_mode: ":".into(),
            help: "?".into(),
            toggle_sidebar: "Ctrl+b".into(),
            command_palette: "Ctrl+i".into(),
            toggle_chat: "Shift+C".into(),

            // Resource actions
            describe: "d".into(),
            create_resource: "c".into(),
            apply_manifest: "Ctrl+o".into(),
            delete: "Ctrl+d".into(),
            force_delete: "Ctrl+k".into(),

            // Pod actions
            logs: "l".into(),
            shell: "s".into(),
            port_forward: "f".into(),

            // Viewer actions
            toggle_wrap: "w".into(),
            copy: "c".into(),
            toggle_view: "h".into(),
            toggle_managed_fields: "m".into(),
            edit: "e".into(),
            reveal_secrets: "r".into(),
            apply_edit: "Ctrl+s".into(),

            // Log viewer
            toggle_follow: "f".into(),
            toggle_timestamps: "t".into(),

            // Deployment actions
            scale_up: "+".into(),
            scale_down: "-".into(),
            restart: "Shift+R".into(),
            trigger: "Shift+T".into(),

            // Apply manifest
            apply_manifest_confirm: "Ctrl+Enter".into(),

            // Settings
            settings: ",".into(),
        }
    }
}

impl KeyBindings {
    /// Check whether a keyboard event matches the binding for the given action.
    pub fn matches(
        &self,
        action: &str,
        key: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) -> bool {
        let binding = match action {
            "overview" => &self.overview,
            "pods" => &self.pods,
            "deployments" => &self.deployments,
            "services" => &self.services,
            "events" => &self.events,
            "nodes" => &self.nodes,
            "port_forwards" => &self.port_forwards,
            "search" => &self.search,
            "namespace" => &self.namespace,
            "command_mode" => &self.command_mode,
            "help" => &self.help,
            "toggle_sidebar" => &self.toggle_sidebar,
            "command_palette" => &self.command_palette,
            "toggle_chat" => &self.toggle_chat,
            "describe" => &self.describe,
            "create_resource" => &self.create_resource,
            "apply_manifest" => &self.apply_manifest,
            "delete" => &self.delete,
            "force_delete" => &self.force_delete,
            "logs" => &self.logs,
            "shell" => &self.shell,
            "port_forward" => &self.port_forward,
            "toggle_wrap" => &self.toggle_wrap,
            "copy" => &self.copy,
            "toggle_view" => &self.toggle_view,
            "toggle_managed_fields" => &self.toggle_managed_fields,
            "edit" => &self.edit,
            "reveal_secrets" => &self.reveal_secrets,
            "apply_edit" => &self.apply_edit,
            "toggle_follow" => &self.toggle_follow,
            "toggle_timestamps" => &self.toggle_timestamps,
            "scale_up" => &self.scale_up,
            "scale_down" => &self.scale_down,
            "restart" => &self.restart,
            "trigger" => &self.trigger,
            "apply_manifest_confirm" => &self.apply_manifest_confirm,
            "settings" => &self.settings,
            _ => return false,
        };
        ParsedHotkey::parse(binding).matches(key, ctrl, shift, alt, meta)
    }

    /// Return the raw hotkey string for display in the UI.
    pub fn display(&self, action: &str) -> &str {
        match action {
            "overview" => &self.overview,
            "pods" => &self.pods,
            "deployments" => &self.deployments,
            "services" => &self.services,
            "events" => &self.events,
            "nodes" => &self.nodes,
            "port_forwards" => &self.port_forwards,
            "search" => &self.search,
            "namespace" => &self.namespace,
            "command_mode" => &self.command_mode,
            "help" => &self.help,
            "toggle_sidebar" => &self.toggle_sidebar,
            "command_palette" => &self.command_palette,
            "toggle_chat" => &self.toggle_chat,
            "describe" => &self.describe,
            "create_resource" => &self.create_resource,
            "apply_manifest" => &self.apply_manifest,
            "delete" => &self.delete,
            "force_delete" => &self.force_delete,
            "logs" => &self.logs,
            "shell" => &self.shell,
            "port_forward" => &self.port_forward,
            "toggle_wrap" => &self.toggle_wrap,
            "copy" => &self.copy,
            "toggle_view" => &self.toggle_view,
            "toggle_managed_fields" => &self.toggle_managed_fields,
            "edit" => &self.edit,
            "reveal_secrets" => &self.reveal_secrets,
            "apply_edit" => &self.apply_edit,
            "toggle_follow" => &self.toggle_follow,
            "toggle_timestamps" => &self.toggle_timestamps,
            "scale_up" => &self.scale_up,
            "scale_down" => &self.scale_down,
            "restart" => &self.restart,
            "trigger" => &self.trigger,
            "apply_manifest_confirm" => &self.apply_manifest_confirm,
            "settings" => &self.settings,
            _ => "",
        }
    }

    /// Return all keybinding entries grouped by category.
    /// Each tuple is `(category, action_id, human_label, binding_value, context)`.
    /// The `context` field groups bindings that are active simultaneously —
    /// only bindings in the same context can truly conflict.
    pub fn entries(&self) -> Vec<(&'static str, &'static str, &'static str, &str, &'static str)> {
        vec![
            // Navigation (global context — always active)
            (
                "Navigation",
                "overview",
                "Overview",
                &self.overview,
                "global",
            ),
            ("Navigation", "pods", "Pods", &self.pods, "global"),
            (
                "Navigation",
                "deployments",
                "Deployments",
                &self.deployments,
                "global",
            ),
            (
                "Navigation",
                "services",
                "Services",
                &self.services,
                "global",
            ),
            ("Navigation", "events", "Events", &self.events, "global"),
            ("Navigation", "nodes", "Nodes", &self.nodes, "global"),
            (
                "Navigation",
                "port_forwards",
                "Port Forwards",
                &self.port_forwards,
                "global",
            ),
            // General (global context — always active)
            ("General", "search", "Search", &self.search, "global"),
            (
                "General",
                "namespace",
                "Namespace Selector",
                &self.namespace,
                "global",
            ),
            (
                "General",
                "command_mode",
                "Command Mode",
                &self.command_mode,
                "global",
            ),
            ("General", "help", "Help", &self.help, "global"),
            (
                "General",
                "toggle_sidebar",
                "Toggle Sidebar",
                &self.toggle_sidebar,
                "global",
            ),
            (
                "General",
                "command_palette",
                "Command Palette",
                &self.command_palette,
                "global",
            ),
            (
                "General",
                "toggle_chat",
                "Toggle Chat",
                &self.toggle_chat,
                "global",
            ),
            ("General", "settings", "Settings", &self.settings, "global"),
            // Resource Actions (active in resource list views)
            (
                "Resource Actions",
                "describe",
                "Describe / YAML",
                &self.describe,
                "resource_list",
            ),
            (
                "Resource Actions",
                "create_resource",
                "Create Resource",
                &self.create_resource,
                "resource_list",
            ),
            (
                "Resource Actions",
                "apply_manifest",
                "Apply Manifest",
                &self.apply_manifest,
                "resource_list",
            ),
            (
                "Resource Actions",
                "delete",
                "Delete",
                &self.delete,
                "resource_list",
            ),
            (
                "Resource Actions",
                "force_delete",
                "Force Delete",
                &self.force_delete,
                "resource_list",
            ),
            // Pod Actions (active only in pods view)
            ("Pod Actions", "logs", "View Logs", &self.logs, "pods"),
            ("Pod Actions", "shell", "Shell / Exec", &self.shell, "pods"),
            (
                "Pod Actions",
                "port_forward",
                "Port Forward",
                &self.port_forward,
                "pods",
            ),
            // Viewer Actions (active in YAML/describe viewer)
            (
                "Viewer Actions",
                "toggle_wrap",
                "Toggle Wrap",
                &self.toggle_wrap,
                "viewer",
            ),
            ("Viewer Actions", "copy", "Copy", &self.copy, "viewer"),
            (
                "Viewer Actions",
                "toggle_view",
                "Toggle View",
                &self.toggle_view,
                "viewer",
            ),
            (
                "Viewer Actions",
                "toggle_managed_fields",
                "Toggle Managed Fields",
                &self.toggle_managed_fields,
                "viewer",
            ),
            ("Viewer Actions", "edit", "Edit", &self.edit, "viewer"),
            (
                "Viewer Actions",
                "reveal_secrets",
                "Reveal Secrets",
                &self.reveal_secrets,
                "viewer",
            ),
            (
                "Viewer Actions",
                "apply_edit",
                "Apply Edit",
                &self.apply_edit,
                "viewer",
            ),
            // Log Viewer (active only in log viewer)
            (
                "Log Viewer",
                "toggle_follow",
                "Toggle Follow",
                &self.toggle_follow,
                "log_viewer",
            ),
            (
                "Log Viewer",
                "toggle_timestamps",
                "Toggle Timestamps",
                &self.toggle_timestamps,
                "log_viewer",
            ),
            // Deployment Actions (active only in deployments view)
            (
                "Deployment Actions",
                "scale_up",
                "Scale Up",
                &self.scale_up,
                "deployments",
            ),
            (
                "Deployment Actions",
                "scale_down",
                "Scale Down",
                &self.scale_down,
                "deployments",
            ),
            (
                "Deployment Actions",
                "restart",
                "Restart Rollout",
                &self.restart,
                "deployments",
            ),
            (
                "Deployment Actions",
                "trigger",
                "Trigger Job",
                &self.trigger,
                "deployments",
            ),
            // Apply Manifest (active only in apply modal)
            (
                "Apply Manifest",
                "apply_manifest_confirm",
                "Confirm Apply",
                &self.apply_manifest_confirm,
                "apply_modal",
            ),
        ]
    }

    /// Set the binding for a given action. Returns `true` if the action was found.
    pub fn set_binding(&mut self, action: &str, value: String) -> bool {
        match action {
            "overview" => self.overview = value,
            "pods" => self.pods = value,
            "deployments" => self.deployments = value,
            "services" => self.services = value,
            "events" => self.events = value,
            "nodes" => self.nodes = value,
            "port_forwards" => self.port_forwards = value,
            "search" => self.search = value,
            "namespace" => self.namespace = value,
            "command_mode" => self.command_mode = value,
            "help" => self.help = value,
            "toggle_sidebar" => self.toggle_sidebar = value,
            "command_palette" => self.command_palette = value,
            "toggle_chat" => self.toggle_chat = value,
            "settings" => self.settings = value,
            "describe" => self.describe = value,
            "create_resource" => self.create_resource = value,
            "apply_manifest" => self.apply_manifest = value,
            "delete" => self.delete = value,
            "force_delete" => self.force_delete = value,
            "logs" => self.logs = value,
            "shell" => self.shell = value,
            "port_forward" => self.port_forward = value,
            "toggle_wrap" => self.toggle_wrap = value,
            "copy" => self.copy = value,
            "toggle_view" => self.toggle_view = value,
            "toggle_managed_fields" => self.toggle_managed_fields = value,
            "edit" => self.edit = value,
            "reveal_secrets" => self.reveal_secrets = value,
            "apply_edit" => self.apply_edit = value,
            "toggle_follow" => self.toggle_follow = value,
            "toggle_timestamps" => self.toggle_timestamps = value,
            "scale_up" => self.scale_up = value,
            "scale_down" => self.scale_down = value,
            "restart" => self.restart = value,
            "trigger" => self.trigger = value,
            "apply_manifest_confirm" => self.apply_manifest_confirm = value,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_resolution() {
        let config = PluginConfig::with_defaults();
        assert_eq!(config.resolve_alias("dp"), Some(&"deployments".to_string()));
        assert_eq!(config.resolve_alias("unknown"), None);
    }

    #[test]
    fn test_template_expansion() {
        let hotkey = CustomHotkey {
            key: "Ctrl+L".to_string(),
            command: "stern {{name}} -n {{namespace}} --context={{context}}".to_string(),
            description: "Tail logs".to_string(),
            requires_selection: true,
            open_terminal: true,
        };

        let ctx = TemplateContext {
            namespace: Some("default".to_string()),
            name: Some("my-pod".to_string()),
            context: Some("prod-cluster".to_string()),
            kind: Some("Pod".to_string()),
        };

        let expanded = hotkey.expand_command(&ctx);
        assert_eq!(expanded, "stern my-pod -n default --context=prod-cluster");
    }

    #[test]
    fn test_yaml_serialization() {
        let config = PluginConfig::with_defaults();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("dp:"));
        assert!(yaml.contains("deployments"));
    }

    #[test]
    fn test_keybindings_default_values() {
        let kb = KeyBindings::default();
        assert_eq!(kb.overview, "o");
        assert_eq!(kb.pods, "p");
        assert_eq!(kb.deployments, "2");
        assert_eq!(kb.services, "3");
        assert_eq!(kb.events, "v");
        assert_eq!(kb.nodes, "Shift+N");
        assert_eq!(kb.port_forwards, "Shift+F");
        assert_eq!(kb.search, "/");
        assert_eq!(kb.namespace, "n");
        assert_eq!(kb.describe, "d");
        assert_eq!(kb.delete, "Ctrl+d");
        assert_eq!(kb.force_delete, "Ctrl+k");
        assert_eq!(kb.logs, "l");
        assert_eq!(kb.shell, "s");
        assert_eq!(kb.apply_edit, "Ctrl+s");
        assert_eq!(kb.restart, "Shift+R");
        assert_eq!(kb.trigger, "Shift+T");
    }

    #[test]
    fn test_keybindings_matches_simple_key() {
        let kb = KeyBindings::default();
        // "d" matches describe
        assert!(kb.matches("describe", "d", false, false, false, false));
        // "d" with Ctrl should NOT match describe (it matches delete)
        assert!(!kb.matches("describe", "d", true, false, false, false));
        // Wrong key
        assert!(!kb.matches("describe", "x", false, false, false, false));
    }

    #[test]
    fn test_keybindings_matches_ctrl_combo() {
        let kb = KeyBindings::default();
        // Ctrl+D matches delete
        assert!(kb.matches("delete", "d", true, false, false, false));
        // Plain "d" should NOT match delete
        assert!(!kb.matches("delete", "d", false, false, false, false));
        // Ctrl+K matches force_delete
        assert!(kb.matches("force_delete", "k", true, false, false, false));
    }

    #[test]
    fn test_keybindings_matches_shift_key() {
        let kb = KeyBindings::default();
        // Shift+N matches nodes
        assert!(kb.matches("nodes", "N", false, true, false, false));
        // Shift+R matches restart
        assert!(kb.matches("restart", "R", false, true, false, false));
        // Shift+T matches trigger
        assert!(kb.matches("trigger", "T", false, true, false, false));
    }

    #[test]
    fn test_keybindings_display() {
        let kb = KeyBindings::default();
        assert_eq!(kb.display("overview"), "o");
        assert_eq!(kb.display("pods"), "p");
        assert_eq!(kb.display("delete"), "Ctrl+d");
        assert_eq!(kb.display("nodes"), "Shift+N");
        assert_eq!(kb.display("unknown_action"), "");
    }

    #[test]
    fn test_keybindings_matches_unknown_action() {
        let kb = KeyBindings::default();
        assert!(!kb.matches("nonexistent", "x", false, false, false, false));
    }

    #[test]
    fn test_keybindings_in_plugin_config() {
        let config = PluginConfig::with_defaults();
        assert_eq!(config.keybindings, KeyBindings::default());
    }

    #[test]
    fn test_keybindings_entries() {
        let kb = KeyBindings::default();
        let entries = kb.entries();
        // Should have all keybinding entries
        assert!(entries.len() >= 37);
        // Check first entry
        assert_eq!(
            entries[0],
            ("Navigation", "overview", "Overview", "o", "global")
        );
        // Check that all entries have non-empty categories, action_ids, labels, and contexts
        for (cat, action, label, _val, ctx) in &entries {
            assert!(!cat.is_empty());
            assert!(!action.is_empty());
            assert!(!label.is_empty());
            assert!(!ctx.is_empty());
        }
    }

    #[test]
    fn test_keybindings_set_binding() {
        let mut kb = KeyBindings::default();
        assert!(kb.set_binding("overview", "Ctrl+1".to_string()));
        assert_eq!(kb.overview, "Ctrl+1");
        assert!(kb.set_binding("delete", "Alt+d".to_string()));
        assert_eq!(kb.delete, "Alt+d");
        // Unknown action returns false
        assert!(!kb.set_binding("nonexistent", "x".to_string()));
    }

    #[test]
    fn test_keybindings_settings_field() {
        let kb = KeyBindings::default();
        assert_eq!(kb.settings, ",");
        assert!(kb.matches("settings", ",", false, false, false, false));
        assert_eq!(kb.display("settings"), ",");
    }
}
