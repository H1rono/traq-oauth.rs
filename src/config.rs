use std::{env, fs::File, io::BufReader, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub client_id: String,
    pub access_token: Option<String>,
    // TODO: expires_in, refresh_token, id_token
}

#[allow(unused)]
impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // TODO: provide default client_id
        let client_id = env::var("TRAQ_CLIENT_ID").with_context(|| "TRAQ_CLIENT_ID")?;
        let access_token = env::var("TRAQ_CLIENT_TOKEN").ok();
        Ok(Self {
            client_id,
            access_token,
        })
    }

    /// as json
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let value = serde_json::from_reader(reader)?;
        Ok(value)
    }

    /// priority: env-vars > file
    pub fn from_env_or_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::from_env().or(Self::from_file(path))
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
