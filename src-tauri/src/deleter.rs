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

use crate::model::{DeletedHandle, DeletedSession, Session};
use crate::paths;
use crate::scanner;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MANIFEST: &str = "vastdeck.json";

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
}

pub fn soft_delete(session: &Session) -> Result<DeletedHandle> {
    let transcript = PathBuf::from(&session.path);
    if !transcript.is_file() {
        return Err(anyhow!("transcript is already gone: {}", session.path));
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let short = &session.id[..8.min(session.id.len())];
    let backup = paths::backups_dir()?.join(format!("{stamp}-{short}"));
    std::fs::create_dir_all(&backup)?;

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
        let Some(transcript) = transcript_in(&dir) else {
            continue;
        };
        let size = std::fs::metadata(&transcript).map(|m| m.len()).unwrap_or(0);
        let manifest = read_manifest(&dir);
        let meta = scanner::quick_meta(&transcript).ok();

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

fn transcript_in(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.extension().and_then(|x| x.to_str()) == Some("jsonl")).then_some(p)
    })
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
}
