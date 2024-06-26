use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::{extract, routing};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{error::SendError, Sender};
use tokio::sync::Notify;

#[derive(Clone)]
pub struct AppState {
    code_sender: Option<Sender<String>>,
    notify: Arc<Notify>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AuthorizedQuery {
    code: String,
}

impl Default for AppState {
    fn default() -> Self {
        let notify = Arc::new(Notify::new());
        Self {
            code_sender: None,
            notify,
        }
    }
}

impl AppState {
    pub fn new(sender: Sender<String>) -> Self {
        let notify = Arc::new(Notify::new());
        Self {
            code_sender: Some(sender),
            notify,
        }
    }

    pub fn clone_without_sender(&self) -> Self {
        Self {
            code_sender: None,
            notify: self.notify.clone(),
        }
    }

    pub async fn send_code(&self, code: String) -> Result<(), SendError<String>> {
        let Some(sender) = &self.code_sender else {
            return Ok(());
        };
        sender.send(code).await
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

#[tracing::instrument(skip(st))]
async fn authorized(
    st: extract::State<AppState>,
    extract::Query(params): extract::Query<AuthorizedQuery>,
) -> &'static str {
    tracing::debug!("authorized");
    if let Err(e) = st.0.send_code(params.code).await {
        tracing::error!("failed to send authorization code: {e}");
    }
    st.0.notify_shutdown().await;
    "success! please close this page."
}

pub async fn listen(addr: impl Into<SocketAddr>, state: AppState) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr.into()).await?;
    let signal_state = state.clone_without_sender();
    let app = Router::new()
        .route("/ping", routing::get(ping))
        .route("/_authorized", routing::get(authorized))
        .with_state(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(signal_state))
        .await?;
    Ok(())
}
