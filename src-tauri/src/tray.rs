//! Notification-area icon.
//!
//! The icon only exists while "minimize to tray" is on — an always-present tray
//! icon for a feature nobody enabled is just clutter — so it is built and torn
//! down as the setting changes.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "vastdeck-tray";

/// Brings the window back from the tray, from a minimized state, or from behind
/// whatever is in front of it.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Creates or removes the tray icon to match `enabled`.
///
/// Must run on the thread that owns the event loop. The Win32 call behind
/// removing an icon is silently ignored from a worker thread, which leaves a
/// dead icon sitting in the notification area until the process exits — so
/// callers off the main thread go through [`sync_on_main_thread`].
pub fn sync(app: &AppHandle, enabled: bool) -> anyhow::Result<()> {
    match (enabled, app.tray_by_id(TRAY_ID)) {
        (true, None) => build(app)?,
        (false, Some(icon)) => {
            // Hiding first, then dropping the handle: the drop alone proved
            // unreliable in practice.
            let _ = icon.set_visible(false);
            drop(icon);
            app.remove_tray_by_id(TRAY_ID);
        }
        _ => {}
    }
    Ok(())
}

/// [`sync`], hopped onto the event-loop thread. Use this from Tauri commands.
pub fn sync_on_main_thread(app: &AppHandle, enabled: bool) -> anyhow::Result<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sync(&handle, enabled);
    })?;
    Ok(())
}

fn build(app: &AppHandle) -> anyhow::Result<()> {
    // Left-clicking the icon already restores the window, so the menu carries
    // only the thing that has no other route: actually quitting.
    let quit = MenuItem::with_id(app, "quit", "Quit Vastdeck", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Vastdeck")
        .menu(&menu)
        // Left click restores the window; the menu belongs on right click, which
        // is what people expect of a tray icon on Windows.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
