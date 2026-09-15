use crate::model::LaunchMode;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalChoice {
    /// Windows Terminal, then pwsh, then powershell, then cmd.
    Auto,
    WindowsTerminal,
    Pwsh,
    PowerShell,
    Cmd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub terminal: TerminalChoice,
    /// Open in a tab of the existing Windows Terminal window rather than a new one.
    pub wt_new_tab: bool,
    /// Closing hides the window to the notification area instead of quitting.
    /// Quitting for real then happens from the tray icon's menu.
    #[serde(default)]
    pub close_to_tray: bool,
    /// Launch Vastdeck when Windows starts. Mirrored into the registry Run key
    /// by the autostart plugin, which stays the source of truth.
    #[serde(default)]
    pub start_with_windows: bool,
    /// What a plain click does. Deliberately never `SkipPermissions` by default.
    pub default_launch_mode: LaunchMode,
    /// Skip the backup copy and remove transcripts outright.
    pub hard_delete: bool,
    pub theme: String,
    pub group_by_workspace: bool,
    pub sort: String,
    /// Last mode used per session, so the split button remembers.
    #[serde(default)]
    pub session_modes: HashMap<String, LaunchMode>,
    /// Workspaces where skip-permissions has already been confirmed once.
    #[serde(default)]
    pub trusted_workspaces: Vec<String>,
    /// Overrides PATH lookup when the CLI lives somewhere unusual.
    #[serde(default)]
    pub claude_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            terminal: TerminalChoice::Auto,
            wt_new_tab: true,
            close_to_tray: false,
            start_with_windows: false,
            default_launch_mode: LaunchMode::Normal,
            hard_delete: false,
            theme: "system".to_string(),
            group_by_workspace: true,
            sort: "recent".to_string(),
            session_modes: HashMap::new(),
            trusted_workspaces: Vec::new(),
            claude_path: None,
        }
    }
}

pub struct SettingsStore {
    inner: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load() -> Self {
        let settings = Self::file()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default();
        Self { inner: Mutex::new(settings) }
    }

    fn file() -> Option<std::path::PathBuf> {
        paths::app_data_dir().ok().map(|d| d.join("settings.json"))
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn set(&self, next: Settings) {
        if let Ok(mut current) = self.inner.lock() {
            *current = next;
        }
        self.save();
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) {
        if let Ok(mut current) = self.inner.lock() {
            f(&mut current);
        }
        self.save();
    }

    fn save(&self) {
        let (Some(path), Ok(settings)) = (Self::file(), self.inner.lock()) else {
            return;
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&*settings) {
            let _ = std::fs::write(path, bytes);
        }
    }
}
