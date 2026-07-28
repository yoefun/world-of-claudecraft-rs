//! Authoritative host: HTTP health/version + auth/characters + WebSocket game loop.

mod auth;
mod characters;
mod game_ws;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use woc_persist::Persist;
use woc_version::{footer, VersionInfo};

pub struct AppState {
    pub persist: Persist,
}

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let persist = Persist::from_env()
        .await
        .expect("persist backend (set DATABASE_URL for Postgres, else in-memory)");
    let state = Arc::new(AppState { persist });

    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/ws/game", get(game_ws::ws_handler))
        .route("/ws", get(game_ws::ws_handler))
        .merge(auth::router())
        .merge(characters::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8787));
    tracing::info!("woc-server listening on http://{addr} ({})", footer());
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
