use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Root of the Claude Code data directory, honouring `CLAUDE_CONFIG_DIR`.
pub fn claude_dir() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !custom.trim().is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot locate home directory"))?;
    Ok(home.join(".claude"))
}

/// `~/.claude/projects` — one subfolder per workspace, one .jsonl per session.
pub fn projects_dir() -> Result<PathBuf> {
    Ok(claude_dir()?.join("projects"))
}

/// `~/.claude/sessions` — one .json per *running* CLI process.
pub fn sessions_registry_dir() -> Result<PathBuf> {
    Ok(claude_dir()?.join("sessions"))
}

/// `~/.claude/session-env/<sessionId>` — removed alongside the transcript.
pub fn session_env_dir(session_id: &str) -> Result<PathBuf> {
    Ok(claude_dir()?.join("session-env").join(session_id))
}

/// Where soft-deleted sessions are parked so a delete stays undoable.
pub fn backups_dir() -> Result<PathBuf> {
    Ok(claude_dir()?.join("backups").join("vastdeck"))
}

/// `~/.claude.json` — per-workspace aggregates (cost, tokens, line counts).
pub fn claude_json() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot locate home directory"))?;
    Ok(home.join(".claude.json"))
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.to_path_buf())
}

/// True when a file named `portable.txt` sits next to the executable. The
/// portable download ships one; the installers do not.
pub fn is_portable() -> bool {
    exe_dir()
        .map(|dir| dir.join("portable.txt").is_file())
        .unwrap_or(false)
}

/// Vastdeck's own data directory, holding settings, the scan cache and launch
/// scripts.
///
/// In portable mode this is `data/` beside the executable, so the whole app is
/// one folder that can be carried around or deleted without leaving anything
/// behind. It falls back to `%LOCALAPPDATA%` when that folder cannot be written
/// to — a read-only share, a mounted image, Program Files — because failing to
/// start would be a far worse outcome than writing somewhere less tidy.
pub fn app_data_dir() -> Result<PathBuf> {
    if is_portable() {
        if let Some(dir) = exe_dir().map(|d| d.join("data")) {
            if std::fs::create_dir_all(&dir).is_ok() {
                return Ok(dir);
            }
        }
    }
    let base = dirs::data_local_dir().ok_or_else(|| anyhow!("cannot locate local app data"))?;
    let dir = base.join("vastdeck");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
