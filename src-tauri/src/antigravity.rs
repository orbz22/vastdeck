//! Antigravity CLI (`agy`) conversations.
//!
//! The CLI keeps its data in `~/.gemini/antigravity-cli`: one SQLite file per
//! conversation under `conversations/`, plus `conversation_summaries.db`, an
//! index holding each one's title, workspace and step count. The list comes
//! from that index, read through a read-only connection — `agy` writes it
//! while it runs. The Antigravity IDE indexes its own conversations into the
//! same table; only those with a file in the CLI's `conversations/` are the
//! CLI's.
//!
//! `agy` has no command to delete a conversation, so a delete moves the
//! conversation's own files — never the shared index — into Vastdeck's backup
//! folder, and a restore moves them back.

use crate::model::{LiveInfo, Session};
use anyhow::{anyhow, Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PROVIDER_ID: &str = "antigravity";

/// The CLI's data folder.
pub fn home() -> Option<PathBuf> {
    // Lets the round-trip test run against a copy, never the real folder.
    #[cfg(test)]
    if let Some(dir) = std::env::var_os("VASTDECK_ANTIGRAVITY_HOME") {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|h| h.join(".gemini").join("antigravity-cli"))
}

fn index_path() -> Option<PathBuf> {
    home()
        .map(|h| h.join("conversation_summaries.db"))
        .filter(|p| p.is_file())
}

fn conversation_file(home: &Path, id: &str) -> PathBuf {
    home.join("conversations").join(format!("{id}.db"))
}

/// Everything the CLI keeps for one conversation, relative to [`home`]: the
/// conversation itself, the agent's notes on it, and its small sidecars.
pub fn owned_paths(id: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from("conversations").join(format!("{id}.db")),
        PathBuf::from("brain").join(id),
        PathBuf::from("annotations").join(format!("{id}.pbtxt")),
        PathBuf::from("implicit").join(format!("{id}.pb")),
        PathBuf::from("presence").join(format!("{id}.lock")),
    ]
}

fn open_read_only(db: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {}", db.display()))?;
    conn.busy_timeout(Duration::from_secs(2))?;
    Ok(conn)
}

/// Conversations someone talked in, newest first. Sub-agent conversations
/// belong to their parent's; ones with no steps are empty windows.
pub fn scan() -> Result<Vec<Session>> {
    let (Some(home), Some(index)) = (home(), index_path()) else {
        return Ok(Vec::new());
    };
    let conn = open_read_only(&index)?;
    let mut stmt = conn.prepare(
        "SELECT conversation_id, title, preview, last_modified_time, workspace_uris
           FROM conversation_summaries
          WHERE nesting_depth = 0 AND parent_conversation_id = '' AND step_count > 0
          ORDER BY last_modified_time DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut out = Vec::new();
    for (id, title, preview, modified, uris) in rows.flatten() {
        let file = conversation_file(&home, &id);
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        let Some(workspace) = first_workspace(&uris) else {
            continue;
        };
        let title = one_line(&title);
        let preview = one_line(&preview);
        out.push(Session {
            id,
            provider: PROVIDER_ID.to_string(),
            path: file.to_string_lossy().into_owned(),
            workspace,
            // Untitled conversations carry their first prompt as the preview.
            title: Some(if title.is_empty() { preview.clone() } else { title.clone() })
                .filter(|t| !t.is_empty()),
            ai_title: None,
            preview: Some(preview).filter(|p| !p.is_empty() && *p != title),
            git_branch: None,
            cli_version: None,
            permission_mode: None,
            created_at: None,
            updated_at: parse_time(&modified).unwrap_or(0),
            size_bytes: meta.len(),
            message_count: None,
            live: None,
        });
    }
    Ok(out)
}

pub fn count() -> Result<usize> {
    scan().map(|s| s.len())
}

/// `["file:///C:/Local%20Disk%20%28D%29/work", …]` → `C:\Local Disk (D)\work`.
fn first_workspace(uris_json: &str) -> Option<String> {
    let uris: Vec<String> = serde_json::from_str(uris_json).ok()?;
    let uri = uris.into_iter().find(|u| u.starts_with("file:///"))?;
    let decoded = percent_decode(&uri["file:///".len()..]);
    let path = if cfg!(windows) {
        let mut p = decoded.replace('/', "\\");
        // The IDE writes `c:`; everything else here says `C:`.
        if p.as_bytes().get(1) == Some(&b':') {
            p[..1].make_ascii_uppercase();
        }
        p
    } else {
        format!("/{decoded}")
    };
    Some(path.trim_end_matches(['\\', '/']).to_string()).filter(|p| !p.is_empty())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `2026-10-22 14:28:53.0795655+00:00`, as the Go driver writes it.
fn parse_time(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f%:z")
        .ok()
        .map(|t| t.timestamp_millis())
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(200).collect()
}

/// `agy` records no process per conversation, but a resumed one runs as
/// `agy_real.exe --conversation <id>`, so the command line names it — and
/// that pid is where Focus starts looking for the terminal.
pub fn live_sessions(sessions: &[Session]) -> HashMap<String, LiveInfo> {
    let running = resumed_conversations();
    sessions
        .iter()
        .filter_map(|s| {
            running.get(&s.id).map(|pid| {
                (
                    s.id.clone(),
                    LiveInfo {
                        pid: *pid,
                        status: "running".to_string(),
                        name: None,
                        started_at: None,
                    },
                )
            })
        })
        .collect()
}

fn resumed_conversations() -> HashMap<String, u32> {
    crate::winproc::processes_named("agy_real.exe")
        .into_iter()
        .filter_map(|(pid, line)| resumed_id(&line).map(|id| (id, pid)))
        .collect()
}

/// The id after `--conversation` (or `--conversation=`) on a command line.
fn resumed_id(command_line: &str) -> Option<String> {
    let mut words = command_line.split_whitespace().map(|w| w.trim_matches('"'));
    while let Some(word) = words.next() {
        if let Some(id) = word.strip_prefix("--conversation=") {
            return Some(id.to_string()).filter(|id| !id.is_empty());
        }
        if word == "--conversation" {
            return words.next().map(str::to_string);
        }
    }
    None
}

pub fn live_fingerprint() -> Vec<String> {
    let mut ids: Vec<String> = resumed_conversations().into_keys().collect();
    ids.sort();
    ids
}

/// `agy` from PATH, or where its installer puts it.
pub fn resolve() -> Result<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        for name in ["agy.exe", "agy.cmd"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    dirs::data_local_dir()
        .map(|d| d.join("agy").join("bin").join("agy.exe"))
        .filter(|p| p.is_file())
        .ok_or_else(|| anyhow!("could not find the agy executable on PATH"))
}

/// Whether the conversation's own file is back in place, which a restore
/// must not overwrite.
pub fn conversation_exists(id: &str) -> bool {
    home().is_some_and(|h| conversation_file(&h, id).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_uris_become_windows_paths() {
        assert_eq!(
            first_workspace(r#"["file:///c%3A/Local%20Disk%20%28D%29/.Project/x"]"#).as_deref(),
            Some(r"C:\Local Disk (D)\.Project\x")
        );
        assert_eq!(
            first_workspace(r#"["file:///C:/Users/raiha"]"#).as_deref(),
            Some(r"C:\Users\raiha")
        );
        assert_eq!(first_workspace("[]"), None);
    }

    #[test]
    fn index_times_parse() {
        assert_eq!(
            parse_time("2026-10-22 14:28:53.0795655+00:00"),
            Some(1792679333079)
        );
    }

    #[test]
    fn the_resumed_conversation_is_read_from_the_command_line() {
        let id = "7d7a05c1-c668-4ffc-947c-7ec039e5df98";
        assert_eq!(
            resumed_id(&format!(r#""C:\agy\agy_real.exe" --conversation {id}"#)).as_deref(),
            Some(id)
        );
        assert_eq!(
            resumed_id(&format!(r"agy_real.exe --mode plan --conversation={id}")).as_deref(),
            Some(id)
        );
        assert_eq!(
            resumed_id(r"agy_real.exe --bg-updater --app_data_dir=antigravity-cli"),
            None
        );
    }

    #[test]
    #[ignore = "reads the real Antigravity index; run with --ignored"]
    fn scans_the_real_index_quickly() {
        let started = std::time::Instant::now();
        let sessions = scan().unwrap();
        let elapsed = started.elapsed();
        eprintln!("{} Antigravity conversations in {elapsed:?}", sessions.len());
        for s in sessions.iter().take(3) {
            eprintln!("  {:?} | {} | {}", s.title, s.workspace, s.updated_at);
        }
        assert!(elapsed.as_millis() < 1500, "scan too slow: {elapsed:?}");
    }
}
