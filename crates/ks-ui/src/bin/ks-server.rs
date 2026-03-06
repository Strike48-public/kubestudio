//! KubeStudio Standalone Server
//!
//! Serves the KubeStudio UI as a web application via Dioxus liveview.
//! No Matrix connector or external dependencies required.
//!
//! Environment variables:
//! - `PORT` — Listen port (default: 8080)
//! - `RUST_LOG` — Log filter (default: info)
//! - `KUBECONFIG` — Path(s) to kubeconfig files (standard kube-rs behavior)

use axum::{Router, response::Redirect, routing::get};
use dioxus_liveview::LiveviewRouter as _;
use std::net::SocketAddr;
use tokio::signal;

/// Health check — always returns 200.
async fn health() -> &'static str {
    "OK"
}

/// Readiness check — returns 200 if at least one kubeconfig is loadable.
async fn readiness() -> Result<&'static str, axum::http::StatusCode> {
    match kube::config::Kubeconfig::read() {
        Ok(_) => Ok("OK"),
        Err(_) => {
            // Also try in-cluster config
            match kube::Config::incluster() {
                Ok(_) => Ok("OK"),
                Err(_) => Err(axum::http::StatusCode::SERVICE_UNAVAILABLE),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let router = Router::new()
        .with_app("/", ks_ui::App)
        .route("/", get(|| async { Redirect::temporary("/liveview") }))
        .route("/health", get(health))
        .route("/readiness", get(readiness));

    tracing::info!("KubeStudio server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind to address");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received SIGINT, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }
}
