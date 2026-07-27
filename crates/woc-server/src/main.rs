//! Thin authoritative-host scaffold.
//!
//! v0.1 exposes health/version only. A future release will host `woc-sim` over WebSocket
//! using the same protocol types as the offline Bevy host.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use woc_version::{footer, VersionInfo};

#[derive(Serialize)]
struct Health {
    ok: bool,
    service: &'static str,
    footer: String,
}

async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        service: "woc-server",
        footer: footer(),
    })
}

async fn version() -> Json<VersionInfo> {
    Json(VersionInfo::current())
}

async fn ws_placeholder() -> &'static str {
    "WebSocket game host not implemented in rewrite 0.1.0 (combat-slice is offline Bevy). \
     Upstream pin: see GET /version."
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/ws", get(ws_placeholder))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8787));
    tracing::info!("woc-server listening on http://{addr} ({})", footer());
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
