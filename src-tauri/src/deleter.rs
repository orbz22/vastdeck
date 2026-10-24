//! Deleting sessions.
//!
//! A transcript is the only record of a conversation, and there is ~400 MB of
//! them here, so the default is a move into `~/.claude/backups/vastdeck/` that
//! can be undone. Hard delete stays available but is opt-in.
//!
//! Each backup folder carries a `vastdeck.json` manifest naming where the
//! transcript came from. Without it a backup is a dead end: the folder it
//! belongs to is `~/.claude/projects/<encoded-cwd>/`, and that encoding is
//! lossy, so the path cannot be reconstructed from the transcript alone.
//!
//! OpenCode sessions live in a database rather than a file, so there is
//! nothing to move: a soft delete saves `opencode export` output to the same
//! backup folder before `opencode session delete`, and a restore imports it.
//!
//! Codex has an archive of its own, which hides a thread and can bring it back
//! untouched. A soft delete archives the thread and leaves a small record in
//! the backup folder so it shows in the Deleted view; a restore unarchives it,
//! and purging the record deletes the thread from Codex for good.
//!
//! Antigravity's `agy` has no delete of its own, so its conversation files —
//! never the index it shares with the IDE — move into the backup folder under
//! `antigravity/`, laid out as they were, and a restore moves them back.

use crate::antigravity;
use crate::codex;
use crate::model::{DeletedHandle, DeletedSession, Session};
use crate::opencode;
use crate::paths;
use crate::scanner;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MANIFEST: &str = "vastdeck.json";
/// The `opencode export` output inside an OpenCode session's backup.
const OPENCODE_EXPORT: &str = "opencode-session.json";
/// What a Codex session's backup holds in place of a transcript: the thread
/// stays in Codex's own archive, and this names it for the Deleted view.
const CODEX_RECORD: &str = "codex-thread.json";
/// Names an Antigravity conversation for the Deleted view; its files sit
/// beside it under [`ANTIGRAVITY_FILES`].
const ANTIGRAVITY_RECORD: &str = "antigravity-conversation.json";
const ANTIGRAVITY_FILES: &str = "antigravity";

/// A session the backup holds by name rather than as a transcript.
#[derive(Debug, Serialize, Deserialize)]
struct SessionRecord {
    id: String,
    title: Option<String>,
    workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    session_id: String,
    original_path: String,
    #[serde(default)]
    original_env_dir: Option<String>,
    #[serde(default)]
    workspace: String,
    deleted_at: i64,
    /// Absent in backups made before other CLIs were supported, which are all
    /// Claude Code ones.
    #[serde(default)]
    provider: Option<String>,
}

impl Manifest {
    fn is_opencode(&self) -> bool {
        self.provider.as_deref() == Some(opencode::PROVIDER_ID)
    }

    fn is_codex(&self) -> bool {
        self.provider.as_deref() == Some(codex::PROVIDER_ID)
    }

    fn is_antigravity(&self) -> bool {
        self.provider.as_deref() == Some(antigravity::PROVIDER_ID)
    }
}

fn new_backup_dir(session: &Session) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let short = session
        .id
        .strip_prefix("ses_")
        .unwrap_or(&session.id)
        .chars()
        .take(8)
        .collect::<String>();
    let backup = paths::backups_dir()?.join(format!("{stamp}-{short}"));
    std::fs::create_dir_all(&backup)?;
    Ok(backup)
}

fn soft_delete_opencode(session: &Session) -> Result<DeletedHandle> {
    // Export first and delete only once the backup is on disk: a failed
    // export must leave the session exactly where it was.
    let export = opencode::export(&session.id)?;
    let backup = new_backup_dir(session)?;
    std::fs::write(backup.join(OPENCODE_EXPORT), export)?;
    let manifest = Manifest {
        session_id: session.id.clone(),
        original_path: session.path.clone(),
        original_env_dir: None,
        workspace: session.workspace.clone(),
        deleted_at: chrono::Utc::now().timestamp_millis(),
        provider: Some(opencode::PROVIDER_ID.to_string()),
    };
    std::fs::write(backup.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;

    if let Err(err) = opencode::delete(&session.id) {
        // Nothing was deleted, so a backup would only list it twice.
        let _ = std::fs::remove_dir_all(&backup);
        return Err(err);
    }
    Ok(DeletedHandle {
        session_id: session.id.clone(),
        backup_dir: Some(backup.to_string_lossy().into_owned()),
        original_path: session.path.clone(),
        original_env_dir: None,
    })
}

fn soft_delete_codex(session: &Session) -> Result<DeletedHandle> {
    let backup = new_backup_dir(session)?;
    let record = SessionRecord {
        id: session.id.clone(),
        title: session.title.clone(),
        workspace: session.workspace.clone(),
    };
    std::fs::write(backup.join(CODEX_RECORD), serde_json::to_vec_pretty(&record)?)?;
    let manifest = Manifest {
        session_id: session.id.clone(),
        original_path: session.path.clone(),
        original_env_dir: None,
        workspace: session.workspace.clone(),
        deleted_at: chrono::Utc::now().timestamp_millis(),
        provider: Some(codex::PROVIDER_ID.to_string()),
    };
    std::fs::write(backup.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;

    if let Err(err) = codex::archive(&session.id) {
        let _ = std::fs::remove_dir_all(&backup);
        return Err(err);
    }
    Ok(DeletedHandle {
        session_id: session.id.clone(),
        backup_dir: Some(backup.to_string_lossy().into_owned()),
        original_path: session.path.clone(),
        original_env_dir: None,
    })
}

fn soft_delete_antigravity(session: &Session) -> Result<DeletedHandle> {
    let home = antigravity::home().ok_or_else(|| anyhow!("no home folder"))?;
    let backup = new_backup_dir(session)?;
    let record = SessionRecord {
        id: session.id.clone(),
        title: session.title.clone(),
        workspace: session.workspace.clone(),
    };
    std::fs::write(backup.join(ANTIGRAVITY_RECORD), serde_json::to_vec_pretty(&record)?)?;
    let manifest = Manifest {
        session_id: session.id.clone(),
        original_path: session.path.clone(),
        original_env_dir: None,
        workspace: session.workspace.clone(),
        deleted_at: chrono::Utc::now().timestamp_millis(),
        provider: Some(antigravity::PROVIDER_ID.to_string()),
    };
    std::fs::write(backup.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;

    let stored = backup.join(ANTIGRAVITY_FILES);
    for (i, rel) in antigravity::owned_paths(&session.id).iter().enumerate() {
        let from = home.join(rel);
        if !from.exists() {
            continue;
        }
        let to = stored.join(rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match move_path(&from, &to) {
            Ok(()) => {}
            // The conversation itself comes first: if it will not move — open
            // in a running agy, say — nothing has been touched yet.
            Err(err) if i == 0 => {
                let _ = std::fs::remove_dir_all(&backup);
                return Err(err);
            }
            // A sidecar left behind is harmless; the conversation is gone.
            Err(_) => {}
        }
    }
    Ok(DeletedHandle {
        session_id: session.id.clone(),
        backup_dir: Some(backup.to_string_lossy().into_owned()),
        original_path: session.path.clone(),
        original_env_dir: None,
    })
}

pub fn soft_delete(session: &Session) -> Result<DeletedHandle> {
    if session.provider == opencode::PROVIDER_ID {
        return soft_delete_opencode(session);
    }
    if session.provider == codex::PROVIDER_ID {
        return soft_delete_codex(session);
    }
    if session.provider == antigravity::PROVIDER_ID {
        return soft_delete_antigravity(session);
    }
    let transcript = PathBuf::from(&session.path);
    if !transcript.is_file() {
        return Err(anyhow!("transcript is already gone: {}", session.path));
    }

    let backup = new_backup_dir(session)?;

    // The env folder is worthless without the transcript, so it travels with it.
    let env_dir = paths::session_env_dir(&session.id)?;
    let moved_env = if env_dir.is_dir() {
        move_path(&env_dir, &backup.join("session-env"))
            .ok()
            .map(|_| env_dir.to_string_lossy().into_owned())
    } else {
        None
    };

    // Written before the move, so a crash mid-move still leaves a readable trail.
    let manifest = Manifest {
        session_id: session.id.clone(),
        original_path: session.path.clone(),
        original_env_dir: moved_env.clone(),
        workspace: session.workspace.clone(),
        deleted_at: chrono::Utc::now().timestamp_millis(),
        provider: None,
    };
    std::fs::write(backup.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;

    let dest = backup.join(
        transcript
            .file_name()
            .ok_or_else(|| anyhow!("transcript has no file name"))?,
    );
    move_path(&transcript, &dest)?;

    Ok(DeletedHandle {
        session_id: session.id.clone(),
        backup_dir: Some(backup.to_string_lossy().into_owned()),
        original_path: session.path.clone(),
        original_env_dir: moved_env,
    })
}

pub fn hard_delete(session: &Session) -> Result<DeletedHandle> {
    if session.provider == antigravity::PROVIDER_ID {
        let home = antigravity::home().ok_or_else(|| anyhow!("no home folder"))?;
        for (i, rel) in antigravity::owned_paths(&session.id).iter().enumerate() {
            let path = home.join(rel);
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else if path.exists() {
                std::fs::remove_file(&path)
            } else {
                Ok(())
            };
            if i == 0 {
                removed?;
            }
        }
        return Ok(DeletedHandle {
            session_id: session.id.clone(),
            backup_dir: None,
            original_path: session.path.clone(),
            original_env_dir: None,
        });
    }
    if session.provider == opencode::PROVIDER_ID || session.provider == codex::PROVIDER_ID {
        if session.provider == codex::PROVIDER_ID {
            codex::delete(&session.id)?;
        } else {
            opencode::delete(&session.id)?;
        }
        return Ok(DeletedHandle {
            session_id: session.id.clone(),
            backup_dir: None,
            original_path: session.path.clone(),
            original_env_dir: None,
        });
    }
    let transcript = PathBuf::from(&session.path);
    if transcript.is_file() {
        std::fs::remove_file(&transcript)?;
    }
    let env_dir = paths::session_env_dir(&session.id)?;
    if env_dir.is_dir() {
        let _ = std::fs::remove_dir_all(&env_dir);
    }
    Ok(DeletedHandle {
        session_id: session.id.clone(),
        backup_dir: None,
        original_path: session.path.clone(),
        original_env_dir: None,
    })
}

/// Undo of a delete that just happened. The handle only carries the backup
/// path; everything else comes from the manifest, so this and the Deleted view
/// take the same route back.
pub fn undo(handle: &DeletedHandle) -> Result<()> {
    let backup = handle
        .backup_dir
        .as_ref()
        .ok_or_else(|| anyhow!("this delete was permanent and cannot be undone"))?;
    restore(backup)
}

/// Everything currently sitting in the backup folder, newest first.
pub fn list_deleted() -> Result<Vec<DeletedSession>> {
    let root = paths::backups_dir()?;
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(out);
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = read_manifest(&dir);
        let Some(transcript) = transcript_in(&dir) else {
            continue;
        };
        let size = std::fs::metadata(&transcript).map(|m| m.len()).unwrap_or(0);
        let by_record = manifest
            .as_ref()
            .is_some_and(|m| m.is_codex() || m.is_antigravity());
        let meta = if by_record {
            session_record(&transcript).map(|record| scanner::QuickMeta {
                workspace: Some(record.workspace),
                title: record.title,
                preview: None,
            })
        } else if manifest.as_ref().is_some_and(Manifest::is_opencode) {
            opencode::export_meta(&transcript).map(|(title, workspace)| scanner::QuickMeta {
                workspace,
                title,
                preview: None,
            })
        } else {
            scanner::quick_meta(&transcript).ok()
        };

        let deleted_at = manifest
            .as_ref()
            .map(|m| m.deleted_at)
            .or_else(|| {
                std::fs::metadata(&dir)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
            })
            .unwrap_or(0);

        out.push(DeletedSession {
            backup_dir: dir.to_string_lossy().into_owned(),
            session_id: manifest
                .as_ref()
                .map(|m| m.session_id.clone())
                .or_else(|| {
                    transcript
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                })
                .unwrap_or_default(),
            workspace: manifest
                .as_ref()
                .map(|m| m.workspace.clone())
                .filter(|w| !w.is_empty())
                .or_else(|| meta.as_ref().and_then(|m| m.workspace.clone()))
                .unwrap_or_default(),
            title: meta.as_ref().and_then(|m| m.title.clone()),
            preview: meta.as_ref().and_then(|m| m.preview.clone()),
            deleted_at,
            size_bytes: size,
            restorable: manifest.is_some(),
        });
    }

    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}

/// Puts a backed-up transcript back where it came from.
pub fn restore(backup_dir: &str) -> Result<()> {
    let backup = PathBuf::from(backup_dir);
    let manifest = read_manifest(&backup).ok_or_else(|| {
        anyhow!("this backup has no manifest, so its original location is unknown")
    })?;

    if manifest.is_antigravity() {
        let home = antigravity::home().ok_or_else(|| anyhow!("no home folder"))?;
        if antigravity::conversation_exists(&manifest.session_id) {
            return Err(anyhow!(
                "Antigravity already has this conversation; restoring would overwrite it"
            ));
        }
        let stored = backup.join(ANTIGRAVITY_FILES);
        for rel in antigravity::owned_paths(&manifest.session_id) {
            let from = stored.join(&rel);
            if !from.exists() {
                continue;
            }
            let to = home.join(&rel);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            move_path(&from, &to)?;
        }
        let _ = std::fs::remove_dir_all(&backup);
        return Ok(());
    }

    if manifest.is_codex() {
        codex::unarchive(&manifest.session_id)?;
        let _ = std::fs::remove_dir_all(&backup);
        return Ok(());
    }

    if manifest.is_opencode() {
        let export = backup.join(OPENCODE_EXPORT);
        if !export.is_file() {
            return Err(anyhow!("backup no longer holds the OpenCode export"));
        }
        opencode::import(&manifest.session_id, &export, &manifest.workspace)?;
        let _ = std::fs::remove_dir_all(&backup);
        return Ok(());
    }

    let original = PathBuf::from(&manifest.original_path);
    if original.exists() {
        return Err(anyhow!(
            "a transcript already exists at {}; restoring would overwrite it",
            manifest.original_path
        ));
    }
    let stored = transcript_in(&backup)
        .ok_or_else(|| anyhow!("backup no longer holds a transcript"))?;

    if let Some(parent) = original.parent() {
        std::fs::create_dir_all(parent)?;
    }
    move_path(&stored, &original)?;

    if let Some(env_dir) = manifest.original_env_dir.as_ref() {
        let stored_env = backup.join("session-env");
        if stored_env.is_dir() {
            let _ = move_path(&stored_env, Path::new(env_dir));
        }
    }

    let _ = std::fs::remove_file(backup.join(MANIFEST));
    let _ = std::fs::remove_dir(&backup);
    Ok(())
}

/// Removes a backup for good.
pub fn purge(backup_dir: &str) -> Result<()> {
    let backup = PathBuf::from(backup_dir);
    let root = paths::backups_dir()?;
    // Never delete outside the folder this feature owns, whatever gets passed in.
    if !backup.starts_with(&root) {
        return Err(anyhow!("refusing to delete outside the backup folder"));
    }
    // A Codex backup is only a record; the thread itself waits in Codex's
    // archive, and deleting the record for good has to delete that too.
    if let Some(manifest) = read_manifest(&backup).filter(Manifest::is_codex) {
        codex::delete_if_present(&manifest.session_id)?;
    }
    if backup.is_dir() {
        std::fs::remove_dir_all(&backup)?;
    }
    Ok(())
}

pub fn purge_all() -> Result<usize> {
    let mut removed = 0;
    for deleted in list_deleted()? {
        if purge(&deleted.backup_dir).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let bytes = std::fs::read(dir.join(MANIFEST)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The conversation a backup holds: a Claude transcript, an OpenCode export,
/// or the record of an archived Codex thread or a moved Antigravity one.
fn transcript_in(dir: &Path) -> Option<PathBuf> {
    for held in [OPENCODE_EXPORT, CODEX_RECORD, ANTIGRAVITY_RECORD] {
        let file = dir.join(held);
        if file.is_file() {
            return Some(file);
        }
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.extension().and_then(|x| x.to_str()) == Some("jsonl")).then_some(p)
    })
}

fn session_record(file: &Path) -> Option<SessionRecord> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

/// `rename` first; fall back to copy-then-remove when the two ends sit on
/// different volumes.
fn move_path(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        std::fs::remove_dir_all(from)?;
    } else {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Session;

    fn fixture(dir: &Path, id: &str) -> Session {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(
            &path,
            "{\"type\":\"user\",\"cwd\":\"C:/tmp\",\"timestamp\":\"2026-01-01T00:00:00.000Z\"}\n",
        )
        .unwrap();
        Session {
            id: id.to_string(),
            provider: "claude-code".into(),
            path: path.to_string_lossy().into_owned(),
            workspace: r"C:\tmp".into(),
            title: None,
            ai_title: None,
            preview: None,
            git_branch: None,
            cli_version: None,
            permission_mode: None,
            created_at: None,
            updated_at: 0,
            size_bytes: 0,
            message_count: None,
            live: None,
        }
    }

    /// The destructive path, end to end: a soft delete must leave the transcript
    /// recoverable, and undo must put it back exactly where it was.
    #[test]
    fn soft_delete_round_trips() {
        let dir = std::env::temp_dir().join("vastdeck-del-test");
        let _ = std::fs::remove_dir_all(&dir);
        let session = fixture(&dir, "11111111-2222-3333-4444-555555555555");
        let original = PathBuf::from(&session.path);
        let before = std::fs::read(&original).unwrap();

        let handle = soft_delete(&session).expect("soft delete failed");
        assert!(!original.exists(), "transcript should have moved away");
        let backup = PathBuf::from(handle.backup_dir.clone().unwrap());
        assert!(backup.join(MANIFEST).is_file(), "manifest should be written");

        undo(&handle).expect("undo failed");
        assert!(original.exists(), "undo should restore the transcript");
        assert_eq!(std::fs::read(&original).unwrap(), before, "contents changed");
        assert!(!backup.exists(), "empty backup dir should be cleaned up");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Deleted view's route: no handle in memory, only what is on disk.
    #[test]
    fn a_backup_can_be_listed_and_restored_from_disk_alone() {
        let dir = std::env::temp_dir().join("vastdeck-del-test3");
        let _ = std::fs::remove_dir_all(&dir);
        let session = fixture(&dir, "22222222-3333-4444-5555-666666666666");
        let original = PathBuf::from(&session.path);

        let handle = soft_delete(&session).expect("soft delete failed");
        let backup = handle.backup_dir.clone().unwrap();

        let listed = list_deleted().expect("listing failed");
        let mine = listed
            .iter()
            .find(|d| d.session_id == session.id)
            .expect("the backup should be listed");
        assert!(mine.restorable, "a manifest was written, so it is restorable");
        assert_eq!(mine.workspace, session.workspace);

        restore(&backup).expect("restore failed");
        assert!(original.exists(), "restore should put the transcript back");
        assert!(
            !listed.is_empty() && !PathBuf::from(&backup).exists(),
            "the backup folder should be gone"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_refuses_to_overwrite_a_transcript_that_came_back() {
        let dir = std::env::temp_dir().join("vastdeck-del-test4");
        let _ = std::fs::remove_dir_all(&dir);
        let session = fixture(&dir, "33333333-4444-5555-6666-777777777777");

        let handle = soft_delete(&session).expect("soft delete failed");
        let backup = handle.backup_dir.clone().unwrap();
        // The CLI created a new session at the same path in the meantime.
        std::fs::write(&session.path, "{\"type\":\"user\"}\n").unwrap();

        let err = restore(&backup).expect_err("should refuse");
        assert!(err.to_string().contains("already exists"), "{err}");

        purge(&backup).expect("purge failed");
        assert!(!PathBuf::from(&backup).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn purge_refuses_paths_outside_the_backup_folder() {
        let outside = std::env::temp_dir().join("vastdeck-not-a-backup");
        std::fs::create_dir_all(&outside).unwrap();
        assert!(purge(&outside.to_string_lossy()).is_err());
        assert!(outside.exists(), "nothing outside the folder may be touched");
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn hard_delete_removes_the_file() {
        let dir = std::env::temp_dir().join("vastdeck-del-test2");
        let _ = std::fs::remove_dir_all(&dir);
        let session = fixture(&dir, "66666666-7777-8888-9999-000000000000");
        let path = PathBuf::from(&session.path);

        let handle = hard_delete(&session).expect("hard delete failed");
        assert!(!path.exists());
        assert!(handle.backup_dir.is_none(), "hard delete must not claim a backup");
        assert!(undo(&handle).is_err(), "a permanent delete cannot be undone");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs the real `opencode` CLI against a throwaway data folder, never the
    /// user's own database. Needs an `opencode export` file to seed it:
    /// `VASTDECK_OPENCODE_FIXTURE=<export.json> cargo test -- --ignored opencode_round_trip`
    #[test]
    #[ignore = "runs the opencode CLI; needs VASTDECK_OPENCODE_FIXTURE"]
    fn opencode_round_trip() {
        let fixture = PathBuf::from(std::env::var("VASTDECK_OPENCODE_FIXTURE").unwrap());
        let root = std::env::temp_dir().join("vastdeck-opencode-round-trip");
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("CLAUDE_CONFIG_DIR", root.join("claude"));

        let (id, _) = serde_json::from_slice::<serde_json::Value>(&std::fs::read(&fixture).unwrap())
            .ok()
            .and_then(|v| Some((v.pointer("/info/id")?.as_str()?.to_string(), ())))
            .unwrap();
        let ws = workspace.to_string_lossy().into_owned();
        opencode::import(&id, &fixture, &ws).unwrap();

        let find = || opencode::scan().unwrap().into_iter().find(|s| s.id == id);
        let session = find().expect("imported session not listed");

        let handle = soft_delete(&session).unwrap();
        assert!(find().is_none(), "session still listed after delete");
        let deleted = list_deleted().unwrap();
        assert_eq!(deleted.len(), 1);
        assert!(deleted[0].restorable);
        assert_eq!(deleted[0].title, session.title);

        undo(&handle).unwrap();
        let back = find().expect("session not back after restore");
        assert_eq!(back.workspace, session.workspace);
        assert_eq!(back.message_count, session.message_count);
        assert!(list_deleted().unwrap().is_empty());

        // Restoring over a session that exists again must refuse, not overwrite.
        assert!(opencode::import(&id, &fixture, &ws).is_err());

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Moves an Antigravity conversation out and back inside a throwaway copy
    /// of the CLI folder:
    /// `VASTDECK_ANTIGRAVITY_HOME=<copy> VASTDECK_ANTIGRAVITY_ID=<id>
    /// cargo test -- --ignored antigravity_round_trip`
    #[test]
    #[ignore = "needs a copy of the Antigravity CLI folder"]
    fn antigravity_round_trip() {
        let home = PathBuf::from(std::env::var("VASTDECK_ANTIGRAVITY_HOME").unwrap());
        let id = std::env::var("VASTDECK_ANTIGRAVITY_ID").unwrap();
        let root = std::env::temp_dir().join("vastdeck-antigravity-round-trip");
        let _ = std::fs::remove_dir_all(&root);
        std::env::set_var("CLAUDE_CONFIG_DIR", root.join("claude"));

        let find = || antigravity::scan().unwrap().into_iter().find(|s| s.id == id);
        let session = find().expect("conversation not listed");
        let present = |rel: &PathBuf| home.join(rel).exists();
        let owned: Vec<PathBuf> = antigravity::owned_paths(&id)
            .into_iter()
            .filter(|rel| present(rel))
            .collect();

        let handle = soft_delete(&session).unwrap();
        assert!(find().is_none(), "conversation still listed after delete");
        assert!(owned.iter().all(|rel| !present(rel)), "files left behind");
        let deleted = list_deleted().unwrap();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].title, session.title);
        assert_eq!(deleted[0].workspace, session.workspace);

        undo(&handle).unwrap();
        assert!(find().is_some(), "conversation not back after restore");
        assert!(owned.iter().all(present), "files not restored");
        assert!(list_deleted().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Runs the real `codex` CLI against a copy of a Codex home, never the
    /// user's own. The copy's rollout paths must point inside it:
    /// `VASTDECK_CODEX_FIXTURE=<codex home copy> VASTDECK_CODEX_THREAD=<id>
    /// cargo test -- --ignored codex_round_trip`
    #[test]
    #[ignore = "runs the codex CLI; needs VASTDECK_CODEX_FIXTURE"]
    fn codex_round_trip() {
        let fixture = PathBuf::from(std::env::var("VASTDECK_CODEX_FIXTURE").unwrap());
        let id = std::env::var("VASTDECK_CODEX_THREAD").unwrap();
        let root = std::env::temp_dir().join("vastdeck-codex-round-trip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("CODEX_HOME", &fixture);
        std::env::set_var("CLAUDE_CONFIG_DIR", root.join("claude"));

        let find = || codex::scan().unwrap().into_iter().find(|s| s.id == id);
        let session = find().expect("thread not listed");

        let handle = soft_delete(&session).unwrap();
        assert!(find().is_none(), "thread still listed after delete");
        let deleted = list_deleted().unwrap();
        assert_eq!(deleted.len(), 1);
        assert!(deleted[0].restorable);
        assert_eq!(deleted[0].title, session.title);
        assert_eq!(deleted[0].workspace, session.workspace);

        undo(&handle).unwrap();
        assert!(find().is_some(), "thread not back after restore");
        assert!(list_deleted().unwrap().is_empty());

        // Purging the record deletes the archived thread from Codex too.
        let handle = soft_delete(&session).unwrap();
        purge(handle.backup_dir.as_deref().unwrap()).unwrap();
        assert!(list_deleted().unwrap().is_empty());
        assert!(codex::unarchive(&id).is_err(), "thread survived the purge");

        let _ = std::fs::remove_dir_all(&root);
    }
}