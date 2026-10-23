// scmCode: the workbench owns the user `settings.json` and reads and writes it through
// its configuration service, as VS Code does (`ConfigurationEditing`). SideX kept a
// second copy here (`SettingsStore`, `settings_get` / `settings_update` /
// `settings_load`) that was loaded once at startup, from another directory on Windows
// and macOS, and rewrote the whole file from that stale copy on every update: hand
// edits were ignored or lost, and comments dropped (docs/audit TR-08). It is gone.
// The backend reads the workbench's file when it needs a value (`server.rs`,
// `ai_enabled_in`).

use serde_json::Value;
use sidex_settings::{modify_jsonc, parse_jsonc};

/// Parse a JSONC string (strips comments & trailing commas) and return the
/// resulting JSON value. Useful for the frontend to preview or validate
/// settings files.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_parse_jsonc(input: String) -> Result<Value, String> {
    parse_jsonc(&input).map_err(|e| e.to_string())
}

/// Edit a value inside a JSONC document by key-path, preserving surrounding
/// comments and formatting. Returns the modified JSONC string.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_modify_jsonc(
    input: String,
    path: Vec<String>,
    value: Value,
) -> Result<String, String> {
    let refs: Vec<&str> = path.iter().map(String::as_str).collect();
    modify_jsonc(&input, &refs, &value).map_err(|e| e.to_string())
}
