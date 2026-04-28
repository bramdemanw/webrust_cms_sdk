pub mod host;
pub mod types;

pub use types::*;

pub mod prelude {
    pub use crate::host;
    pub use crate::include_html;
    pub use crate::types::*;
    pub use extism_pdk::{self, *};
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
}

/// Embed an HTML file at compile time with optional `[[key]]` placeholder substitution.
///
/// Placeholders use `[[key]]` syntax, which is safe alongside CSS `{ }` and
/// email template `{{variable}}` — no escaping needed anywhere.
///
/// Paths are relative to the calling source file (same as `include_str!`).
///
/// # Examples
/// ```rust
/// // Just embed a static file
/// let html = include_html!("templates/help.html");
///
/// // Embed with substitution
/// let html = include_html!("templates/page.html",
///     "title"   => page_title,
///     "rows"    => table_rows_html,
///     "css"     => form_css(),
/// );
/// ```
#[macro_export]
macro_rules! include_html {
    ($path:expr $(,)?) => {
        include_str!($path).to_string()
    };
    ($path:expr, $($key:literal => $val:expr),+ $(,)?) => {{
        let mut _s = include_str!($path).to_string();
        $(
            _s = _s.replace(concat!("[[", $key, "]]"), &($val).to_string());
        )*
        _s
    }};
}
