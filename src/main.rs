use std::collections::HashMap;

use reqwest::Client;
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

mod routes;

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or("info".into()))
        .init();
    let client_id = std::env::var("CLIENT_ID")?;
    let (code_tx, mut code_rx) = mpsc::channel(2);

    let route_state = routes::AppState::new(code_tx);
    let route = tokio::spawn(routes::listen(route_state));

    const OAUTH_ENDPOINT: &str = "https://q.trap.jp/api/v3/oauth2/authorize";
    Command::new("open")
        .arg(format!(
            "{OAUTH_ENDPOINT}?response_type=code&client_id={client_id}"
        ))
        .output()
        .await?;

    let Some(code) = code_rx.recv().await else {
        tracing::error!("channel closed unexpectedly");
        return Ok(());
    };
    let token_request_form: HashMap<&str, &str> = [
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("code", &code),
    ]
    .into_iter()
    .collect();
    let client = Client::new();
    let token_response: Value = client
        .post("https://q.trap.jp/api/v3/oauth2/token")
        .form(&token_request_form)
        .send()
        .await?
        .json()
        .await?;
    tracing::debug!("received token: {token_response}");

    let Some(token) = token_response.get("access_token").and_then(|a| a.as_str()) else {
        tracing::error!("requested token was not found in the response {token_response}");
        return Ok(());
    };
    let me: Value = client
        .get("https://q.trap.jp/api/v3/users/me")
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;
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
