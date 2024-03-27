use anyhow::anyhow;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

mod client;
mod config;
mod routes;

const CREDENTIAL_FILE_PATH: &str = "credential.json";
const API_BASE_PATH: &str = "https://q.trap.jp/api/v3";

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or("info".into()))
        .init();
    let config = config::Config::from_env_or_file(CREDENTIAL_FILE_PATH)?;
    let client = load_client(config).await?;
    client.export_config().save_to(CREDENTIAL_FILE_PATH)?;

    let me: Value = client.get_me().await?;
    tracing::debug!("your info: {me}");
    let id = me.get("id").and_then(|i| i.as_str());
    let name = me.get("name").and_then(|n| n.as_str());
    let Some((id, name)) = id.zip(name) else {
        tracing::error!("user info was not found in the response {me}");
        return Ok(());
    };
    tracing::info!("Hello, {name}! Your id is {id}");
    Ok(())
}

async fn load_client(config: config::Config) -> anyhow::Result<client::Client> {
    let config::Config {
        client_id,
        access_token,
    } = config;
    let client = if let Some(access_token) = access_token {
        client::Client::builder()
            .client_id(client_id)
            .access_token(access_token)
            .api_base_path(API_BASE_PATH)
            .build()
    } else {
        let client = client::Client::builder()
            .client_id(client_id)
            .api_base_path(API_BASE_PATH)
            .build();
        oauth2_authorize(client).await?
    };
    Ok(client)
}

async fn oauth2_authorize(client: client::Client) -> anyhow::Result<client::Client> {
    let (code_tx, mut code_rx) = mpsc::channel(2);
    let route_state = routes::AppState::new(code_tx);
    let route = tokio::spawn(routes::listen(([0, 0, 0, 0], 8080), route_state));
    // FIXME: macOS only
    Command::new("open")
        .arg(client.authorize_endpoint())
        .output()
        .await?;
    let code = code_rx
        .recv()
        .await
        .ok_or(anyhow!("channel closed unexpectedly"))?;
    let client = client.authorize_with(&code).await?;
    route.await??;
    Ok(client)
}
