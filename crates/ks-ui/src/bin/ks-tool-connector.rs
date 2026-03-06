//! KubeStudio Tool Connector for Matrix
//!
//! This binary exposes KubeStudio capabilities as tools for AI agents via Matrix.
//! It implements the TOOL behavior from the Strike48 SDK.
//!
//! # Usage
//!
//! ```bash
//! TENANT_ID=non-prod STRIKE48_HOST=localhost:50061 cargo run --bin ks-tool-connector --features connector
//! ```
//!
//! # Available Tools
//!
//! - `list_clusters`: List available Kubernetes clusters from kubeconfig
//! - `get_current_context`: Get the current active context
//! - `get_cluster_info`: Get detailed info about a specific cluster

use ks_kube::auth;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use strike48_connector::*;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

/// Tool schemas for registration
fn tool_schemas() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "list_clusters",
            "description": "List all available Kubernetes clusters/contexts from the kubeconfig file. Returns context names, cluster endpoints, and which context is currently active.",
            "parameters": {
                "type": "object",
                "properties": {
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": []
            }
        },
        {
            "name": "get_current_context",
            "description": "Get the name of the currently active Kubernetes context from kubeconfig.",
            "parameters": {
                "type": "object",
                "properties": {
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": []
            }
        },
        {
            "name": "get_cluster_info",
            "description": "Get detailed information about a specific Kubernetes cluster context, including server endpoint, namespace, and user.",
            "parameters": {
                "type": "object",
                "properties": {
                    "context_name": {
                        "type": "string",
                        "description": "Name of the context to get info for"
                    },
                    "kubeconfig_path": {
                        "type": "string",
                        "description": "Optional path to kubeconfig file. If not provided, uses default location (~/.kube/config)"
                    }
                },
                "required": ["context_name"]
            }
        }
    ])
}

/// Request parameters for list_clusters
#[derive(Debug, Deserialize, Default)]
struct ListClustersParams {
    kubeconfig_path: Option<String>,
}

/// Request parameters for get_current_context
#[derive(Debug, Deserialize, Default)]
struct GetCurrentContextParams {
    kubeconfig_path: Option<String>,
}

/// Request parameters for get_cluster_info
#[derive(Debug, Deserialize)]
struct GetClusterInfoParams {
    context_name: String,
    kubeconfig_path: Option<String>,
}

/// KubeStudio Tool Connector
struct KubeStudioToolConnector;

impl KubeStudioToolConnector {
    async fn list_clusters(&self, params: ListClustersParams) -> serde_json::Value {
        let path = params.kubeconfig_path.map(std::path::PathBuf::from);

        match auth::load_kubeconfig(path).await {
            Ok(config) => {
                let contexts = auth::list_contexts(&config);
                let current = auth::current_context(&config);

                // Build detailed context list
                let context_details: Vec<serde_json::Value> = config
                    .contexts
                    .iter()
                    .map(|ctx| {
                        let is_current = current.as_ref() == Some(&ctx.name);
                        let cluster_name = ctx
                            .context
                            .as_ref()
                            .map(|c| c.cluster.clone())
                            .unwrap_or_default();
                        let namespace = ctx.context.as_ref().and_then(|c| c.namespace.clone());
                        let user = ctx
                            .context
                            .as_ref()
                            .map(|c| c.user.clone())
                            .unwrap_or_default();

                        // Find the cluster endpoint
                        let server = config
                            .clusters
                            .iter()
                            .find(|c| c.name == cluster_name)
                            .and_then(|c| c.cluster.as_ref())
                            .and_then(|c| c.server.clone());

                        serde_json::json!({
                            "name": ctx.name,
                            "cluster": cluster_name,
                            "namespace": namespace,
                            "user": user,
                            "server": server,
                            "is_current": is_current
                        })
                    })
                    .collect();

                serde_json::json!({
                    "success": true,
                    "current_context": current,
                    "contexts": context_details,
                    "count": contexts.len()
                })
            }
            Err(e) => serde_json::json!({
                "success": false,
                "error": format!("Failed to load kubeconfig: {}", e)
            }),
        }
    }

    async fn get_current_context(&self, params: GetCurrentContextParams) -> serde_json::Value {
        let path = params.kubeconfig_path.map(std::path::PathBuf::from);

        match auth::load_kubeconfig(path).await {
            Ok(config) => {
                let current = auth::current_context(&config);
                serde_json::json!({
                    "success": true,
                    "current_context": current
                })
            }
            Err(e) => serde_json::json!({
                "success": false,
                "error": format!("Failed to load kubeconfig: {}", e)
            }),
        }
    }

    async fn get_cluster_info(&self, params: GetClusterInfoParams) -> serde_json::Value {
        let path = params.kubeconfig_path.map(std::path::PathBuf::from);

        match auth::load_kubeconfig(path).await {
            Ok(config) => {
                // Find the requested context
                let context = config
                    .contexts
                    .iter()
                    .find(|c| c.name == params.context_name);

                match context {
                    Some(ctx) => {
                        let cluster_name = ctx
                            .context
                            .as_ref()
                            .map(|c| c.cluster.clone())
                            .unwrap_or_default();
                        let namespace = ctx.context.as_ref().and_then(|c| c.namespace.clone());
                        let user = ctx
                            .context
                            .as_ref()
                            .map(|c| c.user.clone())
                            .unwrap_or_default();

                        // Find cluster details
                        let cluster = config.clusters.iter().find(|c| c.name == cluster_name);

                        let (server, tls_server_name, insecure) = cluster
                            .and_then(|c| c.cluster.as_ref())
                            .map(|c| {
                                (
                                    c.server.clone(),
                                    c.tls_server_name.clone(),
                                    c.insecure_skip_tls_verify,
                                )
                            })
                            .unwrap_or((None, None, None));

                        let is_current = auth::current_context(&config).as_ref() == Some(&ctx.name);

                        serde_json::json!({
                            "success": true,
                            "context": {
                                "name": ctx.name,
                                "cluster": cluster_name,
                                "namespace": namespace,
                                "user": user,
                                "is_current": is_current
                            },
                            "cluster": {
                                "server": server,
                                "tls_server_name": tls_server_name,
                                "insecure_skip_tls_verify": insecure
                            }
                        })
                    }
                    None => serde_json::json!({
                        "success": false,
                        "error": format!("Context '{}' not found", params.context_name)
                    }),
                }
            }
            Err(e) => serde_json::json!({
                "success": false,
                "error": format!("Failed to load kubeconfig: {}", e)
            }),
        }
    }
}

impl BaseConnector for KubeStudioToolConnector {
    fn connector_type(&self) -> &str {
        "kubestudio-tools"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn behavior(&self) -> ConnectorBehavior {
        ConnectorBehavior::Tool
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("name".to_string(), "KubeStudio Tools".to_string());
        meta.insert(
            "description".to_string(),
            "Kubernetes cluster management tools for AI agents".to_string(),
        );
        meta.insert("vendor".to_string(), "Strike48".to_string());
        meta.insert("icon".to_string(), "hero-server-stack".to_string());

        // Tool schemas - REQUIRED for TOOL behavior
        meta.insert(
            "tool_schemas".to_string(),
            serde_json::to_string(&tool_schemas()).unwrap_or_default(),
        );
        meta.insert("tool_count".to_string(), "3".to_string());

        meta
    }

    fn execute(
        &self,
        request: serde_json::Value,
        _capability_id: Option<&str>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send + '_>>
    {
        Box::pin(async move {
            // Extract tool name and parameters
            let tool = request
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let params = request
                .get("parameters")
                .or_else(|| request.get("args"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let result = match tool {
                "list_clusters" => {
                    let req: ListClustersParams =
                        serde_json::from_value(params).unwrap_or_default();
                    self.list_clusters(req).await
                }

                "get_current_context" => {
                    let req: GetCurrentContextParams =
                        serde_json::from_value(params).unwrap_or_default();
                    self.get_current_context(req).await
                }

                "get_cluster_info" => {
                    let req: GetClusterInfoParams =
                        serde_json::from_value(params).map_err(|e| {
                            ConnectorError::InvalidConfig(format!("Invalid parameters: {e}"))
                        })?;
                    self.get_cluster_info(req).await
                }

                "" => {
                    return Err(ConnectorError::InvalidConfig(
                        "Missing 'tool' field in request".to_string(),
                    ));
                }

                _ => {
                    return Err(ConnectorError::InvalidConfig(format!(
                        "Unknown tool: {tool}"
                    )));
                }
            };

            Ok(serde_json::json!({
                "tool": tool,
                "result": result,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    tracing_subscriber::fmt::init();

    println!("=== KubeStudio Tool Connector ===");
    println!("Kubernetes cluster management tools for AI agents");
    println!();

    // Create connector configuration from environment
    let instance_id = std::env::var("INSTANCE_ID").unwrap_or_else(|_| {
        format!(
            "kubestudio-tools-{}",
            chrono::Utc::now().timestamp_millis() % 10000
        )
    });

    let config = ConnectorConfig {
        connector_type: "kubestudio-tools".to_string(),
        instance_id: instance_id.clone(),
        version: "1.0.0".to_string(),
        max_concurrent_requests: 100,
        display_name: std::env::var("CONNECTOR_NAME").ok(),
        ..ConnectorConfig::from_env()
    };

    println!("Configuration:");
    println!("  Host: {}", config.host);
    println!("  Tenant ID: {}", config.tenant_id);
    println!("  Instance ID: {}", config.instance_id);
    if let Some(name) = &config.display_name {
        println!("  Display Name: {name}");
    }
    println!();
    println!("Available Tools:");
    println!("  - list_clusters: List available Kubernetes clusters");
    println!("  - get_current_context: Get the current context name");
    println!("  - get_cluster_info: Get detailed info about a specific cluster");
    println!();

    let connector = Arc::new(KubeStudioToolConnector);
    let runner = ConnectorRunner::new(config, connector);

    // Get shutdown handle BEFORE running
    let shutdown_handle = runner.shutdown_handle();

    // Handle shutdown signals
    #[cfg(unix)]
    {
        let handle = shutdown_handle.clone();
        tokio::spawn(async move {
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    println!("\nReceived SIGTERM, shutting down...");
                }
                _ = sigint.recv() => {
                    println!("\nReceived SIGINT, shutting down...");
                }
            }

            handle.shutdown();
        });
    }

    #[cfg(not(unix))]
    {
        let handle = shutdown_handle.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to register Ctrl+C handler");
            println!("\nReceived Ctrl+C, shutting down...");
            handle.shutdown();
        });
    }

    // Run the connector
    println!("Starting connector...");
    if let Err(e) = runner.run().await {
        eprintln!("Connector error: {e}");
        std::process::exit(1);
    }

    Ok(())
}
