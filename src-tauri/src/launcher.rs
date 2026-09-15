//! Opening a session in a terminal.
//!
//! Quoting is the whole problem here. Workspace paths routinely look like
//! `C:\Work Projects (2026)\.config\my-app` — spaces and parentheses — and
//! Windows Terminal additionally treats `;` as a command separator, so nesting a
//! command inside a `wt` invocation is a reliable way to get it wrong.
//!
//! So nothing is nested. Each launch writes a tiny script into Vastdeck's own
//! app-data folder (a path with no spaces or semicolons) that does the `cd` and
//! the `claude` call itself, and the terminal is handed only that clean path.

use crate::model::{LaunchMode, Session};
use crate::paths;
use crate::settings::{Settings, TerminalChoice};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Locates `claude`, preferring an explicit override, then PATH, then the
/// default install location.
pub fn resolve_claude(settings: &Settings) -> Result<PathBuf> {
    if let Some(custom) = settings.claude_path.as_ref().filter(|p| !p.is_empty()) {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Ok(path);
        }
    }

    let candidates = ["claude.exe", "claude.cmd", "claude.bat", "claude"];
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in candidates {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        for name in candidates {
            let candidate = home.join(".local").join("bin").join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(anyhow!("could not find the claude executable on PATH"))
}

fn which(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(exe))
        .find(|p| p.is_file())
}

fn scripts_dir() -> Result<PathBuf> {
    let dir = paths::app_data_dir()?.join("launch");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Launch scripts are single-use; clear out anything older than an hour.
fn sweep_old_scripts(dir: &Path) {
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|m| m < cutoff)
                .unwrap_or(false);
            if stale {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// A console title only; strip anything that could confuse a shell.
fn sanitize_title(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Claude Code".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

fn args_for(session: &Session, mode: LaunchMode) -> Vec<String> {
    let mut args = vec!["--resume".to_string(), session.id.clone()];
    args.extend(mode.flags().iter().map(|s| s.to_string()));
    args
}

fn write_cmd_script(session: &Session, claude: &Path, mode: LaunchMode) -> Result<PathBuf> {
    let dir = scripts_dir()?;
    sweep_old_scripts(&dir);
    let file = dir.join(script_name(session, "cmd"));

    let args = args_for(session, mode)
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(" ");

    let body = format!(
        "@echo off\r\n\
         title {title}\r\n\
         cd /d \"{cwd}\"\r\n\
         \"{claude}\" {args}\r\n",
        title = sanitize_title(display_name(session)),
        cwd = session.workspace,
        claude = claude.display(),
        args = args,
    );
    std::fs::write(&file, body)?;
    Ok(file)
}

fn write_ps_script(session: &Session, claude: &Path, mode: LaunchMode) -> Result<PathBuf> {
    let dir = scripts_dir()?;
    sweep_old_scripts(&dir);
    let file = dir.join(script_name(session, "ps1"));

    let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let args = args_for(session, mode)
        .iter()
        .map(|a| quote(a))
        .collect::<Vec<_>>()
        .join(" ");

    let body = format!(
        "$Host.UI.RawUI.WindowTitle = {title}\r\n\
         Set-Location -LiteralPath {cwd}\r\n\
         & {claude} {args}\r\n",
        title = quote(&sanitize_title(display_name(session))),
        cwd = quote(&session.workspace),
        claude = quote(&claude.display().to_string()),
        args = args,
    );
    std::fs::write(&file, body)?;
    Ok(file)
}

fn display_name(session: &Session) -> &str {
    session
        .live
        .as_ref()
        .and_then(|l| l.name.as_deref())
        .or(session.title.as_deref())
        .unwrap_or("Claude Code")
}

/// Unique name for a launch script. The millisecond clock alone is not enough —
/// two launches of the same session inside one tick would land on the same file
/// and one would overwrite the other — so a per-process counter is appended.
fn script_name(session: &Session, ext: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let short = &session.id[..8.min(session.id.len())];
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{millis}-{seq}-{short}.{ext}")
}

/// Command line for Windows Terminal. `new_tab` decides whether the session
/// joins the window already on screen or gets one of its own: `-w 0` targets the
/// most recently used window, and without it `wt` opens a new one.
fn wt_args(cmd_exe: &Path, script: &Path, new_tab: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if new_tab {
        args.push("-w".into());
        args.push("0".into());
    }
    args.push("new-tab".into());
    args.push(cmd_exe.to_string_lossy().into_owned());
    args.push("/k".into());
    args.push(script.to_string_lossy().into_owned());
    args
}

/// Opens `session` in a terminal under `mode`. Returns the terminal that was used.
pub fn launch(session: &Session, mode: LaunchMode, settings: &Settings) -> Result<String> {
    if !Path::new(&session.workspace).is_dir() {
        return Err(anyhow!(
            "workspace no longer exists: {}",
            session.workspace
        ));
    }
    let claude = resolve_claude(settings)?;

    let order: Vec<TerminalChoice> = match settings.terminal {
        TerminalChoice::Auto => vec![
            TerminalChoice::WindowsTerminal,
            TerminalChoice::Pwsh,
            TerminalChoice::PowerShell,
            TerminalChoice::Cmd,
        ],
        explicit => vec![explicit, TerminalChoice::Cmd],
    };

    let mut last_error = None;
    for choice in order {
        match try_launch(choice, session, &claude, mode, settings) {
            Ok(()) => return Ok(format!("{choice:?}")),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("no terminal available")))
}

fn try_launch(
    choice: TerminalChoice,
    session: &Session,
    claude: &Path,
    mode: LaunchMode,
    settings: &Settings,
) -> Result<()> {
    match choice {
        TerminalChoice::WindowsTerminal => {
            let wt = which("wt.exe").ok_or_else(|| anyhow!("wt.exe not found"))?;
            let script = write_cmd_script(session, claude, mode)?;
            let cmd_exe = which("cmd.exe").unwrap_or_else(|| PathBuf::from("cmd.exe"));

            let mut command = Command::new(wt);
            command.args(wt_args(&cmd_exe, &script, settings.wt_new_tab));
            spawn_detached(command)
        }
        TerminalChoice::Pwsh => {
            let exe = which("pwsh.exe").ok_or_else(|| anyhow!("pwsh.exe not found"))?;
            let script = write_ps_script(session, claude, mode)?;
            let mut command = Command::new(exe);
            command.args(["-NoLogo", "-NoExit", "-ExecutionPolicy", "Bypass", "-File"]);
            command.arg(script);
            spawn_console(command)
        }
        TerminalChoice::PowerShell => {
            let exe = which("powershell.exe")
                .ok_or_else(|| anyhow!("powershell.exe not found"))?;
            let script = write_ps_script(session, claude, mode)?;
            let mut command = Command::new(exe);
            command.args(["-NoLogo", "-NoExit", "-ExecutionPolicy", "Bypass", "-File"]);
            command.arg(script);
            spawn_console(command)
        }
        TerminalChoice::Cmd => {
            let exe = which("cmd.exe").unwrap_or_else(|| PathBuf::from("cmd.exe"));
            let script = write_cmd_script(session, claude, mode)?;
            let mut command = Command::new(exe);
            command.arg("/k");
            command.arg(script);
            spawn_console(command)
        }
        TerminalChoice::Auto => Err(anyhow!("Auto is resolved before this point")),
    }
}

#[cfg(windows)]
fn spawn_detached(mut command: Command) -> Result<()> {
    use std::os::windows::process::CommandExt;
    command.creation_flags(DETACHED_PROCESS);
    command.spawn()?;
    Ok(())
}

#[cfg(windows)]
fn spawn_console(mut command: Command) -> Result<()> {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NEW_CONSOLE);
    command.spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_detached(mut command: Command) -> Result<()> {
    command.spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_console(mut command: Command) -> Result<()> {
    command.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Session;

    fn fixture() -> Session {
        Session {
            id: "62f777eb-0a59-4861-bb96-fe731ca6a36e".into(),
            provider: "claude-code".into(),
            path: String::new(),
            // The real shape of a path on this machine: spaces, a dot-folder and
            // parentheses. This is what the quoting has to survive.
            workspace: r"C:\Work Projects (2026)\.config\my-app".into(),
            title: Some("Vastdeck; rm -rf /".into()),
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

    #[test]
    fn cmd_script_quotes_the_workspace_and_carries_the_mode() {
        let session = fixture();
        let claude = Path::new(r"C:\Users\dev\.local\bin\claude.exe");
        let file =
            write_cmd_script(&session, claude, LaunchMode::SkipPermissions).expect("write failed");
        let body = std::fs::read_to_string(&file).unwrap();

        assert!(
            body.contains(r#"cd /d "C:\Work Projects (2026)\.config\my-app""#),
            "workspace not quoted: {body}"
        );
        assert!(body.contains(r#""--resume" "62f777eb-0a59-4861-bb96-fe731ca6a36e""#));
        assert!(body.contains(r#""--dangerously-skip-permissions""#));
        // A title only ever becomes a console title; `;` would otherwise split a
        // Windows Terminal command line.
        assert!(!body.contains(';'), "title metacharacters leaked: {body}");

        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn ps_script_quotes_the_workspace_and_carries_the_mode() {
        let session = fixture();
        let claude = Path::new(r"C:\Users\dev\.local\bin\claude.exe");
        let file = write_ps_script(&session, claude, LaunchMode::Plan).expect("write failed");
        let body = std::fs::read_to_string(&file).unwrap();

        assert!(
            body.contains(r"Set-Location -LiteralPath 'C:\Work Projects (2026)\.config\my-app'"),
            "workspace not quoted: {body}"
        );
        assert!(body.contains("'--permission-mode' 'plan'"));

        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn single_quotes_in_a_path_are_doubled_for_powershell() {
        let mut session = fixture();
        session.workspace = r"C:\it's mine\proj".into();
        let claude = Path::new(r"C:\bin\claude.exe");
        let file = write_ps_script(&session, claude, LaunchMode::Normal).expect("write failed");
        let body = std::fs::read_to_string(&file).unwrap();

        assert!(
            body.contains(r"Set-Location -LiteralPath 'C:\it''s mine\proj'"),
            "apostrophe not escaped: {body}"
        );
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn each_mode_maps_to_the_documented_flags() {
        assert!(LaunchMode::Normal.flags().is_empty());
        assert_eq!(
            LaunchMode::SkipPermissions.flags(),
            ["--dangerously-skip-permissions"]
        );
        assert_eq!(
            LaunchMode::AcceptEdits.flags(),
            ["--permission-mode", "acceptEdits"]
        );
        assert_eq!(LaunchMode::Plan.flags(), ["--permission-mode", "plan"]);
        assert_eq!(LaunchMode::Fork.flags(), ["--fork-session"]);
    }

    #[test]
    fn wt_joins_the_current_window_only_when_asked() {
        let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
        let script = Path::new(r"C:\Users\dev\AppData\Local\vastdeck\launch\1-0-62f777eb.cmd");

        let tab = wt_args(cmd, script, true);
        assert_eq!(&tab[..3], ["-w", "0", "new-tab"], "should target window 0");

        let window = wt_args(cmd, script, false);
        assert_eq!(window[0], "new-tab", "a new window must not pass -w");
        assert!(!window.contains(&"-w".to_string()));

        // Both forms still hand the terminal nothing but the script path.
        for args in [tab, window] {
            assert_eq!(args[args.len() - 2], "/k");
            assert_eq!(args[args.len() - 1], script.to_string_lossy());
        }
    }

    #[test]
    fn titles_are_reduced_to_safe_characters() {
        assert_eq!(sanitize_title("Vastdeck; rm -rf /"), "Vastdeck rm -rf");
        assert_eq!(sanitize_title("   "), "Claude Code");
        assert_eq!(sanitize_title(r#"a&b|c"d""#), "abcd");
    }
}
