//! Codex CLI sessions.
//!
//! Codex records every thread in `$CODEX_HOME/state_<n>.sqlite` (`~/.codex`
//! when the variable is unset), with the conversation itself in a
//! zstd-compressed rollout file next to it. The list comes from the database
//! alone, read through a read-only connection: Codex — and the Codex desktop
//! app, which shares the folder — may be writing to it at the same moment.
//!
//! Every change goes through the `codex` CLI. Its own `archive` is the soft
//! delete: it hides the thread and moves the rollout aside, and `unarchive`
//! undoes exactly that, so Vastdeck never has to copy a rollout itself.

use crate::model::{LiveInfo, Session};
use anyhow::{anyhow, Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const PROVIDER_ID: &str = "codex";

/// Codex's data folder.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
}

/// The thread database. Its name carries a schema generation
/// (`state_5.sqlite`), so the highest one present is the current one.
pub fn db_path() -> Option<PathBuf> {
    let entries = std::fs::read_dir(home()?).ok()?;
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let generation: u32 = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse()
                .ok()?;
            Some((generation, e.path()))
        })
        .max_by_key(|(generation, _)| *generation)
        .map(|(_, path)| path)
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

/// Threads a person started, as Codex's own resume picker shows them: not
/// archived, with at least one user turn, and not a sub-agent spawned by
/// another thread (those belong to their parent's conversation).
const LISTED: &str = "archived = 0 AND has_user_event = 1
                      AND coalesce(thread_source, '') <> 'subagent'";

/// Listed threads, newest first.
pub fn scan() -> Result<Vec<Session>> {
    let Some(db) = db_path() else {
        return Ok(Vec::new());
    };
    let conn = open_read_only(&db)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, cwd, title, first_user_message, git_branch, cli_version,
                coalesce(created_at_ms, created_at * 1000),
                coalesce(updated_at_ms, updated_at * 1000),
                rollout_path
           FROM threads
          WHERE {LISTED}
          ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map([], |row| {
        let title: String = row.get(2)?;
        let first_prompt: String = row.get(3)?;
        let rollout: String = row.get(8)?;
        let title = one_line(&title);
        let first_prompt = one_line(&first_prompt);
        Ok(Session {
            id: row.get(0)?,
            provider: PROVIDER_ID.to_string(),
            path: rollout.clone(),
            workspace: native_path(&row.get::<_, String>(1)?),
            // A thread nobody renamed is titled with its first prompt, which
            // would then show twice.
            preview: Some(first_prompt.clone()).filter(|p| !p.is_empty() && *p != title),
            title: Some(title).filter(|t| !t.is_empty()),
            ai_title: None,
            git_branch: row.get::<_, Option<String>>(4)?.filter(|b| !b.is_empty()),
            cli_version: row.get::<_, Option<String>>(5)?.filter(|v| !v.is_empty()),
            permission_mode: None,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            size_bytes: rollout_size(&rollout),
            // Counting turns means decompressing every rollout.
            message_count: None,
            live: None,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// How many threads [`scan`] would list.
pub fn count() -> Result<usize> {
    let Some(db) = db_path() else {
        return Ok(0);
    };
    let conn = open_read_only(&db)?;
    let n: i64 = conn.query_row(
        &format!("SELECT count(*) FROM threads WHERE {LISTED}"),
        [],
        |row| row.get(0),
    )?;
    Ok(n as usize)
}

fn thread_exists(id: &str) -> Result<bool> {
    let Some(db) = db_path() else {
        return Ok(false);
    };
    let conn = open_read_only(&db)?;
    Ok(conn
        .query_row("SELECT 1 FROM threads WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

/// The database names the rollout `.jsonl`, but Codex compresses it in place
/// to `.jsonl.zst` once the thread goes quiet.
fn rollout_size(path: &str) -> u64 {
    std::fs::metadata(path)
        .or_else(|_| std::fs::metadata(format!("{path}.zst")))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Codex stores extended-length paths, `\\?\C:\work`; everything else here
/// says `C:\work`, and the two must agree for grouping and for `cd`.
fn native_path(dir: &str) -> String {
    dir.strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| dir.strip_prefix(r"\\?\").map(str::to_string))
        .unwrap_or_else(|| dir.to_string())
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(200).collect()
}

/// Codex writes no record of which process runs which thread, but a resumed
/// thread runs as `codex.exe resume <id>`, so the command line names it — and
/// that pid is the one Focus walks up from to find the terminal.
pub fn live_sessions(sessions: &[Session]) -> HashMap<String, LiveInfo> {
    let running = resumed_threads();
    if running.is_empty() {
        return HashMap::new();
    }
    sessions
        .iter()
        .filter_map(|s| {
            running.get(&s.id).map(|pid| {
                (
                    s.id.clone(),
                    LiveInfo {
                        pid: *pid,
                        // The command line does not say whether it is working.
                        status: "running".to_string(),
                        name: None,
                        started_at: None,
                    },
                )
            })
        })
        .collect()
}

/// Thread id to pid for every running `codex resume <id>`.
fn resumed_threads() -> HashMap<String, u32> {
    crate::winproc::processes_named("codex.exe")
        .into_iter()
        .filter_map(|(pid, line)| resumed_id(&line).map(|id| (id, pid)))
        .collect()
}

/// The thread id in a `codex … resume [flags] <id>` command line.
fn resumed_id(command_line: &str) -> Option<String> {
    let mut words = command_line.split_whitespace().map(|w| w.trim_matches('"'));
    words.find(|w| *w == "resume")?;
    words
        .filter(|w| !w.starts_with('-'))
        .find(|w| is_uuid(w))
        .map(str::to_string)
}

fn is_uuid(word: &str) -> bool {
    word.len() == 36
        && word.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

/// Running thread ids, for spotting a change cheaply.
pub fn live_fingerprint() -> Vec<String> {
    let mut ids: Vec<String> = resumed_threads().into_keys().collect();
    ids.sort();
    ids
}

/// Locates `codex` on PATH. The npm shim (`codex.cmd`) runs a Node wrapper
/// around a native binary; the binary is used directly so the CLI can run
/// without a console window flashing up, and without Node in between.
pub fn resolve() -> Result<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        if dir.join("codex.cmd").is_file() {
            if let Some(native) = native_behind_shim(&dir) {
                return Ok(native);
            }
        }
        for name in ["codex.exe", "codex.cmd", "codex.bat"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(anyhow!("could not find the codex executable on PATH"))
}

/// `node_modules/@openai/codex/node_modules/@openai/codex-win32-<arch>/vendor/<triple>/bin/codex.exe`
fn native_behind_shim(npm_dir: &Path) -> Option<PathBuf> {
    let scope = npm_dir
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("node_modules")
        .join("@openai");
    for platform in std::fs::read_dir(scope).ok()?.flatten() {
        if !platform
            .file_name()
            .to_string_lossy()
            .starts_with("codex-win32-")
        {
            continue;
        }
        let Ok(triples) = std::fs::read_dir(platform.path().join("vendor")) else {
            continue;
        };
        for triple in triples.flatten() {
            let exe = triple.path().join("bin").join("codex.exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn run_cli(args: &[&str]) -> Result<()> {
    let exe = resolve()?;
    // Codex starts a background `git ls-remote` for its plugins that outlives
    // the command and inherits its output handles. Reading a pipe would wait
    // for that git to finish too — minutes, offline — so stderr goes to a file
    // and only the codex process itself is waited for.
    let log = std::env::temp_dir().join(format!(
        "vastdeck-codex-{}-{}.log",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let stderr_file = std::fs::File::create(&log)?;
    let mut command = Command::new(exe);
    command
        .args(args)
        // `delete` asks for confirmation on a terminal; with no stdin it
        // refuses outright rather than waiting.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(stderr_file)
        // That git otherwise waits on a credential prompt nobody can see.
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let status = command.status();
    let stderr = std::fs::read(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    if !status?.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        // Codex warns about PATH helpers on every run; the error is the rest.
        let reason = stderr
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("WARNING:"))
            .collect::<Vec<_>>()
            .join(" ");
        return Err(anyhow!(
            "codex {} failed{}",
            args.first().copied().unwrap_or(""),
            if reason.is_empty() {
                String::new()
            } else {
                format!(": {}", reason.trim())
            }
        ));
    }
    Ok(())
}

pub fn archive(id: &str) -> Result<()> {
    run_cli(&["archive", id])
}

pub fn unarchive(id: &str) -> Result<()> {
    if !thread_exists(id)? {
        return Err(anyhow!("Codex no longer has this session"));
    }
    run_cli(&["unarchive", id])
}

/// Deletes the thread and its rollout for good. `--force` skips the prompt
/// Codex would otherwise refuse to run without; it takes only a UUID.
pub fn delete(id: &str) -> Result<()> {
    run_cli(&["delete", "--force", id])
}

/// Deletes a thread unless Codex has already lost it, in which case there is
/// nothing left to remove.
pub fn delete_if_present(id: &str) -> Result<()> {
    if thread_exists(id)? {
        delete(id)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_length_paths_become_plain_ones() {
        assert_eq!(
            native_path(r"\\?\C:\Local Disk (D)\work"),
            r"C:\Local Disk (D)\work"
        );
        assert_eq!(native_path(r"\\?\UNC\server\share\x"), r"\\server\share\x");
        assert_eq!(native_path(r"C:\work"), r"C:\work");
    }

    #[test]
    fn the_resumed_thread_is_read_from_the_command_line() {
        let id = "019dc7aa-3837-7d73-86ad-367dc86d2abd";
        assert_eq!(
            resumed_id(&format!(r"C:\x\codex.exe resume {id}")).as_deref(),
            Some(id)
        );
        assert_eq!(
            resumed_id(&format!(
                r#""C:\x\codex.exe" resume --sandbox read-only "{id}""#
            ))
            .as_deref(),
            Some(id)
        );
        assert_eq!(
            resumed_id(r"C:\x\codex.exe app-server --listen unix://"),
            None
        );
        assert_eq!(resumed_id(r"C:\x\codex.exe resume --last"), None);
    }

    #[test]
    #[ignore = "reads the real Codex database; run with --ignored"]
    fn scans_the_real_database_quickly() {
        let started = std::time::Instant::now();
        let sessions = scan().unwrap();
        let elapsed = started.elapsed();
        eprintln!("{} Codex threads in {elapsed:?}", sessions.len());
        for s in sessions.iter().take(3) {
            eprintln!("  {:?} | {} | {} bytes", s.title, s.workspace, s.size_bytes);
        }
        assert!(sessions.iter().all(|s| !s.workspace.starts_with(r"\\?\")));
        assert!(elapsed.as_millis() < 1500, "scan too slow: {elapsed:?}");
    }
}
