use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

mod client;
mod config;
mod routes;

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or("info".into()))
        .init();
    let client_id = std::env::var("CLIENT_ID")?;
    let (code_tx, mut code_rx) = mpsc::channel(2);
    let client = client::Client::builder()
        .client_id(client_id)
        .api_base_path("https://q.trap.jp/api/v3")
        .build();

    let route_state = routes::AppState::new(code_tx);
    let route = tokio::spawn(routes::listen(([0, 0, 0, 0], 8080), route_state));

    // FIXME: macOS only
    Command::new("open")
        .arg(client.authorize_endpoint())
        .output()
        .await?;

    let Some(code) = code_rx.recv().await else {
        tracing::error!("channel closed unexpectedly");
        return Ok(());
    };
    let client = client.authorize_with(&code).await?;

    let me: Value = client.get_me().await?;
    tracing::debug!("your info: {me}");
    let id = me.get("id").and_then(|i| i.as_str());
    let name = me.get("name").and_then(|n| n.as_str());
    let Some((id, name)) = id.zip(name) else {
        tracing::error!("user info was not found in the response {me}");
        return Ok(());
    };
    tracing::info!("Hello, {name}! Your id is {id}");

    route.await??;
    Ok(())
}
