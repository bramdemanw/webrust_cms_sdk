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

    // --- BeforeRequest / AfterRequest helpers ---

    /// Request path when the event is `before_request` / `after_request`.
    pub fn request_path(&self) -> Option<&str> {
        self.get_str("path")
    }

    /// Request method when the event is `before_request` / `after_request`.
    pub fn request_method(&self) -> Option<&str> {
        self.get_str("method")
    }

    /// Lookup a cookie from the request context (case-sensitive, as cookies are).
    pub fn request_cookie(&self, name: &str) -> Option<String> {
        self.get("cookies")?
            .as_object()?
            .get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Lookup a request header (case-insensitive).
    pub fn request_header(&self, name: &str) -> Option<String> {
        let map = self.get("headers")?.as_object()?;
        map.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.as_str())
            .map(|s| s.to_string())
    }

    /// Halt the request and respond with the given status + body.
    /// Caller should wrap the context in `HookResult::Halt` to stop further hooks,
    /// but the host halts on `response_status` regardless of the result variant.
    pub fn set_response(&mut self, status: u16, body: impl Into<String>) {
        self.set("response_status", status as u64);
        self.set("response_body", body.into());
    }

    /// Halt the request with a 302 redirect to `url`.
    pub fn set_response_redirect(&mut self, url: impl Into<String>) {
        self.set("response_status", 302u64);
        self.set("response_body", "");
        let mut headers = serde_json::Map::new();
        headers.insert("Location".to_string(), serde_json::Value::String(url.into()));
        self.set("response_headers", serde_json::Value::Object(headers));
    }

    /// Attach an arbitrary response header when halting a request via `set_response`.
    pub fn set_response_header(&mut self, name: &str, value: impl Into<String>) {
        let mut headers = self
            .get("response_headers")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        headers.insert(name.to_string(), serde_json::Value::String(value.into()));
        self.set("response_headers", serde_json::Value::Object(headers));
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
    #[serde(default)]
    pub cookies: HashMap<String, String>,
    #[serde(default)]
    pub remote_ip: Option<String>,
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

    pub fn cookie(&self, key: &str) -> Option<&str> {
        self.cookies.get(key).map(|s| s.as_str())
    }

    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|s| s.as_str()).or_else(|| {
            // HTTP headers are case-insensitive
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str())
        })
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

    /// Render `body` inside the CMS admin shell (sidebar, topbar, styles).
    /// `title` goes in the page `<title>` + topbar, `active_key` should match
    /// the plugin's own `admin_nav` entry for sidebar highlighting
    /// (format `plugin:<slug>:<label-slug>`).
    pub fn admin_page(title: &str, active_key: &str, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
        headers.insert("X-Admin-Shell".to_string(), "1".to_string());
        headers.insert("X-Admin-Title".to_string(), title.to_string());
        headers.insert("X-Admin-Active-Key".to_string(), active_key.to_string());
        Self {
            status: 200,
            headers,
            body: body.to_string(),
        }
    }

    pub fn not_found() -> Self {
        Self::html(404, "<h1>404 Not Found</h1>")
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    /// Attach a Set-Cookie header. Builds a cookie string from the given options.
    /// Only one Set-Cookie per response is supported (HashMap limitation) — if you need
    /// to set multiple cookies, use a single call and concatenate or call again to replace.
    pub fn with_cookie(mut self, cookie: Cookie) -> Self {
        self.headers.insert("Set-Cookie".to_string(), cookie.serialize());
        self
    }
}

/// Minimal cookie builder for plugin responses.
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub path: Option<String>,
    pub max_age: Option<i64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
}

impl Cookie {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            path: Some("/".to_string()),
            max_age: None,
            http_only: true,
            secure: false,
            same_site: Some("Lax".to_string()),
        }
    }

    pub fn removed(name: &str) -> Self {
        let mut c = Self::new(name, "");
        c.max_age = Some(0);
        c
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }

    pub fn with_max_age(mut self, seconds: i64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    pub fn http_only(mut self, on: bool) -> Self {
        self.http_only = on;
        self
    }

    pub fn secure(mut self, on: bool) -> Self {
        self.secure = on;
        self
    }

    pub fn same_site(mut self, mode: &str) -> Self {
        self.same_site = Some(mode.to_string());
        self
    }

    pub fn serialize(&self) -> String {
        let mut s = format!("{}={}", self.name, self.value);
        if let Some(p) = &self.path {
            s.push_str(&format!("; Path={}", p));
        }
        if let Some(m) = self.max_age {
            s.push_str(&format!("; Max-Age={}", m));
        }
        if self.http_only {
            s.push_str("; HttpOnly");
        }
        if self.secure {
            s.push_str("; Secure");
        }
        if let Some(ss) = &self.same_site {
            s.push_str(&format!("; SameSite={}", ss));
        }
        s
    }
}

// --- Plugin Info ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
}
