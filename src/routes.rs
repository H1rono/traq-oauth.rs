use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::{extract, routing};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::Notify;

#[derive(Clone)]
pub struct AppState {
    notify: Arc<Notify>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AuthorizedQuery {
    code: String,
}

impl AppState {
    pub fn new() -> Self {
        let notify = Arc::new(Notify::new());
        Self { notify }
    }

    pub async fn notify_shutdown(&self) {
        self.notify.notify_one();
    }

    pub async fn wait_shutdown(&self) {
        self.notify.notified().await
    }
}

#[tracing::instrument(skip_all)]
async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::info!("{e}");
        }
        tracing::info!("ctrl_c");
    };
    let shutdown = async {
        state.wait_shutdown().await;
        tracing::info!("shutdown");
    };
    tokio::select! {
        _ = ctrl_c => {}
        _ = shutdown => {}
    }
}

async fn ping() -> &'static str {
    "pong"
}

async fn shutdown(st: extract::State<AppState>) -> &'static str {
    st.0.notify_shutdown().await;
    "shutdown"
}

#[tracing::instrument()]
async fn authorized(_params: extract::Query<AuthorizedQuery>) -> &'static str {
    tracing::info!("authorized");
    "success!"
}

pub async fn listen() -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;
    let state = AppState::new();
    let app = Router::new()
        .route("/ping", routing::get(ping))
        .route("/shutdown", routing::get(shutdown))
        .route("/_authorized", routing::get(authorized))
        .with_state(state.clone());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;
    Ok(())
}
