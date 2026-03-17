use ks_plugin::{
    CustomHotkey, ExternalTool, KeyBindings, ParsedHotkey, PluginConfig, TemplateContext,
};
use std::io::Write;

// --- Config loading from file ---

#[test]
fn test_load_from_nonexistent_path_returns_defaults() {
    let path = std::path::Path::new("/tmp/kubestudio_test_nonexistent_config.yaml");
    // Ensure it doesn't exist
    let _ = std::fs::remove_file(path);

    let config = PluginConfig::load_from_path(path).unwrap();
    // Should have default aliases
    assert_eq!(config.resolve_alias("dp"), Some(&"deployments".to_string()));
    assert_eq!(config.resolve_alias("po"), Some(&"pods".to_string()));
    // Should have default tools including echo-test
    assert!(config.tools.iter().any(|t| t.name == "echo-test"));
    assert!(config.tools.iter().any(|t| t.name == "kubectl-get"));
}

#[test]
fn test_load_from_empty_file_returns_defaults() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    // Write valid but empty YAML (just a comment)
    writeln!(tmp, "# empty config").unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    // Defaults should still be present
    assert!(config.aliases.contains_key("dp"));
    assert!(!config.tools.is_empty());
}

#[test]
fn test_load_user_aliases_merge_with_defaults() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "aliases:\n  myalias: pods\n  dp: custom-deployments").unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    // User alias added
    assert_eq!(config.resolve_alias("myalias"), Some(&"pods".to_string()));
    // User alias overrides default
    assert_eq!(
        config.resolve_alias("dp"),
        Some(&"custom-deployments".to_string())
    );
    // Other defaults still present
    assert_eq!(config.resolve_alias("po"), Some(&"pods".to_string()));
    assert_eq!(config.resolve_alias("svc"), Some(&"services".to_string()));
}

#[test]
fn test_load_user_hotkeys_replace_defaults() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"hotkeys:
  - key: "Alt+T"
    command: "echo test"
    description: "Test hotkey"
    requires_selection: false
    open_terminal: false
"#
    )
    .unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    assert_eq!(config.hotkeys.len(), 1);
    assert_eq!(config.hotkeys[0].key, "Alt+T");
    assert_eq!(config.hotkeys[0].command, "echo test");
}

#[test]
fn test_load_user_tools_merge_by_name() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"tools:
  - name: kubectl-get
    command: kubectl
    args: ["get", "pods"]
    description: "Custom kubectl-get"
  - name: mytool
    command: mytool
    args: []
    description: "My custom tool"
"#
    )
    .unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    // kubectl-get should be overridden by user config
    let kubectl = config
        .tools
        .iter()
        .find(|t| t.name == "kubectl-get")
        .unwrap();
    assert_eq!(kubectl.args, vec!["get", "pods"]);
    assert_eq!(kubectl.description, Some("Custom kubectl-get".to_string()));
    // mytool should be added
    assert!(config.tools.iter().any(|t| t.name == "mytool"));
    // Default tools still present
    assert!(config.tools.iter().any(|t| t.name == "echo-test"));
}

#[test]
fn test_load_partial_config_only_aliases() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "aliases:\n  x: pods").unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    assert_eq!(config.resolve_alias("x"), Some(&"pods".to_string()));
    // Hotkeys should be empty (no user hotkeys, no defaults)
    assert!(config.hotkeys.is_empty());
    // Default tools still present
    assert!(!config.tools.is_empty());
}

#[test]
fn test_load_invalid_yaml_returns_error() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "{{{{ invalid yaml !@#$").unwrap();

    let result = PluginConfig::load_from_path(tmp.path());
    assert!(result.is_err());
}

// --- Alias resolution ---

#[test]
fn test_resolve_alias_all_defaults() {
    let config = PluginConfig::with_defaults();
    let expected = vec![
        ("dp", "deployments"),
        ("deploy", "deployments"),
        ("po", "pods"),
        ("svc", "services"),
        ("sec", "secrets"),
        ("cm", "configmaps"),
        ("ns", "namespaces"),
        ("no", "nodes"),
        ("ing", "ingresses"),
        ("pv", "persistentvolumes"),
        ("pvc", "persistentvolumeclaims"),
        ("sc", "storageclasses"),
        ("sts", "statefulsets"),
        ("ds", "daemonsets"),
        ("rs", "replicasets"),
        ("cj", "cronjobs"),
        ("rb", "rolebindings"),
        ("crb", "clusterrolebindings"),
        ("ep", "endpoints"),
        ("ev", "events"),
    ];
    for (alias, target) in expected {
        assert_eq!(
            config.resolve_alias(alias),
            Some(&target.to_string()),
            "alias '{}' should resolve to '{}'",
            alias,
            target
        );
    }
}

#[test]
fn test_resolve_alias_unknown_returns_none() {
    let config = PluginConfig::with_defaults();
    assert_eq!(config.resolve_alias("nonexistent"), None);
    assert_eq!(config.resolve_alias(""), None);
    assert_eq!(config.resolve_alias("PODS"), None); // case-sensitive
}

// --- Template expansion ---

#[test]
fn test_hotkey_expand_command_all_values() {
    let hotkey = CustomHotkey {
        key: "Ctrl+L".to_string(),
        command: "kubectl describe {{kind}} {{name}} -n {{namespace}} --context={{context}}"
            .to_string(),
        description: "Describe".to_string(),
        requires_selection: true,
        open_terminal: true,
    };
    let ctx = TemplateContext {
        namespace: Some("prod".to_string()),
        name: Some("my-pod".to_string()),
        kind: Some("Pod".to_string()),
        context: Some("prod-cluster".to_string()),
    };
    assert_eq!(
        hotkey.expand_command(&ctx),
        "kubectl describe Pod my-pod -n prod --context=prod-cluster"
    );
}

#[test]
fn test_hotkey_expand_command_missing_values_become_empty() {
    let hotkey = CustomHotkey {
        key: "Ctrl+L".to_string(),
        command: "stern {{name}} -n {{namespace}} --context={{context}}".to_string(),
        description: "Logs".to_string(),
        requires_selection: false,
        open_terminal: true,
    };
    let ctx = TemplateContext::default(); // all None
    assert_eq!(hotkey.expand_command(&ctx), "stern  -n  --context=");
}

#[test]
fn test_tool_expand_args_all_values() {
    let tool = ExternalTool {
        name: "stern".to_string(),
        command: "stern".to_string(),
        args: vec![
            "{{name}}".to_string(),
            "--namespace".to_string(),
            "{{namespace}}".to_string(),
            "--context".to_string(),
            "{{context}}".to_string(),
        ],
        description: None,
    };
    let ctx = TemplateContext {
        namespace: Some("default".to_string()),
        name: Some("my-pod".to_string()),
        kind: Some("Pod".to_string()),
        context: Some("dev".to_string()),
    };
    assert_eq!(
        tool.expand_args(&ctx),
        vec!["my-pod", "--namespace", "default", "--context", "dev"]
    );
}

#[test]
fn test_tool_expand_args_missing_values_become_empty() {
    let tool = ExternalTool {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec!["ns={{namespace}}".to_string(), "n={{name}}".to_string()],
        description: None,
    };
    let ctx = TemplateContext::default();
    assert_eq!(tool.expand_args(&ctx), vec!["ns=", "n="]);
}

#[test]
fn test_tool_expand_args_empty_args() {
    let tool = ExternalTool {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        description: None,
    };
    let ctx = TemplateContext {
        namespace: Some("default".to_string()),
        ..Default::default()
    };
    let expanded: Vec<String> = tool.expand_args(&ctx);
    assert!(expanded.is_empty());
}

#[test]
fn test_tool_expand_args_drops_empty_flag_value_pairs() {
    // Simulates: kubectl --context kind-dev --namespace {{namespace}}
    // When namespace is None, --namespace and "" should both be dropped
    let tool = ExternalTool {
        name: "kubectl".to_string(),
        command: "kubectl".to_string(),
        args: vec![
            "--context".to_string(),
            "{{context}}".to_string(),
            "--namespace".to_string(),
            "{{namespace}}".to_string(),
        ],
        description: None,
    };
    let ctx = TemplateContext {
        context: Some("kind-dev".to_string()),
        namespace: None,
        ..Default::default()
    };
    assert_eq!(tool.expand_args(&ctx), vec!["--context", "kind-dev"]);
}

#[test]
fn test_tool_expand_args_drops_standalone_empty() {
    // Simulates: stern {{name}} --namespace {{namespace}}
    // When name is None, the empty standalone arg is dropped
    let tool = ExternalTool {
        name: "stern".to_string(),
        command: "stern".to_string(),
        args: vec![
            "{{name}}".to_string(),
            "--namespace".to_string(),
            "{{namespace}}".to_string(),
        ],
        description: None,
    };
    let ctx = TemplateContext {
        namespace: Some("default".to_string()),
        name: None,
        ..Default::default()
    };
    assert_eq!(tool.expand_args(&ctx), vec!["--namespace", "default"]);
}

// --- Hotkey parsing ---

#[test]
fn test_parse_single_key() {
    let h = ParsedHotkey::parse("L");
    assert!(!h.ctrl);
    assert!(!h.shift);
    assert!(!h.alt);
    assert!(!h.meta);
    assert_eq!(h.key, "L");
}

#[test]
fn test_parse_all_modifiers() {
    let h = ParsedHotkey::parse("Ctrl+Shift+Alt+Meta+X");
    assert!(h.ctrl);
    assert!(h.shift);
    assert!(h.alt);
    assert!(h.meta);
    assert_eq!(h.key, "X");
}

#[test]
fn test_parse_modifier_aliases() {
    // Control = Ctrl
    let h = ParsedHotkey::parse("Control+A");
    assert!(h.ctrl);
    assert_eq!(h.key, "A");

    // Option = Alt
    let h = ParsedHotkey::parse("Option+B");
    assert!(h.alt);
    assert_eq!(h.key, "B");

    // Command/Cmd/Win/Super = Meta
    for modifier in &["Command", "Cmd", "Win", "Super"] {
        let h = ParsedHotkey::parse(&format!("{}+C", modifier));
        assert!(h.meta, "modifier '{}' should set meta", modifier);
        assert_eq!(h.key, "C");
    }
}

#[test]
fn test_parse_whitespace_tolerance() {
    let h = ParsedHotkey::parse("Ctrl + Shift + L");
    assert!(h.ctrl);
    assert!(h.shift);
    assert_eq!(h.key, "L");
}

// --- Hotkey matching ---

#[test]
fn test_matches_exact() {
    let h = ParsedHotkey::parse("Ctrl+Shift+L");
    assert!(h.matches("L", true, true, false, false));
    assert!(h.matches("l", true, true, false, false)); // case insensitive
}

#[test]
fn test_matches_missing_required_modifier_fails() {
    let h = ParsedHotkey::parse("Ctrl+Shift+L");
    assert!(!h.matches("L", false, true, false, false)); // missing ctrl
    assert!(!h.matches("L", true, false, false, false)); // missing shift
}

#[test]
fn test_matches_extra_shift_tolerated() {
    // "Ctrl+L" should match even with extra Shift (platform variance)
    let h = ParsedHotkey::parse("Ctrl+L");
    assert!(h.matches("L", true, false, false, false)); // normal
    assert!(h.matches("L", true, true, false, false)); // extra Shift OK
}

#[test]
fn test_matches_extra_ctrl_rejected() {
    let h = ParsedHotkey::parse("L");
    assert!(h.matches("L", false, false, false, false));
    assert!(!h.matches("L", true, false, false, false)); // extra Ctrl rejected
}

#[test]
fn test_matches_extra_alt_rejected() {
    let h = ParsedHotkey::parse("Ctrl+L");
    assert!(!h.matches("L", true, false, true, false)); // extra Alt rejected
}

#[test]
fn test_matches_extra_meta_rejected() {
    let h = ParsedHotkey::parse("Ctrl+L");
    assert!(!h.matches("L", true, false, false, true)); // extra Meta rejected
}

#[test]
fn test_matches_wrong_key() {
    let h = ParsedHotkey::parse("Ctrl+L");
    assert!(!h.matches("K", true, false, false, false));
}

// --- YAML round-trip ---

#[test]
fn test_config_yaml_roundtrip() {
    let config = PluginConfig::with_defaults();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: PluginConfig = serde_yaml::from_str(&yaml).unwrap();

    // Aliases preserved
    assert_eq!(parsed.aliases.len(), config.aliases.len());
    for (k, v) in &config.aliases {
        assert_eq!(parsed.aliases.get(k), Some(v));
    }

    // Tools preserved
    assert_eq!(parsed.tools.len(), config.tools.len());
    for (orig, parsed_tool) in config.tools.iter().zip(parsed.tools.iter()) {
        assert_eq!(orig.name, parsed_tool.name);
        assert_eq!(orig.command, parsed_tool.command);
        assert_eq!(orig.args, parsed_tool.args);
    }
}

// --- Default tools ---

#[test]
fn test_defaults_include_echo_test_tool() {
    let config = PluginConfig::with_defaults();
    let echo_test = config.tools.iter().find(|t| t.name == "echo-test");
    assert!(
        echo_test.is_some(),
        "defaults should include echo-test tool"
    );
    let tool = echo_test.unwrap();
    assert_eq!(tool.command, "echo");
    assert!(tool.args[0].contains("{{namespace}}"));
    assert!(tool.args[0].contains("{{name}}"));
    assert!(tool.args[0].contains("{{kind}}"));
    assert!(tool.args[0].contains("{{context}}"));
}

// --- check_command_exists ---

#[test]
fn test_check_command_exists_echo() {
    assert!(ks_plugin::check_command_exists("echo"));
}

#[test]
fn test_check_command_exists_nonexistent() {
    assert!(!ks_plugin::check_command_exists(
        "kubestudio_nonexistent_tool_xyz_123"
    ));
}

// --- Save and reload ---

#[test]
fn test_save_and_reload() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.yaml");

    let mut config = PluginConfig::with_defaults();
    config
        .aliases
        .insert("custom".to_string(), "pods".to_string());
    config.hotkeys.push(CustomHotkey {
        key: "Alt+T".to_string(),
        command: "echo hello".to_string(),
        description: "test".to_string(),
        requires_selection: false,
        open_terminal: false,
    });

    // Save manually
    let yaml = serde_yaml::to_string(&config).unwrap();
    std::fs::write(&path, yaml).unwrap();

    // Reload
    let loaded = PluginConfig::load_from_path(&path).unwrap();
    assert_eq!(loaded.resolve_alias("custom"), Some(&"pods".to_string()));
    assert_eq!(loaded.hotkeys.len(), 1);
    assert_eq!(loaded.hotkeys[0].key, "Alt+T");
}

// --- Keybinding loading ---

#[test]
fn test_load_config_with_keybindings_section() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"keybindings:
  pods: "x"
  describe: "y"
"#
    )
    .unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    // Overridden values
    assert_eq!(config.keybindings.pods, "x");
    assert_eq!(config.keybindings.describe, "y");
    // Defaults preserved for non-overridden fields
    assert_eq!(config.keybindings.overview, "o");
    assert_eq!(config.keybindings.deployments, "2");
    assert_eq!(config.keybindings.services, "3");
    assert_eq!(config.keybindings.delete, "Ctrl+d");
    assert_eq!(config.keybindings.logs, "l");
}

#[test]
fn test_load_config_without_keybindings_has_defaults() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "aliases:\n  dp: deployments").unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    assert_eq!(config.keybindings, KeyBindings::default());
}

#[test]
fn test_keybindings_matches_after_override() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"keybindings:
  pods: "x"
"#
    )
    .unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    // "x" should now match pods
    assert!(
        config
            .keybindings
            .matches("pods", "x", false, false, false, false)
    );
    // "p" should no longer match pods
    assert!(
        !config
            .keybindings
            .matches("pods", "p", false, false, false, false)
    );
    // Other defaults still work
    assert!(
        config
            .keybindings
            .matches("overview", "o", false, false, false, false)
    );
    assert!(
        config
            .keybindings
            .matches("delete", "d", true, false, false, false)
    );
}

#[test]
fn test_keybindings_display_after_override() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"keybindings:
  pods: "x"
  delete: "Ctrl+x"
"#
    )
    .unwrap();

    let config = PluginConfig::load_from_path(tmp.path()).unwrap();
    assert_eq!(config.keybindings.display("pods"), "x");
    assert_eq!(config.keybindings.display("delete"), "Ctrl+x");
    // Defaults preserved
    assert_eq!(config.keybindings.display("overview"), "o");
}

#[test]
fn test_keybindings_yaml_roundtrip() {
    let config = PluginConfig::with_defaults();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: PluginConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed.keybindings, config.keybindings);
}
