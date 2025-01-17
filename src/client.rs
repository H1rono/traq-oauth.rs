use std::{collections::HashMap, fmt::Display, path::Path};

use anyhow::anyhow;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config;

#[derive(Clone)]
pub struct Client {
    inner: reqwest::Client,
    api_base_path: String,
    client_id: String,
    access_token: Option<String>,
}

pub struct ClientBuilder<ApiBasePath = (), ClientId = ()> {
    api_base_path: ApiBasePath,
    client_id: ClientId,
    access_token: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum ImageMime {
    Jpeg,
    Gif,
    Png,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    id: String,
    name: String,
    creator_id: String,
    created_at: String,
    updated_at: String,
    file_id: String,
    is_unicode: bool,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_base_path: (),
            client_id: (),
            access_token: None,
        }
    }
}

#[allow(unused)]
impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<ApiBasePath, ClientId> ClientBuilder<ApiBasePath, ClientId> {
    pub fn api_base_path(self, value: impl Into<String>) -> ClientBuilder<String, ClientId> {
        let Self {
            api_base_path: _,
            client_id,
            access_token,
        } = self;
        ClientBuilder {
            api_base_path: value.into(),
            client_id,
            access_token,
        }
    }

    pub fn client_id(self, value: impl Into<String>) -> ClientBuilder<ApiBasePath, String> {
        let Self {
            api_base_path,
            client_id: _,
            access_token,
        } = self;
        ClientBuilder {
            api_base_path,
            client_id: value.into(),
            access_token,
        }
    }

    pub fn access_token(self, value: impl Into<String>) -> ClientBuilder<ApiBasePath, ClientId> {
        let Self {
            api_base_path,
            client_id,
            access_token: _,
        } = self;
        ClientBuilder {
            api_base_path,
            client_id,
            access_token: Some(value.into()),
        }
    }
}

impl ClientBuilder<String, String> {
    pub fn build(self) -> Client {
        let Self {
            api_base_path,
            client_id,
            access_token,
        } = self;
        Client {
            inner: reqwest::Client::new(),
            api_base_path,
            client_id,
            access_token,
        }
    }
}

#[allow(unused)]
impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn export_config(&self) -> config::Config {
        config::Config {
            client_id: self.client_id.clone(),
            access_token: self.access_token.clone(),
        }
    }

    pub fn authorize_endpoint(&self) -> String {
        let Self {
            api_base_path,
            client_id,
            ..
        } = self;
        format!("{api_base_path}/oauth2/authorize?response_type=code&client_id={client_id}")
    }

    #[tracing::instrument(skip(self), fields(%self.client_id))]
    pub async fn authorize_with(self, code: &str) -> anyhow::Result<Self> {
        let Self {
            inner,
            api_base_path,
            client_id,
            ..
        } = self;
        let token_request_form: HashMap<&str, &str> = [
            ("grant_type", "authorization_code"),
            ("client_id", &client_id),
            ("code", code),
        ]
        .into_iter()
        .collect();
        let token_response: Value = inner
            .post(format!("{api_base_path}/oauth2/token"))
            .form(&token_request_form)
            .send()
            .await?
            .json()
            .await?;
        tracing::debug!("received token: {token_response}");
        let access_token = token_response
            .get("access_token")
            .and_then(|a| a.as_str())
            .ok_or_else(|| anyhow!("received unexpected response: {token_response}"))?
            .to_string();
        Ok(Self {
            inner,
            api_base_path,
            client_id,
            access_token: Some(access_token),
        })
    }

    pub async fn get_me(&self) -> anyhow::Result<Value> {
        let Some(access_token) = &self.access_token else {
            return Err(anyhow!("authorize required before calling API"));
        };
        let url = format!("{}/users/me", self.api_base_path);
        let me: Value = self
            .inner
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await?
            .json()
            .await?;
        Ok(me)
    }

    #[tracing::instrument(skip(self, path))]
    pub async fn add_stamp(&self, name: &str, path: &str) -> anyhow::Result<Stamp> {
        let Some(access_token) = &self.access_token else {
            return Err(anyhow!("authorize required before calling API"));
        };
        let url = format!("{}/stamps", self.api_base_path);
        let file_content = std::fs::read(path)?;
        let mime = ImageMime::try_from_path(path)?;
        let path = path.to_string();
        let file_part = multipart::Part::bytes(file_content)
            .mime_str(&mime.to_string())?
            .file_name(path);
        let form = multipart::Form::new()
            .text("name", name.to_string())
            .part("file", file_part);
        let response = self
            .inner
            .post(url)
            .bearer_auth(access_token)
            .multipart(form)
            .send()
            .await?;
        tracing::debug!("POST /stamps: {}", response.status());
        if !response.status().is_success() {
            let body = response.text().await?;
            tracing::error!("error message: {body}");
            return Err(anyhow!("failed: {body}"));
        }
        let response = response.json().await?;
        Ok(response)
    }
}

impl Display for ImageMime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jpeg => write!(f, "image/jpeg"),
            Self::Gif => write!(f, "image/gif"),
            Self::Png => write!(f, "image/png"),
        }
    }
}

impl ImageMime {
    pub fn try_from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
            return Err(anyhow!("received invalid path format"));
        };
        match extension {
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "gif" => Ok(Self::Gif),
            "png" => Ok(Self::Png),
            _ => Err(anyhow!("unexpected extension")),
        }
    }
}
