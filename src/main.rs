use serde_json::Value;
use tracing_subscriber::EnvFilter;

use traq_oauth::{config, load_client, CREDENTIAL_FILE_PATH};

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
