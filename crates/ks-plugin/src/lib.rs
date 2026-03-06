//! ks-plugin: Plugin and configuration system for KubeStudio
//!
//! This crate provides:
//! - Resource aliases (e.g., `dp` → `deployments`)
//! - Custom hotkey bindings
//! - External tool launchers
//! - Plugin configuration loading/saving

pub mod config;
pub mod executor;

pub use config::{Alias, CustomHotkey, ExternalTool, PluginConfig, PluginError, TemplateContext};
pub use executor::{ParsedHotkey, execute_hotkey, execute_tool};

/// Load the plugin configuration from the default location
pub fn load_config() -> Result<PluginConfig, PluginError> {
    PluginConfig::load()
}

/// Get the config directory path
pub fn config_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("kubestudio"))
}

/// Get the config file path
pub fn config_file() -> Option<std::path::PathBuf> {
    config_dir().map(|p| p.join("config.yaml"))
}
