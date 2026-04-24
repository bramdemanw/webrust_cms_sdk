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
