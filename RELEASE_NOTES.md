Vastdeck 0.2.2 adds OpenCode as a second CLI beside Claude Code, and makes Focus bring up the terminal tab that actually runs the session you clicked.

## In this release

- **OpenCode sessions.** OpenCode now has its own entry in the sidebar, with
  every session grouped by workspace, its last prompt and message count, and a
  running marker while its terminal is open. Resume, Focus and delete work as
  they do for Claude Code. The launch menu offers the modes OpenCode has:
  `--auto`, `--agent plan` and `--fork`. OpenCode has no accept-edits mode, so
  that entry is not offered for its sessions.
- **OpenCode data is only ever read.** Vastdeck opens OpenCode's database
  read-only. A delete first saves `opencode export` output to the same backup
  folder Claude Code sessions use, then runs `opencode session delete`.
  Restore runs `opencode import` from the session's own workspace. Every change
  goes through OpenCode's own CLI.
- **Focus finds the right tab.** Windows Terminal runs every tab under one
  process, so Focus used to raise whichever tab sat on top — often another
  session's. It now picks the tab by its title, and still finds it after a
  `/rename` or when a long name is cut off with `…` in the tab.
- **`/rename` names show in the list.** A session renamed with `/rename` kept
  its generated title in Vastdeck until it was opened again. The name you gave
  it now wins as soon as the transcript records it.
- **The window shows after an update.** On a machine that starts Vastdeck with
  Windows, the installer restarted the updated app with the login flag, and the
  window went straight back to the tray. An update restart now always shows
  the window.

## Also worth knowing

The update-restart fix lives in the app that runs the update, so it takes
effect from the next update on. Updating from 0.2.1 to 0.2.2 on a machine with
start-with-Windows on may still leave the window in the tray once; open it from
the tray icon.

The first launch after updating rescans every transcript once, because the
cached scan now records the `/rename` name too.

## Which download

| | |
|---|---|
| **`Vastdeck_0.2.2_x64-setup.exe`** | Installer. No admin rights — installs for your user only, with a Start-menu entry and an uninstaller. Start here. |
| **`Vastdeck_0.2.2_x64_portable.zip`** | One folder, nothing installed. Unzip and run; settings stay in `data\` beside the executable. |
| `Vastdeck_0.2.2_x64_en-US.msi` | For deploying through group policy. Needs admin. |

`latest.json` is the update manifest the app reads. You do not need to download it.

## Known limitations

- Windows only. Claude Code and OpenCode are wired up; Codex and Antigravity
  are not yet.
- OpenCode sessions show no size: OpenCode keeps all sessions in one database,
  and summing one session's rows takes seconds.
- An OpenCode session counts as running while a window titled `OC | <title>`
  is open. OpenCode records no process per session, so a terminal that has
  lost that title is not detected.
- **The binaries are unsigned** for Windows itself, so SmartScreen will warn the
  first time: *More info → Run anyway*. The update signature is Vastdeck's own
  integrity check and is unrelated to Authenticode.
- Portable copies are not updated in place — the updater works by running an
  installer, so it offers the release page instead.
- Windows 11 files a new tray icon under the hidden-icons chevron until you drag
  it onto the taskbar.
