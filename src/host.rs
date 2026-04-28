use extism_pdk::*;

#[host_fn]
extern "ExtismHost" {
    pub fn cms_log(input: String) -> String;
    pub fn cms_get_setting(key: String) -> String;
    pub fn cms_set_setting(input: String) -> String;
    pub fn cms_hash_password(password: String) -> String;
    pub fn cms_verify_password(input: String) -> String;
    pub fn cms_random_bytes(length: String) -> String;
    pub fn cms_random_token(length: String) -> String;
    pub fn cms_db_query(input: String) -> String;
    pub fn cms_db_execute(input: String) -> String;
    pub fn cms_http_request(input: String) -> String;
    pub fn cms_send_email(input: String) -> String;
}

/// Log a message to the CMS log system.
/// Level must be one of: "trace", "debug", "info", "warn", "error"
pub fn log(level: &str, message: &str) {
    let input = serde_json::json!({
        "level": level,
        "message": message,
    });
    let _ = unsafe { cms_log(input.to_string()) };
}

pub fn log_info(message: &str) {
    log("info", message);
}

pub fn log_warn(message: &str) {
    log("warn", message);
}

pub fn log_error(message: &str) {
    log("error", message);
}

/// Get a plugin-specific setting value from the CMS database.
/// Returns None if the key doesn't exist.
pub fn get_setting(key: &str) -> Option<String> {
    let result = unsafe { cms_get_setting(key.to_string()) }.ok()?;
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Save a plugin-specific setting to the CMS database.
pub fn set_setting(key: &str, value: &str) -> bool {
    let input = serde_json::json!({
        "key": key,
        "value": value,
    });
    unsafe { cms_set_setting(input.to_string()) }.is_ok()
}

/// Hash a plaintext password using Argon2id. Returns a PHC-formatted hash string.
pub fn hash_password(password: &str) -> Option<String> {
    unsafe { cms_hash_password(password.to_string()) }.ok()
}

/// Verify a password against an Argon2 PHC hash. Returns true only on a valid match.
pub fn verify_password(hash: &str, password: &str) -> bool {
    let input = serde_json::json!({
        "hash": hash,
        "password": password,
    });
    unsafe { cms_verify_password(input.to_string()) }
        .map(|s| s == "true")
        .unwrap_or(false)
}

/// Generate `n` cryptographically-secure random bytes as a lowercase hex string.
pub fn random_bytes_hex(n: usize) -> Option<String> {
    unsafe { cms_random_bytes(n.to_string()) }.ok()
}

/// Generate a random alphanumeric token of length `n`. Suitable for session IDs.
pub fn random_token(n: usize) -> Option<String> {
    unsafe { cms_random_token(n.to_string()) }.ok()
}

// --- Database access ---
//
// Plugins get full read/write access to the CMS database. By convention,
// plugin-owned tables should be named `plugin_{slug}_*` (e.g. `plugin_auth_users`)
// so core + other plugins can recognize them and so future versions can tighten
// sandboxing without breaking well-behaved plugins. This convention is NOT
// enforced by the host — you can read/write any table.

/// A single row returned from `db_query`, as a JSON object of column → value.
pub type DbRow = serde_json::Map<String, serde_json::Value>;

#[derive(Debug)]
pub struct DbError(pub String);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "db error: {}", self.0)
    }
}

impl std::error::Error for DbError {}

/// Result summary from an INSERT/UPDATE/DELETE statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct DbExecuteResult {
    pub affected_rows: u64,
    pub last_insert_id: u64,
}

/// Run a SELECT query against the CMS database and return the rows.
/// Use `?` placeholders and pass bind values via `params`.
///
/// Example:
/// ```ignore
/// let rows = host::db_query(
///     "SELECT id, username FROM plugin_auth_users WHERE active = ?",
///     &[serde_json::json!(true)],
/// )?;
/// ```
pub fn db_query(sql: &str, params: &[serde_json::Value]) -> Result<Vec<DbRow>, DbError> {
    let input = serde_json::json!({
        "sql": sql,
        "params": params,
    });
    let raw = unsafe { cms_db_query(input.to_string()) }
        .map_err(|e| DbError(format!("host call failed: {}", e)))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| DbError(format!("invalid response: {}", e)))?;
    if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
        return Err(DbError(err.to_string()));
    }
    let rows = parsed
        .get("rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter_map(|v| v.as_object().cloned())
        .collect())
}

/// Run an INSERT/UPDATE/DELETE or DDL statement. Returns `{affected_rows, last_insert_id}`.
pub fn db_execute(sql: &str, params: &[serde_json::Value]) -> Result<DbExecuteResult, DbError> {
    let input = serde_json::json!({
        "sql": sql,
        "params": params,
    });
    let raw = unsafe { cms_db_execute(input.to_string()) }
        .map_err(|e| DbError(format!("host call failed: {}", e)))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| DbError(format!("invalid response: {}", e)))?;
    if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
        return Err(DbError(err.to_string()));
    }
    Ok(DbExecuteResult {
        affected_rows: parsed
            .get("affected_rows")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        last_insert_id: parsed
            .get("last_insert_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

/// Convenience: fetch at most one row matching `sql`.
pub fn db_query_one(sql: &str, params: &[serde_json::Value]) -> Result<Option<DbRow>, DbError> {
    Ok(db_query(sql, params)?.into_iter().next())
}

// --- HTTP ---

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

#[derive(Debug)]
pub struct HttpError(pub String);

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "http error: {}", self.0)
    }
}

impl std::error::Error for HttpError {}

/// Make an outbound HTTP request from a plugin.
///
/// `headers` is a slice of `("Header-Name", "value")` pairs.
/// `body` is an optional request body string.
///
/// Example:
/// ```ignore
/// let resp = host::http_request(
///     "POST",
///     "https://api.example.com/data",
///     &[("Authorization", "Bearer token"), ("Content-Type", "application/json")],
///     Some(r#"{"key":"value"}"#),
/// )?;
/// println!("{}", resp.body);
/// ```
pub fn http_request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Result<HttpResponse, HttpError> {
    let headers_obj: std::collections::HashMap<&str, &str> = headers.iter().cloned().collect();
    let input = serde_json::json!({
        "method": method,
        "url": url,
        "headers": headers_obj,
        "body": body,
    });
    let raw = unsafe { cms_http_request(input.to_string()) }
        .map_err(|e| HttpError(format!("host call failed: {}", e)))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| HttpError(format!("invalid response: {}", e)))?;
    if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
        return Err(HttpError(err.to_string()));
    }
    Ok(HttpResponse {
        status: parsed.get("status").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        headers: parsed
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default(),
        body: parsed
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Convenience: GET request with no body.
pub fn http_get(url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, HttpError> {
    http_request("GET", url, headers, None)
}

/// Convenience: POST request with a string body.
pub fn http_post(url: &str, headers: &[(&str, &str)], body: &str) -> Result<HttpResponse, HttpError> {
    http_request("POST", url, headers, Some(body))
}

// --- Email ---

/// Send an email via SMTP. Returns Ok(()) on success or Err(message) on failure.
/// `from` and `to` accept "Name <email@domain.com>" or plain "email@domain.com" format.
pub fn send_email(
    smtp_host: &str,
    smtp_port: u16,
    smtp_user: &str,
    smtp_password: &str,
    from: &str,
    to: &str,
    subject: &str,
    html: &str,
    text: Option<&str>,
) -> Result<(), String> {
    let input = serde_json::json!({
        "smtp_host": smtp_host,
        "smtp_port": smtp_port,
        "smtp_user": smtp_user,
        "smtp_password": smtp_password,
        "from": from,
        "to": to,
        "subject": subject,
        "html": html,
        "text": text,
    });
    let raw = unsafe { cms_send_email(input.to_string()) }.map_err(|e| e.to_string())?;
    if raw.is_empty() { Ok(()) } else { Err(raw) }
}
