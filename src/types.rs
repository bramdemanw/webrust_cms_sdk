use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- Hook Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub event: String,
    pub data: HashMap<String, serde_json::Value>,
}

impl HookContext {
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.data.get(key)?.as_str()
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        let val = self.data.get(key)?;
        val.as_u64().or_else(|| val.as_str()?.parse().ok())
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get_str(key).map(|s| s.to_string())
    }

    pub fn set(&mut self, key: &str, value: impl Into<serde_json::Value>) {
        self.data.insert(key.to_string(), value.into());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookResult {
    Continue(HookContext),
    Halt(HookContext),
}

impl HookResult {
    pub fn ok(ctx: HookContext) -> Self {
        Self::Continue(ctx)
    }

    pub fn halt(ctx: HookContext) -> Self {
        Self::Halt(ctx)
    }
}

// --- Route Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub body: Option<String>,
    pub headers: HashMap<String, String>,
    pub params: HashMap<String, String>,
}

impl RouteRequest {
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }

    pub fn body_json<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        let body = self.body.as_ref()?;
        serde_json::from_str(body).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl RouteResponse {
    pub fn html(status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
        Self {
            status,
            headers,
            body: body.to_string(),
        }
    }

    pub fn json(status: u16, value: &impl Serialize) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            status,
            headers,
            body: serde_json::to_string(value).unwrap_or_default(),
        }
    }

    pub fn text(status: u16, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain; charset=utf-8".to_string());
        Self {
            status,
            headers,
            body: body.to_string(),
        }
    }

    pub fn redirect(url: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Location".to_string(), url.to_string());
        Self {
            status: 302,
            headers,
            body: String::new(),
        }
    }

    pub fn not_found() -> Self {
        Self::html(404, "<h1>404 Not Found</h1>")
    }
}

// --- Plugin Info ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
}
