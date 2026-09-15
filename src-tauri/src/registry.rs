//! Live session registry — `~/.claude/sessions/<pid>.json`.
//!
//! The CLI writes one file per running process holding its sessionId, cwd,
//! display name and a `status` of "idle" or "busy". Files are not always cleaned
//! up on a crash, so every entry is validated against the live process list.

use crate::model::LiveInfo;
use crate::paths;
use crate::winproc;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    pid: u32,
    session_id: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    started_at: Option<i64>,
    /// Process creation FILETIME, stored as a decimal string.
    #[serde(default)]
    proc_start: Option<String>,
}

/// Running sessions, keyed by sessionId.
pub fn live_sessions() -> Result<HashMap<String, LiveInfo>> {
    let dir = paths::sessions_registry_dir()?;
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(reg) = serde_json::from_slice::<RegistryFile>(&bytes) else {
            continue;
        };

        let expected_start = reg.proc_start.as_deref().and_then(|s| s.parse::<u64>().ok());
        if !winproc::is_alive(reg.pid, expected_start) {
            continue;
        }

        out.insert(
            reg.session_id.clone(),
            LiveInfo {
                pid: reg.pid,
                status: reg.status.unwrap_or_else(|| "idle".to_string()),
                name: reg.name,
                started_at: reg.started_at,
            },
        );
    }
    Ok(out)
}
