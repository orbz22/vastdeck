//! Transcript scanning.
//!
//! Transcripts reach 18 MB and the whole tree is ~420 MB here, so a session is
//! never parsed end to end. Everything the list needs comes from a small head
//! window, a small tail window, and the file metadata:
//!
//! - head  → `cwd`, `version`, `gitBranch`, first timestamp
//! - tail  → last `ai-title`, `last-prompt`, `permission-mode`
//! - stat  → size, and mtime as the "updated" time
//!
//! Message counts need the whole file, so they are left to [`deep_scan`], which
//! runs after the list is already on screen and is cached per (mtime, size).

use crate::model::Session;
use crate::paths;
use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const HEAD_WINDOW: u64 = 64 * 1024;
const TAIL_WINDOW: u64 = 128 * 1024;
/// Windows tried in order when the first pass comes up short.
const EXPANDED_WINDOWS: [u64; 2] = [1024 * 1024, 8 * 1024 * 1024];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    mtime_ms: i64,
    size: u64,
    workspace: String,
    title: Option<String>,
    preview: Option<String>,
    git_branch: Option<String>,
    cli_version: Option<String>,
    permission_mode: Option<String>,
    created_at: Option<i64>,
    #[serde(default)]
    message_count: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Cache {
    #[serde(default)]
    entries: HashMap<String, CacheEntry>,
}

pub struct ScanCache {
    inner: Mutex<Cache>,
    file: PathBuf,
}

impl ScanCache {
    pub fn load() -> Self {
        let file = paths::app_data_dir()
            .map(|d| d.join("scan-cache.json"))
            .unwrap_or_else(|_| PathBuf::from("scan-cache.json"));
        let cache = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Cache>(&b).ok())
            .unwrap_or_default();
        Self { inner: Mutex::new(cache), file }
    }

    pub fn save(&self) {
        if let Ok(cache) = self.inner.lock() {
            if let Ok(bytes) = serde_json::to_vec(&*cache) {
                let _ = std::fs::write(&self.file, bytes);
            }
        }
    }

    fn get(&self, key: &str, mtime_ms: i64, size: u64) -> Option<CacheEntry> {
        let cache = self.inner.lock().ok()?;
        let entry = cache.entries.get(key)?;
        (entry.mtime_ms == mtime_ms && entry.size == size).then(|| entry.clone())
    }

    fn put(&self, key: String, entry: CacheEntry) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.entries.insert(key, entry);
        }
    }

    /// Drops entries whose transcript no longer exists, so the file cannot grow
    /// without bound as sessions are deleted.
    fn retain_existing(&self, live_keys: &[String]) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.entries.retain(|k, _| live_keys.iter().any(|l| l == k));
        }
    }
}

/// Every `.jsonl` under `~/.claude/projects/*/`.
pub fn transcript_files() -> Result<Vec<PathBuf>> {
    let root = paths::projects_dir()?;
    let mut out = Vec::new();
    let Ok(projects) = std::fs::read_dir(&root) else {
        return Ok(out);
    };
    for project in projects.flatten() {
        if !project.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// Scans every transcript in parallel. Sessions with no real conversation in
/// them — the CLI leaves behind stubs of a few hundred bytes — are dropped.
pub fn scan_all(cache: &ScanCache) -> Result<Vec<Session>> {
    let files = transcript_files()?;
    let keys: Vec<String> = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();

    let sessions: Vec<Session> = files
        .par_iter()
        .filter_map(|path| scan_one(path, cache).ok().flatten())
        .collect();

    cache.retain_existing(&keys);
    cache.save();
    Ok(sessions)
}

fn scan_one(path: &Path, cache: &ScanCache) -> Result<Option<Session>> {
    let meta = std::fs::metadata(path)?;
    let size = meta.len();
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let key = path.to_string_lossy().into_owned();
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let entry = match cache.get(&key, mtime_ms, size) {
        Some(hit) => hit,
        None => {
            let head = parse_head(path, size)?;
            // A stub with no user turn is not a session anyone wants listed.
            if head.workspace.is_none() || (!head.has_user && size <= HEAD_WINDOW) {
                return Ok(None);
            }
            let tail = parse_tail(path, size)?;
            let entry = CacheEntry {
                mtime_ms,
                size,
                workspace: head.workspace.unwrap_or_default(),
                title: tail.title,
                preview: tail.preview,
                git_branch: head.git_branch,
                cli_version: head.cli_version,
                permission_mode: tail.permission_mode,
                created_at: head.created_at,
                message_count: None,
            };
            cache.put(key, entry.clone());
            entry
        }
    };

    if entry.workspace.is_empty() {
        return Ok(None);
    }

    Ok(Some(Session {
        id,
        provider: "claude-code".to_string(),
        path: path.to_string_lossy().into_owned(),
        workspace: entry.workspace,
        title: entry.title,
        preview: entry.preview,
        git_branch: entry.git_branch,
        cli_version: entry.cli_version,
        permission_mode: entry.permission_mode,
        created_at: entry.created_at,
        updated_at: mtime_ms,
        size_bytes: size,
        message_count: entry.message_count,
        live: None,
    }))
}

/// Just enough to describe a transcript that is not part of the live list —
/// a backed-up one, for instance.
pub struct QuickMeta {
    pub workspace: Option<String>,
    pub title: Option<String>,
    pub preview: Option<String>,
}

pub fn quick_meta(path: &Path) -> Result<QuickMeta> {
    let size = std::fs::metadata(path)?.len();
    let head = parse_head(path, size)?;
    let tail = parse_tail(path, size)?;
    Ok(QuickMeta {
        workspace: head.workspace,
        title: tail.title,
        preview: tail.preview,
    })
}

#[derive(Default)]
struct Head {
    workspace: Option<String>,
    git_branch: Option<String>,
    cli_version: Option<String>,
    created_at: Option<i64>,
    has_user: bool,
}

fn parse_head(path: &Path, size: u64) -> Result<Head> {
    let mut windows = vec![HEAD_WINDOW];
    windows.extend(EXPANDED_WINDOWS);

    let mut head = Head::default();
    for window in windows {
        let bytes = read_window(path, 0, window.min(size))?;
        let text = String::from_utf8_lossy(&bytes);
        // The final line is probably cut off unless we reached the end of file.
        let complete = window >= size;
        let mut lines: Vec<&str> = text.split('\n').collect();
        if !complete && lines.len() > 1 {
            lines.pop();
        }

        for line in lines {
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
            if matches!(kind, "user" | "assistant")
                && !v.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
            {
                head.has_user = true;
            }
            if head.workspace.is_none() {
                if let Some(cwd) = v.get("cwd").and_then(Value::as_str) {
                    head.workspace = Some(cwd.to_string());
                }
            }
            if head.cli_version.is_none() {
                if let Some(ver) = v.get("version").and_then(Value::as_str) {
                    head.cli_version = Some(ver.to_string());
                }
            }
            if head.git_branch.is_none() {
                if let Some(branch) = v.get("gitBranch").and_then(Value::as_str) {
                    if !branch.is_empty() {
                        head.git_branch = Some(branch.to_string());
                    }
                }
            }
            if head.created_at.is_none() {
                if let Some(ts) = v.get("timestamp").and_then(Value::as_str) {
                    head.created_at = parse_ts(ts);
                }
            }
        }

        if head.workspace.is_some() || window >= size {
            break;
        }
    }
    Ok(head)
}

#[derive(Default)]
struct Tail {
    title: Option<String>,
    preview: Option<String>,
    permission_mode: Option<String>,
}

fn parse_tail(path: &Path, size: u64) -> Result<Tail> {
    let mut windows = vec![TAIL_WINDOW];
    windows.extend(EXPANDED_WINDOWS);

    let mut tail = Tail::default();
    for window in windows {
        let window = window.min(size);
        let start = size - window;
        let bytes = read_window(path, start, window)?;
        let text = String::from_utf8_lossy(&bytes);
        let mut lines: Vec<&str> = text.split('\n').collect();
        // Unless we started at byte 0 the first line is a fragment.
        if start > 0 && !lines.is_empty() {
            lines.remove(0);
        }

        // Walk backwards: the last occurrence of each field is the current one.
        for line in lines.iter().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            match v.get("type").and_then(Value::as_str).unwrap_or("") {
                "ai-title" if tail.title.is_none() => {
                    tail.title = v
                        .get("aiTitle")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                "last-prompt" if tail.preview.is_none() => {
                    tail.preview = v
                        .get("lastPrompt")
                        .and_then(Value::as_str)
                        .map(|s| clean_preview(s));
                }
                "permission-mode" if tail.permission_mode.is_none() => {
                    tail.permission_mode = v
                        .get("permissionMode")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                _ => {}
            }
            if tail.title.is_some() && tail.preview.is_some() && tail.permission_mode.is_some() {
                return Ok(tail);
            }
        }

        // Finding *any* of the three means the window held real records, so a
        // missing field is genuinely absent — older transcripts carry no
        // `ai-title` at all. Widening the window would just re-read megabytes
        // to come back empty again.
        let found_something =
            tail.title.is_some() || tail.preview.is_some() || tail.permission_mode.is_some();
        if found_something || window >= size {
            break;
        }
    }
    Ok(tail)
}

fn read_window(path: &Path, start: u64, len: u64) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0u8; len as usize];
    let mut filled = 0usize;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

/// Prompts arrive with command wrappers and newlines in them; the list shows one line.
fn clean_preview(raw: &str) -> String {
    let mut s = raw.replace(['\n', '\r', '\t'], " ");
    for tag in [
        "<local-command-caveat>",
        "</local-command-caveat>",
        "<command-name>",
        "</command-name>",
        "<command-message>",
        "</command-message>",
        "<command-args>",
        "</command-args>",
    ] {
        s = s.replace(tag, " ");
    }
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > 240 {
        collapsed.chars().take(240).collect::<String>() + "…"
    } else {
        collapsed
    }
}

fn parse_ts(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// Counts conversation turns by scanning the raw bytes for the type markers.
/// No JSON parsing, so this runs at disk speed; still, it touches every byte and
/// therefore belongs on a background pass rather than the initial list.
pub fn deep_scan(paths: &[PathBuf], cache: &ScanCache) -> Vec<(String, u32)> {
    let out: Vec<(String, u32)> = paths
        .par_iter()
        .filter_map(|path| {
            let meta = std::fs::metadata(path).ok()?;
            let size = meta.len();
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let key = path.to_string_lossy().into_owned();
            let id = path.file_stem()?.to_string_lossy().into_owned();

            if let Some(entry) = cache.get(&key, mtime_ms, size) {
                if let Some(count) = entry.message_count {
                    return Some((id, count));
                }
            }

            let count = count_turns(path).ok()?;
            if let Some(mut entry) = cache.get(&key, mtime_ms, size) {
                entry.message_count = Some(count);
                cache.put(key, entry);
            }
            Some((id, count))
        })
        .collect();
    cache.save();
    out
}

fn count_turns(path: &Path) -> Result<u32> {
    const USER: &[u8] = br#""type":"user""#;
    const ASSISTANT: &[u8] = br#""type":"assistant""#;
    let overlap = ASSISTANT.len() - 1;

    let mut file = File::open(path)?;
    let mut buf = vec![0u8; 1024 * 1024 + overlap];
    let mut carry = 0usize;
    let mut total = 0u32;

    loop {
        let read = file.read(&mut buf[carry..])?;
        if read == 0 {
            break;
        }
        let filled = carry + read;
        let slice = &buf[..filled];
        total += memchr::memmem::find_iter(slice, USER).count() as u32;
        total += memchr::memmem::find_iter(slice, ASSISTANT).count() as u32;

        // Keep the tail so a marker straddling the boundary is not missed, and
        // trim it back off the next window's count by starting the copy there.
        if filled > overlap {
            carry = overlap;
            buf.copy_within(filled - overlap..filled, 0);
            // Subtract matches that fall entirely inside the carried bytes, since
            // the next pass will see them again.
            let carried = &buf[..overlap];
            total -= memchr::memmem::find_iter(carried, USER).count() as u32;
            total -= memchr::memmem::find_iter(carried, ASSISTANT).count() as u32;
        } else {
            carry = 0;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test against the real `~/.claude` tree: it is the only place with
    /// transcripts of the size this code exists to cope with. Skips cleanly when
    /// there is nothing installed.
    #[test]
    fn scans_real_transcripts_quickly() {
        let files = transcript_files().unwrap_or_default();
        if files.is_empty() {
            eprintln!("no transcripts found; skipping");
            return;
        }
        let total: u64 = files
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();

        // Cold: ignore whatever the cache already holds.
        let cache = ScanCache { inner: Mutex::new(Cache::default()), file: std::env::temp_dir().join("vastdeck-test-cache.json") };
        let start = std::time::Instant::now();
        let sessions = scan_all(&cache).expect("scan failed");
        let cold = start.elapsed();

        let start = std::time::Instant::now();
        let again = scan_all(&cache).expect("second scan failed");
        let warm = start.elapsed();

        eprintln!(
            "{} files / {:.0} MB -> {} sessions | cold {:?} | warm {:?}",
            files.len(),
            total as f64 / 1_048_576.0,
            sessions.len(),
            cold,
            warm
        );
        for s in sessions.iter().take(5) {
            eprintln!(
                "  {:<28} {:>7} KB  {}  {:?}",
                s.title.as_deref().unwrap_or("(untitled)").chars().take(28).collect::<String>(),
                s.size_bytes / 1024,
                s.workspace,
                s.permission_mode
            );
        }

        assert_eq!(sessions.len(), again.len(), "cache changed the result");
        assert!(sessions.iter().all(|s| !s.workspace.is_empty()));
        assert!(cold.as_millis() < 3000, "cold scan too slow: {cold:?}");
    }

    #[test]
    fn counts_turns_across_chunk_boundaries() {
        let dir = std::env::temp_dir().join("vastdeck-count-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.jsonl");

        // Pad so the markers land on either side of the 1 MiB read boundary.
        let mut body = String::new();
        for i in 0..40_000 {
            body.push_str(&format!(
                "{{\"type\":\"{}\",\"pad\":\"{}\"}}\n",
                if i % 2 == 0 { "user" } else { "assistant" },
                "x".repeat(30)
            ));
        }
        std::fs::write(&path, &body).unwrap();

        assert_eq!(count_turns(&path).unwrap(), 40_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
