use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct McpClient {
    base_url: String,
    api_key: Option<String>,
    client: Client,
    session_id: Option<String>,
    initialized: bool,
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
            session_id: None,
            initialized: false,
        })
    }

    pub fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        self.ensure_initialized()?;
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
        if let Some(session_id) = &self.session_id {
            req = req.header("mcp-session-id", session_id);
        }

        let resp = req.send().context("send mcp request")?;
        let (status, _headers, value) = parse_mcp_response(resp)?;
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

    fn ensure_initialized(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("time")?
            .as_millis();
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "penpot-cli",
                    "version": env!("CARGO_PKG_VERSION")
                }
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

        let resp = req.send().context("send mcp initialize")?;
        let (status, headers, value) = parse_mcp_response(resp)?;
        if !status.is_success() {
            return Err(anyhow!("http {}: {}", status, value));
        }

        if let Some(error) = value.get("error") {
            return Err(anyhow!("mcp initialize error: {error}"));
        }

        let session_id = headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("mcp session id missing from initialize response"))?;
        self.session_id = Some(session_id);

        let notify = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let mut notify_req = self
            .client
            .post(&self.base_url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&notify);

        if let Some(key) = &self.api_key {
            notify_req = notify_req.header("x-api-key", key);
        }
        if let Some(session_id) = &self.session_id {
            notify_req = notify_req.header("mcp-session-id", session_id);
        }

        let notify_resp = notify_req.send().context("send mcp initialized")?;
        let notify_status = notify_resp.status();
        if !notify_status.is_success() {
            return Err(anyhow!("http {}: mcp initialized failed", notify_status));
        }

        self.initialized = true;
        Ok(())
    }
}

fn parse_mcp_response(resp: reqwest::blocking::Response) -> Result<(reqwest::StatusCode, reqwest::header::HeaderMap, Value)> {
    let status = resp.status();
    let headers = resp.headers().clone();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let body = resp.text().context("read mcp response body")?;
    let json_body = if content_type.starts_with("text/event-stream") {
        let mut data_lines = Vec::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.trim_start());
            }
        }
        if data_lines.is_empty() {
            return Err(anyhow!("mcp sse response missing data"));
        }
        data_lines.join("\n")
    } else {
        body
    };
    let value: Value = serde_json::from_str(&json_body).context("decode mcp json")?;
    Ok((status, headers, value))
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
