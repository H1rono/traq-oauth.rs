use anyhow::anyhow;
use tokio::process::Command;
use tokio::sync::mpsc;

pub mod client;
pub mod config;
pub mod routes;

pub const CREDENTIAL_FILE_PATH: &str = "credential.json";
pub const API_BASE_PATH: &str = "https://q.trap.jp/api/v3";

pub async fn load_client(config: config::Config) -> anyhow::Result<client::Client> {
    let config::Config {
        client_id,
        access_token,
    } = config;
    let builder = client::Client::builder()
        .client_id(client_id)
        .api_base_path(API_BASE_PATH);
    let client = if let Some(access_token) = access_token {
        builder.access_token(access_token).build()
    } else {
        let client = builder.build();
        oauth2_authorize(client).await?
    };
    Ok(client)
}

pub async fn oauth2_authorize(client: client::Client) -> anyhow::Result<client::Client> {
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
