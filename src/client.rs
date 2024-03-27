use std::{borrow::Cow, collections::HashMap};

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

#[derive(Clone, Default)]
pub struct ClientBuilder {
    api_base_path: Option<String>,
    client_id: Option<String>,
    access_token: Option<String>,
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

#[allow(unused)]
impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Client {
        let ClientBuilder {
            api_base_path,
            client_id,
            access_token,
        } = self;
        let api_base_path = api_base_path.unwrap_or_else(|| "https://q.trap.jp/api/v3".to_string());
        let client_id = client_id.expect("client_id is not set");
        Client {
            inner: reqwest::Client::new(),
            api_base_path,
            client_id,
            access_token,
        }
    }

    pub fn api_base_path<'a, S>(self, base_path: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        let base_path = base_path.into().into_owned();
        Self {
            api_base_path: Some(base_path),
            ..self
        }
    }

    pub fn client_id<'a, S>(self, client_id: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        let client_id = client_id.into().into_owned();
        Self {
            client_id: Some(client_id),
            ..self
        }
    }

    pub fn access_token<'a, S>(self, access_token: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        let access_token = access_token.into().into_owned();
        Self {
            access_token: Some(access_token),
            ..self
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
            ("code", &code),
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

    #[tracing::instrument(skip(self, body))]
    pub async fn add_stamp(&self, name: &str, body: &[u8]) -> anyhow::Result<Stamp> {
        let Some(access_token) = &self.access_token else {
            return Err(anyhow!("authorize required before calling API"));
        };
        let url = format!("{}/stamps", self.api_base_path);
        let file_content = body.to_vec();
        let file_part = multipart::Part::bytes(file_content);
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
        let response = response.json().await?;
        Ok(response)
    }
}
