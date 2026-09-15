use serde::{Deserialize, Serialize};

/// A single CLI session as shown in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub provider: String,
    /// Absolute path to the transcript file backing this session.
    pub path: String,
    /// Real working directory, taken from the transcript (the folder name is lossy).
    pub workspace: String,
    /// Last `ai-title` recorded for the session.
    pub title: Option<String>,
    /// Last user prompt, used as the one-line preview.
    pub preview: Option<String>,
    pub git_branch: Option<String>,
    pub cli_version: Option<String>,
    /// Last `permission-mode` the session ran under.
    pub permission_mode: Option<String>,
    /// Epoch millis of the first timestamp in the transcript.
    pub created_at: Option<i64>,
    /// Epoch millis, from file mtime — far cheaper than parsing the tail timestamp.
    pub updated_at: i64,
    pub size_bytes: u64,
    /// Filled in by the deep scan; `None` until that pass lands.
    pub message_count: Option<u32>,
    pub live: Option<LiveInfo>,
}

/// Present only while a `claude` process is actually running this session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveInfo {
    pub pid: u32,
    /// "idle" or "busy", as reported by the CLI itself.
    pub status: String,
    pub name: Option<String>,
    pub started_at: Option<i64>,
}

/// Aggregate figures the CLI keeps per workspace, not per session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStats {
    pub cost_usd: Option<f64>,
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
    pub last_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchMode {
    /// `--resume` alone: permission prompts stay on.
    Normal,
    /// `--dangerously-skip-permissions`
    SkipPermissions,
    /// `--permission-mode acceptEdits`
    AcceptEdits,
    /// `--permission-mode plan`
    Plan,
    /// `--resume --fork-session`: branches off, leaving the original intact.
    Fork,
}

impl LaunchMode {
    /// Extra flags this mode appends after `--resume <id>`.
    pub fn flags(self) -> &'static [&'static str] {
        match self {
            LaunchMode::Normal => &[],
            LaunchMode::SkipPermissions => &["--dangerously-skip-permissions"],
            LaunchMode::AcceptEdits => &["--permission-mode", "acceptEdits"],
            LaunchMode::Plan => &["--permission-mode", "plan"],
            LaunchMode::Fork => &["--fork-session"],
        }
    }
}

/// A transcript sitting in the backup folder, waiting to be restored or purged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedSession {
    pub backup_dir: String,
    pub session_id: String,
    pub workspace: String,
    pub title: Option<String>,
    pub preview: Option<String>,
    /// Epoch millis the delete happened.
    pub deleted_at: i64,
    pub size_bytes: u64,
    /// False for a backup with no manifest — an older one, or one edited by
    /// hand. It can still be purged, just not put back automatically.
    pub restorable: bool,
}

/// A completed soft delete, retained so the UI can offer an undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedHandle {
    pub session_id: String,
    /// Where the transcript was moved to, or `None` for a hard delete.
    pub backup_dir: Option<String>,
    pub original_path: String,
    pub original_env_dir: Option<String>,
}
