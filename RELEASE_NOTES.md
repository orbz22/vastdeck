Vastdeck 0.2.1 makes the window behave like an app rather than a web page: each CLI now carries its own mark in the sidebar, and the browser's right-click menu is gone from everywhere it did not belong.

## In this release

- **CLI marks in the sidebar.** Claude Code, Codex and Antigravity each show
  their own logo beside the name, so the list reads at a glance instead of by
  reading. The marks are inlined SVG paths — nothing is fetched at runtime.
- **No more browser context menu.** Right-clicking anywhere used to offer
  Reload, Save as, Print and Inspect: a browser's menu, surfaced in a window
  that is not a browser and cannot act on any of it. Right-click now does
  nothing, except in the search field, which opens a small native Copy/Paste
  menu.
- **Correct window size on scaled displays.** The window is created hidden so
  an autostart launch can stay in the tray, and a hidden window has no monitor
  to take its scale factor from — on a display at 125% or 150% it came up sized
  in physical pixels and the layout overflowed its own frame. The size is now
  restated in logical units once the window exists.

## Also worth knowing

The README claimed a `CliProvider` trait and no network calls at all. Neither
was true: the provider list is still hardcoded to Claude Code, and the update
check in Settings → System reaches GitHub when you press **Check**. Both are
now described as they actually are.

## Which download

| | |
|---|---|
| **`Vastdeck_0.2.1_x64-setup.exe`** | Installer. No admin rights — installs for your user only, with a Start-menu entry and an uninstaller. Start here. |
| **`Vastdeck_0.2.1_x64_portable.zip`** | One folder, nothing installed. Unzip and run; settings stay in `data\` beside the executable. |
| `Vastdeck_0.2.1_x64_en-US.msi` | For deploying through group policy. Needs admin. |

`latest.json` is the update manifest the app reads. You do not need to download it.

## Known limitations

- Windows only, and Claude Code is the only provider wired up so far.
- **The binaries are unsigned** for Windows itself, so SmartScreen will warn the
  first time: *More info → Run anyway*. The update signature is Vastdeck's own
  integrity check and is unrelated to Authenticode.
- Portable copies are not updated in place — the updater works by running an
  installer, so it offers the release page instead.
- Windows 11 files a new tray icon under the hidden-icons chevron until you drag
  it onto the taskbar.
