use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct McpClient {
    base_url: String,
    api_key: Option<String>,
    client: Client,
}

impl McpClient {
    pub fn new(base_url: String, api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .user_agent("penpot-cli")
            .build()
            .context("build mcp http client")?;
        Ok(Self {
            base_url,
            api_key,
            client,
        })
    }

    pub fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("time")?
            .as_millis();
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args
            }
        });

        let mut req = self
            .client
            .post(&self.base_url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&body);

        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.send().context("send mcp request")?;
        let status = resp.status();
        let value: Value = resp.json().context("decode mcp json")?;
        if !status.is_success() {
            return Err(anyhow!("http {}: {}", status, value));
        }

        if let Some(error) = value.get("error") {
            return Err(anyhow!("mcp error: {error}"));
        }
        let result = value
            .get("result")
            .ok_or_else(|| anyhow!("mcp response missing result"))?;
        Ok(result.clone())
    }
}

pub fn infer_mime(path: &Path) -> Result<&'static str> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return Err(anyhow!("unsupported image format: .{ext}")),
    };
    Ok(mime)
}

pub fn base64_encode(bytes: &[u8]) -> String {
    general_purpose::STANDARD.encode(bytes)
}
