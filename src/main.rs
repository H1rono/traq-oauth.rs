use tokio::process::Command;
use tracing_subscriber::EnvFilter;

mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or("info".into()))
        .init();
    let client_id = std::env::var("CLIENT_ID")?;
    let route = tokio::spawn(routes::listen());
    const OAUTH_ENDPOINT: &str = "https://q.trap.jp/api/v3/oauth2/authorize";
    Command::new("open")
        .arg(format!(
            "{OAUTH_ENDPOINT}?response_type=code&client_id={client_id}"
        ))
        .output()
        .await?;
    route.await??;
    Ok(())
}
