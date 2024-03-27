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

    let (stamp_name, file_path) = load_args()?;
    tracing::debug!(%stamp_name, %file_path);
    let content = std::fs::read(file_path)?;
    let stamp_info = client.add_stamp(&stamp_name, &content).await?;
    tracing::info!("added stamp information is: {stamp_info:?}");
    Ok(())
}

fn load_args() -> anyhow::Result<(String, String)> {
    macro_rules! try_next {
        ($args:ident) => {
            $args
                .next()
                .ok_or(::anyhow::anyhow!("argument is too short"))?
        };
    }

    let mut args = std::env::args();
    let _ = try_next!(args);
    let stamp_name = try_next!(args);
    let file_path = try_next!(args);
    Ok((stamp_name, file_path))
}
