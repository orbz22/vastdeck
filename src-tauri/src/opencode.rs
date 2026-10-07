//! OpenCode sessions.
//!
//! OpenCode keeps every session in one SQLite database,
//! `$XDG_DATA_HOME/opencode/opencode.db` (`~/.local/share/opencode` when the
//! variable is unset — on Windows too). Vastdeck only ever *reads* it, through
//! a read-only connection: OpenCode may be writing to it at the same moment,
//! and a second writer is how databases get corrupted.
//!
//! Anything that changes a session — delete, restore — goes through the
//! `opencode` CLI instead, so OpenCode's own code owns every write.

use crate::model::{LiveInfo, Session};
use anyhow::{anyhow, Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const PROVIDER_ID: &str = "opencode";

/// Prefix OpenCode gives its terminal title: `OC | <session title>`.
const TITLE_PREFIX: &str = "oc | ";

/// OpenCode's data folder.
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))?;
    Some(base.join("opencode"))
}

pub fn db_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("opencode.db"))
}

fn open_read_only(db: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {}", db.display()))?;
    // OpenCode holds the write lock while it commits; wait that out rather
    // than failing the whole list over a busy database.
    conn.busy_timeout(Duration::from_secs(2))?;
    Ok(conn)
}

/// Top-level sessions, newest first. Sub-agent sessions (those with a parent)
/// are part of their parent's conversation and are not listed on their own;
/// archived ones are hidden, as OpenCode itself hides them.
pub fn scan() -> Result<Vec<Session>> {
    let Some(db) = db_path().filter(|p| p.is_file()) else {
        return Ok(Vec::new());
    };
    let conn = open_read_only(&db)?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.directory, s.title, s.version, s.time_created, s.time_updated,
                (SELECT count(*) FROM message m WHERE m.session_id = s.id),
                (SELECT json_extract(p.data, '$.text')
                   FROM message m JOIN part p ON p.message_id = m.id
                  WHERE m.session_id = s.id
                    AND json_extract(m.data, '$.role') = 'user'
                    AND json_extract(p.data, '$.type') = 'text'
                    AND coalesce(json_extract(p.data, '$.synthetic'), 0) = 0
                  ORDER BY m.time_created DESC, p.id DESC
                  LIMIT 1)
           FROM session s
          WHERE s.parent_id IS NULL AND s.time_archived IS NULL
          ORDER BY s.time_updated DESC",
    )?;
    let db_str = db.to_string_lossy().into_owned();
    let rows = stmt.query_map([], |row| {
        let directory: String = row.get(1)?;
        let title: String = row.get(2)?;
        let preview: Option<String> = row.get(7)?;
        Ok(Session {
            id: row.get(0)?,
            provider: PROVIDER_ID.to_string(),
            path: db_str.clone(),
            workspace: native_path(&directory),
            title: Some(title).filter(|t| !t.trim().is_empty()),
            ai_title: None,
            preview: preview.map(|p| one_line(&p)).filter(|p| !p.is_empty()),
            git_branch: None,
            cli_version: row.get(3)?,
            permission_mode: None,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            // Summing a session's rows means reading them all — seconds over
            // a database this size, on every refresh — so size is left out.
            size_bytes: 0,
            message_count: row.get::<_, Option<u32>>(6)?,
            live: None,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// How many sessions [`scan`] would list, without reading their messages.
pub fn count() -> Result<usize> {
    let Some(db) = db_path().filter(|p| p.is_file()) else {
        return Ok(0);
    };
    let conn = open_read_only(&db)?;
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM session WHERE parent_id IS NULL AND time_archived IS NULL",
        [],
        |row| row.get(0),
    )?;
    Ok(n as usize)
}

/// OpenCode stores `C:/Users/me`; everything else here says `C:\Users\me`, and
/// the two must agree for grouping by workspace to work.
fn native_path(dir: &str) -> String {
    if cfg!(windows) {
        dir.replace('/', "\\")
    } else {
        dir.to_string()
    }
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(200).collect()
}

fn session_exists(id: &str) -> Result<bool> {
    let Some(db) = db_path().filter(|p| p.is_file()) else {
        return Ok(false);
    };
    let conn = open_read_only(&db)?;
    Ok(conn
        .query_row("SELECT 1 FROM session WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

/// OpenCode records nothing about which process runs which session, but it
/// titles its terminal `OC | <session title>` (cut off with `…` when long).
/// A window carrying that title means the session is open — and the window's
/// owner is exactly the pid Focus needs to raise it.
pub fn live_sessions(sessions: &[Session]) -> HashMap<String, LiveInfo> {
    let windows: Vec<(u32, String)> = crate::winproc::titled_windows()
        .into_iter()
        .filter_map(|(pid, title)| {
            let lowered = title.to_lowercase();
            lowered
                .find(TITLE_PREFIX)
                .map(|at| (pid, lowered[at + TITLE_PREFIX.len()..].to_string()))
        })
        .collect();
    if windows.is_empty() {
        return HashMap::new();
    }

    let mut out = HashMap::new();
    for session in sessions {
        let Some(title) = session.title.as_deref() else {
            continue;
        };
        let hint = title.trim().to_lowercase();
        if hint.is_empty() {
            continue;
        }
        // A tab title is the session title and nothing after it, so a cut-off
        // title must match the start, and a whole one must match exactly —
        // "fix bug" must not light up "fix bug in parser" as well.
        let matched = windows
            .iter()
            .find(|(_, shown)| names_exactly(shown, &hint));
        if let Some((pid, _)) = matched {
            out.insert(
                session.id.clone(),
                LiveInfo {
                    pid: *pid,
                    // OpenCode's title does not say whether it is working.
                    status: "running".to_string(),
                    name: None,
                    started_at: None,
                },
            );
        }
    }
    out
}

/// Whether what follows `OC | ` in a window title is this session's title:
/// all of it, or — when OpenCode cut it off with `…` — its start.
fn names_exactly(shown: &str, title: &str) -> bool {
    let shown = shown.trim();
    if shown == title {
        return true;
    }
    // Too short a fragment would light up every session that starts alike.
    const MIN_FRAGMENT: usize = 8;
    shown
        .strip_suffix('…')
        .map(str::trim_end)
        .is_some_and(|cut| cut.chars().count() >= MIN_FRAGMENT && title.starts_with(cut))
}

/// Titles of every open OpenCode terminal, for spotting a change cheaply.
pub fn live_fingerprint() -> Vec<String> {
    let mut titles: Vec<String> = crate::winproc::titled_windows()
        .into_iter()
        .map(|(_, t)| t)
        .filter(|t| t.to_lowercase().contains(TITLE_PREFIX))
        .collect();
    titles.sort();
    titles
}

/// Locates `opencode` on PATH. The npm shim (`opencode.cmd`) is resolved to
/// the real binary behind it, so the CLI can be run without a console window
/// flashing up.
pub fn resolve() -> Result<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let behind_shim = dir
            .join("node_modules")
            .join("opencode-ai")
            .join("bin")
            .join("opencode.exe");
        if dir.join("opencode.cmd").is_file() && behind_shim.is_file() {
            return Ok(behind_shim);
        }
        for name in ["opencode.exe", "opencode.cmd", "opencode.bat"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".opencode").join("bin").join("opencode.exe");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(anyhow!("could not find the opencode executable on PATH"))
}

/// Runs the CLI without plugins (`--pure`) — they add seconds of startup and
/// nothing to export, import or delete.
fn run_cli(args: &[&str], cwd: Option<&Path>) -> Result<Vec<u8>> {
    let exe = resolve()?;
    let mut command = Command::new(exe);
    command.arg("--pure").args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output()?;
    if !output.status.success() {
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        let reason = stderr.trim();
        return Err(anyhow!(
            "opencode {} failed{}",
            args.first().copied().unwrap_or(""),
            if reason.is_empty() {
                String::new()
            } else {
                format!(": {reason}")
            }
        ));
    }
    Ok(output.stdout)
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip the CSI sequence up to its final letter.
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The session as OpenCode's own export JSON — everything `import` needs to
/// put it back.
pub fn export(id: &str) -> Result<Vec<u8>> {
    let json = run_cli(&["export", id], None)?;
    // Guard against a CLI that printed a banner or an error to stdout: a
    // backup that cannot be imported again is worse than no delete at all.
    let parsed: serde_json::Value =
        serde_json::from_slice(&json).context("opencode export did not return JSON")?;
    if parsed.pointer("/info/id").and_then(|v| v.as_str()) != Some(id) {
        return Err(anyhow!("opencode export returned a different session"));
    }
    Ok(json)
}

pub fn delete(id: &str) -> Result<()> {
    run_cli(&["session", "delete", id], None).map(|_| ())
}

/// Imports an export back. `import` files the session under the directory it
/// is run from rather than the one in the file, so it runs from the original
/// workspace. It also overwrites a session with the same id, so a session that
/// exists again is left alone instead.
pub fn import(id: &str, file: &Path, workspace: &str) -> Result<()> {
    if session_exists(id)? {
        return Err(anyhow!(
            "OpenCode already has this session; restoring would overwrite it"
        ));
    }
    let cwd = Path::new(workspace);
    if !cwd.is_dir() {
        return Err(anyhow!("workspace no longer exists: {workspace}"));
    }
    let file = file.to_string_lossy().into_owned();
    run_cli(&["import", &file], Some(cwd)).map(|_| ())
}

/// Title and workspace of an export file, for the Deleted view.
pub fn export_meta(file: &Path) -> Option<(Option<String>, Option<String>)> {
    let bytes = std::fs::read(file).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let title = v
        .pointer("/info/title")
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let workspace = v
        .pointer("/info/directory")
        .and_then(|t| t.as_str())
        .map(native_path);
    Some((title, workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_colour_codes_are_removed_from_cli_errors() {
        assert_eq!(
            strip_ansi("\u{1b}[91m\u{1b}[1mError: \u{1b}[0mSession not found: x"),
            "Error: Session not found: x"
        );
    }

    #[test]
    fn window_titles_name_a_session_whole_or_cut_off() {
        assert!(names_exactly("fix bug", "fix bug"));
        assert!(!names_exactly("fix bug", "fix bug in parser"));
        assert!(names_exactly(
            "local offline model setup for beyonda…",
            "local offline model setup for beyondatc at omnitower"
        ));
        assert!(!names_exactly("local…", "local offline model setup"));
    }

    #[test]
    fn previews_are_flattened_to_one_line() {
        assert_eq!(one_line("  fix\n\nthe   bug  "), "fix the bug");
    }

    #[test]
    #[ignore = "reads the real OpenCode database; run with --ignored"]
    fn scans_the_real_database_quickly() {
        let started = std::time::Instant::now();
        let sessions = scan().unwrap();
        let elapsed = started.elapsed();
        eprintln!("{} OpenCode sessions in {elapsed:?}", sessions.len());
        for s in sessions.iter().take(3) {
            eprintln!("  {:?} | {} | {:?}", s.title, s.workspace, s.message_count);
        }
        assert!(elapsed.as_millis() < 1500, "scan too slow: {elapsed:?}");
    }
}
