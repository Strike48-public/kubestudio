use crate::components::{Command, Hotkey};
use crate::hooks::ViewState;
use ks_plugin::PluginConfig;

/// Get the command palette commands
pub fn get_commands() -> Vec<Command> {
    vec![
        Command {
            id: "overview".to_string(),
            label: "Cluster Overview".to_string(),
            shortcut: Some("o".to_string()),
        },
        Command {
            id: "pods".to_string(),
            label: "View Pods".to_string(),
            shortcut: Some("1/p".to_string()),
        },
        Command {
            id: "deployments".to_string(),
            label: "View Deployments".to_string(),
            shortcut: Some("2".to_string()),
        },
        Command {
            id: "services".to_string(),
            label: "View Services".to_string(),
            shortcut: Some("3".to_string()),
        },
    ]
}

/// Get the command palette commands including external tools from plugin config
pub fn get_commands_with_tools(config: &PluginConfig) -> Vec<Command> {
    let mut commands = get_commands();

    // Add external tools from plugin config
    for tool in &config.tools {
        commands.push(Command {
            id: format!("tool:{}", tool.name),
            label: format!("Launch {}", tool.name),
            shortcut: None,
        });
    }

    commands
}

/// Get context-aware hotkeys based on current navigation state and view
#[allow(clippy::too_many_arguments)]
pub fn get_hotkeys(
    nav_state: &ViewState,
    current_view: &str,
    is_pods_view: bool,
    is_services_view: bool,
    is_deployments_view: bool,
    is_statefulsets_view: bool,
    is_daemonsets_view: bool,
    is_cronjobs_view: bool,
) -> Vec<Hotkey> {
    let is_resource_view = current_view != "overview";

    match nav_state {
        ViewState::CreateResource => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "Select".to_string(),
            },
            Hotkey {
                key: "Ctrl+S".to_string(),
                description: "Apply".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::ApplyFile { .. } => vec![
            Hotkey {
                key: "Ctrl+Enter".to_string(),
                description: "Apply".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Cancel".to_string(),
            },
        ],
        ViewState::YamlViewer { .. } => vec![
            Hotkey {
                key: "c".to_string(),
                description: "Copy".to_string(),
            },
            Hotkey {
                key: "h".to_string(),
                description: "Toggle View".to_string(),
            },
            Hotkey {
                key: "w".to_string(),
                description: "Toggle Wrap".to_string(),
            },
            Hotkey {
                key: "↑↓←→".to_string(),
                description: "Scroll".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::LogViewer { .. } => vec![
            Hotkey {
                key: "w".to_string(),
                description: "Toggle Wrap".to_string(),
            },
            Hotkey {
                key: "↑↓←→".to_string(),
                description: "Scroll".to_string(),
            },
            Hotkey {
                key: "PgUp/Dn".to_string(),
                description: "Page".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::ExecViewer { .. } => vec![
            Hotkey {
                key: "Type".to_string(),
                description: "Input".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "Run".to_string(),
            },
            Hotkey {
                key: "Ctrl+C".to_string(),
                description: "Interrupt".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::ContainerDrillDown { .. } => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "View Logs".to_string(),
            },
            Hotkey {
                key: "l".to_string(),
                description: "Logs".to_string(),
            },
            Hotkey {
                key: "s".to_string(),
                description: "Shell".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::DeploymentPods { .. }
        | ViewState::StatefulSetPods { .. }
        | ViewState::DaemonSetPods { .. }
        | ViewState::JobPods { .. } => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "Containers".to_string(),
            },
            Hotkey {
                key: "d".to_string(),
                description: "Describe".to_string(),
            },
            Hotkey {
                key: "l".to_string(),
                description: "Logs".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::CronJobJobs { .. } => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "View Pods".to_string(),
            },
            Hotkey {
                key: "d".to_string(),
                description: "Describe".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::ServiceEndpoints { .. } | ViewState::IngressBackends { .. } => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "Details".to_string(),
            },
            Hotkey {
                key: "d".to_string(),
                description: "Describe".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::PvcPods { .. } => vec![
            Hotkey {
                key: "↑↓".to_string(),
                description: "Navigate".to_string(),
            },
            Hotkey {
                key: "Enter".to_string(),
                description: "Containers".to_string(),
            },
            Hotkey {
                key: "d".to_string(),
                description: "Describe".to_string(),
            },
            Hotkey {
                key: "Esc".to_string(),
                description: "Back".to_string(),
            },
        ],
        ViewState::ResourceList => {
            if is_resource_view {
                let mut keys = vec![
                    Hotkey {
                        key: "d".to_string(),
                        description: "Describe".to_string(),
                    },
                    Hotkey {
                        key: "^d".to_string(),
                        description: "Delete".to_string(),
                    },
                    Hotkey {
                        key: "^k".to_string(),
                        description: "Kill".to_string(),
                    },
                ];
                if is_pods_view {
                    keys.push(Hotkey {
                        key: "l".to_string(),
                        description: "Logs".to_string(),
                    });
                    keys.push(Hotkey {
                        key: "s".to_string(),
                        description: "Shell".to_string(),
                    });
                    keys.push(Hotkey {
                        key: "f".to_string(),
                        description: "Forward".to_string(),
                    });
                } else if is_services_view {
                    keys.push(Hotkey {
                        key: "f".to_string(),
                        description: "Forward".to_string(),
                    });
                } else if is_deployments_view {
                    keys.push(Hotkey {
                        key: "+/-".to_string(),
                        description: "Scale".to_string(),
                    });
                    keys.push(Hotkey {
                        key: "R".to_string(),
                        description: "Restart".to_string(),
                    });
                } else if is_statefulsets_view || is_daemonsets_view {
                    keys.push(Hotkey {
                        key: "R".to_string(),
                        description: "Restart".to_string(),
                    });
                } else if is_cronjobs_view {
                    keys.push(Hotkey {
                        key: "T".to_string(),
                        description: "Trigger".to_string(),
                    });
                }
                keys.extend(vec![
                    Hotkey {
                        key: "/".to_string(),
                        description: "Search".to_string(),
                    },
                    Hotkey {
                        key: "n".to_string(),
                        description: "Namespace".to_string(),
                    },
                    Hotkey {
                        key: "←→".to_string(),
                        description: "Sidebar".to_string(),
                    },
                    Hotkey {
                        key: "Esc".to_string(),
                        description: "Back".to_string(),
                    },
                ]);
                keys
            } else {
                vec![
                    Hotkey {
                        key: "o".to_string(),
                        description: "Overview".to_string(),
                    },
                    Hotkey {
                        key: "1/p".to_string(),
                        description: "Pods".to_string(),
                    },
                    Hotkey {
                        key: "2".to_string(),
                        description: "Deployments".to_string(),
                    },
                    Hotkey {
                        key: "3".to_string(),
                        description: "Services".to_string(),
                    },
                    Hotkey {
                        key: "v".to_string(),
                        description: "Events".to_string(),
                    },
                    Hotkey {
                        key: "/".to_string(),
                        description: "Search".to_string(),
                    },
                    Hotkey {
                        key: ":".to_string(),
                        description: "Command".to_string(),
                    },
                    Hotkey {
                        key: "n".to_string(),
                        description: "Namespace".to_string(),
                    },
                    Hotkey {
                        key: "?".to_string(),
                        description: "Help".to_string(),
                    },
                ]
            }
        }
    }
}
