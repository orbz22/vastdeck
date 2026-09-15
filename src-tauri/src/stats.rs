//! Per-workspace aggregates from `~/.claude.json`.
//!
//! Keys in that file are absolute paths written with forward slashes, so they
//! need normalising before they will match a session's `cwd`.

use crate::model::WorkspaceStats;
use crate::paths;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Lowercased, forward-slashed, trailing separator removed — enough to match
/// two spellings of the same Windows path.
pub fn normalize(path: &str) -> String {
    let s = path.replace('\\', "/").to_lowercase();
    s.trim_end_matches('/').to_string()
}

pub fn workspace_stats() -> Result<HashMap<String, WorkspaceStats>> {
    let mut out = HashMap::new();
    let Ok(bytes) = std::fs::read(paths::claude_json()?) else {
        return Ok(out);
    };
    let Ok(root) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(out);
    };
    let Some(projects) = root.get("projects").and_then(Value::as_object) else {
        return Ok(out);
    };

    for (path, entry) in projects {
        out.insert(
            normalize(path),
            WorkspaceStats {
                cost_usd: entry.get("lastCost").and_then(Value::as_f64),
                lines_added: entry.get("lastLinesAdded").and_then(Value::as_u64),
                lines_removed: entry.get("lastLinesRemoved").and_then(Value::as_u64),
                last_session_id: entry
                    .get("lastSessionId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
        );
    }
    Ok(out)
}
