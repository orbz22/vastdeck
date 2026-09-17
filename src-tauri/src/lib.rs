mod deleter;
mod launcher;
mod model;
mod paths;
mod registry;
mod scanner;
mod settings;
mod stats;
mod tray;
mod winproc;

use model::{DeletedHandle, DeletedSession, LaunchMode, Session, WorkspaceStats};
use scanner::ScanCache;
use serde::Serialize;
use settings::{Settings, SettingsStore};
use std::collections::HashMap;
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

struct AppState {
    cache: ScanCache,
    settings: SettingsStore,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionList {
    sessions: Vec<Session>,
    stats: HashMap<String, WorkspaceStats>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInfo {
    id: String,
    name: String,
    installed: bool,
    session_count: usize,
}

/// Passed to the copy of Vastdeck that Windows starts at login, so it can tell
/// that launch apart from the user opening the app themselves.
const AUTOSTART_FLAG: &str = "--autostart";

fn to_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn list_sessions(state: tauri::State<AppState>) -> Result<SessionList, String> {
    let mut sessions = scanner::scan_all(&state.cache).map_err(to_err)?;
    let live = registry::live_sessions().unwrap_or_default();
    for session in &mut sessions {
        session.live = live.get(&session.id).cloned();
    }
    // Most recent first; the UI re-sorts if the user asks for something else.
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let stats = stats::workspace_stats().unwrap_or_default();
    Ok(SessionList { sessions, stats })
}

/// Message counts need a full read of every transcript, so the UI asks for them
/// separately once the list is already on screen.
#[tauri::command]
fn message_counts(state: tauri::State<AppState>) -> Result<HashMap<String, u32>, String> {
    let files = scanner::transcript_files().map_err(to_err)?;
    Ok(scanner::deep_scan(&files, &state.cache).into_iter().collect())
}

#[tauri::command]
fn list_providers(state: tauri::State<AppState>) -> Result<Vec<ProviderInfo>, String> {
    let count = scanner::scan_all(&state.cache).map(|s| s.len()).unwrap_or(0);
    let installed = launcher::resolve_claude(&state.settings.get()).is_ok();
    Ok(vec![
        ProviderInfo {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            installed,
            session_count: count,
        },
        ProviderInfo {
            id: "codex".into(),
            name: "Codex CLI".into(),
            installed: false,
            session_count: 0,
        },
        ProviderInfo {
            id: "antigravity".into(),
            name: "Antigravity CLI".into(),
            installed: false,
            session_count: 0,
        },
    ])
}

#[tauri::command]
fn launch_session(
    state: tauri::State<AppState>,
    session: Session,
    mode: LaunchMode,
) -> Result<String, String> {
    let settings = state.settings.get();
    let terminal = launcher::launch(&session, mode, &settings).map_err(to_err)?;
    let id = session.id.clone();
    state.settings.update(|s| {
        s.session_modes.insert(id, mode);
    });
    Ok(terminal)
}

/// Raises the terminal already running this session rather than starting a second one.
#[tauri::command]
fn focus_session(pid: u32) -> Result<bool, String> {
    Ok(winproc::focus_window_for_pid(pid))
}

#[tauri::command]
fn delete_session(
    state: tauri::State<AppState>,
    session: Session,
) -> Result<DeletedHandle, String> {
    if state.settings.get().hard_delete {
        deleter::hard_delete(&session).map_err(to_err)
    } else {
        deleter::soft_delete(&session).map_err(to_err)
    }
}

#[tauri::command]
fn delete_session_permanently(session: Session) -> Result<DeletedHandle, String> {
    deleter::hard_delete(&session).map_err(to_err)
}

#[tauri::command]
fn undo_delete(handle: DeletedHandle) -> Result<(), String> {
    deleter::undo(&handle).map_err(to_err)
}

#[tauri::command]
fn list_deleted() -> Result<Vec<DeletedSession>, String> {
    deleter::list_deleted().map_err(to_err)
}

#[tauri::command]
fn restore_deleted(backup_dir: String) -> Result<(), String> {
    deleter::restore(&backup_dir).map_err(to_err)
}

#[tauri::command]
fn purge_deleted(backup_dir: String) -> Result<(), String> {
    deleter::purge(&backup_dir).map_err(to_err)
}

#[tauri::command]
fn purge_all_deleted() -> Result<usize, String> {
    deleter::purge_all().map_err(to_err)
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.settings.get()
}

/// Two settings reach outside the app — the tray icon and the Windows Run key —
/// so saving has to apply them, not just record them.
#[tauri::command]
fn set_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: Settings,
) -> Result<(), String> {
    let tray_wanted = settings.close_to_tray;
    let autostart_wanted = settings.start_with_windows;
    state.settings.set(settings);

    tray::sync_on_main_thread(&app, tray_wanted).map_err(to_err)?;
    apply_autostart(&app, autostart_wanted)?;
    Ok(())
}

fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if current == enabled {
        return Ok(());
    }
    if enabled {
        manager.enable().map_err(to_err)
    } else {
        manager.disable().map_err(to_err)
    }
}

/// Rewrites the Run key so it names the executable that is running right now.
///
/// The key stores an absolute path. A portable copy that has been moved, or an
/// install that has been upgraded into a new folder, would otherwise leave
/// Windows pointing at a file that is no longer there — and the failure is
/// silent: nothing launches at login while the setting still reads as on.
/// [`apply_autostart`] cannot catch that, because the key exists either way and
/// it only acts when the on/off state differs.
fn reassert_autostart(app: &tauri::AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    if enabled {
        let _ = manager.disable();
        let _ = manager.enable();
    } else if manager.is_enabled().unwrap_or(false) {
        let _ = manager.disable();
    }
}

/// Where settings, the scan cache and launch scripts are kept — next to the
/// executable in portable mode, otherwise under `%LOCALAPPDATA%`.
#[tauri::command]
fn data_location() -> Result<(String, bool), String> {
    let dir = paths::app_data_dir().map_err(to_err)?;
    Ok((dir.to_string_lossy().into_owned(), paths::is_portable()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    /// The updater works by running an installer, which a portable copy has no
    /// business doing — so the UI offers the release page instead.
    portable: bool,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        portable: paths::is_portable(),
    }
}

/// Records that the user has accepted skip-permissions for this workspace, so
/// the confirmation is asked once rather than every launch.
#[tauri::command]
fn trust_workspace(state: tauri::State<AppState>, workspace: String) {
    let key = stats::normalize(&workspace);
    state.settings.update(|s| {
        if !s.trusted_workspaces.contains(&key) {
            s.trusted_workspaces.push(key);
        }
    });
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    opener_reveal(&path).map_err(to_err)
}

fn opener_reveal(path: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe").arg(path).spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}

/// Watches the transcript tree and the live registry, coalescing the storm of
/// events an active session produces into one refresh every 600 ms.
fn spawn_watcher(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc::{channel, RecvTimeoutError};
        use std::time::Duration;

        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => return,
        };

        for dir in [paths::projects_dir(), paths::sessions_registry_dir()]
            .into_iter()
            .flatten()
        {
            let _ = watcher.watch(&dir, RecursiveMode::Recursive);
        }

        let mut pending = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(600)) {
                Ok(_) => pending = true,
                Err(RecvTimeoutError::Timeout) => {
                    if pending {
                        pending = false;
                        let _ = app.emit("sessions-changed", ());
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// The registry files carry an idle/busy flag that changes without any
/// filesystem event we can rely on, so poll it on a slow interval.
fn spawn_live_poller(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut previous: HashMap<String, String> = HashMap::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let current: HashMap<String, String> = registry::live_sessions()
                .unwrap_or_default()
                .into_iter()
                .map(|(id, info)| (id, info.status))
                .collect();
            if current != previous {
                previous = current;
                let _ = app.emit("live-changed", ());
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first. Two copies would each hold their own
        // in-memory settings and the last one to save would win, quietly
        // resurrecting whatever the other had changed — besides fighting over
        // the tray icon and the Run key.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
        ))
        .manage(AppState {
            cache: ScanCache::load(),
            settings: SettingsStore::load(),
        })
        // Handled here rather than in the close button so that every route to
        // closing behaves the same — Alt+F4 and the taskbar included.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if state.settings.get().close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = app.state::<AppState>().settings.get();

            let _ = tray::sync(&handle, settings.close_to_tray);
            reassert_autostart(&handle, settings.start_with_windows);

            // The window starts hidden (see tauri.conf.json) so a login launch
            // never flashes on screen before being sent to the tray.
            let launched_at_login = std::env::args().any(|arg| arg == AUTOSTART_FLAG);
            if !(launched_at_login && settings.close_to_tray) {
                tray::show_main_window(&handle);
            }

            // A window created hidden misses the scale factor of the monitor it
            // will appear on, so on a display at anything but 100% it comes up
            // sized in physical pixels and the layout overflows its own frame.
            // Restating the size in logical units once, after the window exists,
            // puts it back to what tauri.conf.json asks for.
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.set_size(tauri::LogicalSize::new(1080.0, 720.0));
            }

            spawn_watcher(handle.clone());
            spawn_live_poller(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            message_counts,
            list_providers,
            launch_session,
            focus_session,
            delete_session,
            delete_session_permanently,
            undo_delete,
            list_deleted,
            restore_deleted,
            purge_deleted,
            purge_all_deleted,
            get_settings,
            data_location,
            app_info,
            set_settings,
            trust_workspace,
            open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running vastdeck");
}
