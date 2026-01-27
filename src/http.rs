use anyhow::{Context, Result, anyhow};
use reqwest::blocking::Client;
use serde_json::{Value, json};

pub struct HttpClient {
    token: String,
    client: Client,
}

impl HttpClient {
    pub fn new(token: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("penpot-cli")
            .build()
            .context("build http client")?;
        Ok(Self { token, client })
    }

    pub fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let auth = normalize_token(&self.token);
        let resp = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .header("authorization", auth)
            .json(body)
            .send()
            .context("send request")?;

        let status = resp.status();
        let value: Value = resp.json().context("decode json")?;

        if !status.is_success() {
            return Err(anyhow!("http {}: {}", status, value));
        }
        Ok(value)
    }
}

fn normalize_token(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    if lower.starts_with("token ") {
        token.to_string()
    } else {
        format!("Token {}", token)
    }
}

pub fn build_empty_body() -> Value {
    json!({})
}
