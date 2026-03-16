//! Plugin configuration types and loading

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
}
