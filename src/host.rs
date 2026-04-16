use extism_pdk::*;

#[host_fn]
extern "ExtismHost" {
    pub fn cms_log(input: String) -> String;
    pub fn cms_get_setting(key: String) -> String;
    pub fn cms_set_setting(input: String) -> String;
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
