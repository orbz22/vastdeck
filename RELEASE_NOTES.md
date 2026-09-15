Vastdeck can now update itself. Settings → System shows the version you are on and checks GitHub for a newer one; installed copies download and apply it in place.

## In-app updates

The **Check** button in Settings reads the release manifest from this repository
and, when there is a newer version, downloads the installer, verifies it against
a signing key baked into the build, and runs it. Nothing installs unless that
signature matches, so a tampered download is refused.

Two things worth knowing:

- **Portable copies are not updated in place.** The updater works by running an
  installer, and a portable folder has nothing to install into — so it offers
  the release page instead of an Update button.
- **0.1.0 cannot update itself to this release.** It shipped before the updater
  existed and has no way to check. Install 0.2.0 by hand once and every release
  after this one is a button.

## Which download

| | |
|---|---|
| **`Vastdeck_0.2.0_x64-setup.exe`** | Installer. No admin rights — installs for your user only, with a Start-menu entry and an uninstaller. Start here. |
| **`Vastdeck_0.2.0_x64_portable.zip`** | One folder, nothing installed. Unzip and run; settings stay in `data\` beside the executable. |
| `Vastdeck_0.2.0_x64_en-US.msi` | For deploying through group policy. Needs admin. |

`latest.json` is the update manifest the app reads. You do not need to download it.

## Also in this release

- **Deleted view.** Soft-deleted sessions used to be reachable only through the
  ten-second undo toast; after that the backup was an orphan, and nothing
  recorded where the transcript came from, so it could not have been put back.
  Backups now carry a manifest, and the sidebar has a **Deleted** view that
  restores or purges them.
- **Close to the system tray**, and **Start with Windows**. The Run key is
  rewritten on every start, so moving a portable copy no longer leaves Windows
  pointing at a file that is not there.
- **True portable mode.** A `portable.txt` beside the executable keeps settings,
  cache and launch scripts in `data\` next to it rather than in `%LOCALAPPDATA%`.
- **Only one copy runs at a time.** Two instances each held their own settings in
  memory and the last one to save won, quietly undoing the other's changes.
- Windows Terminal tab-or-new-window is now an explicit choice rather than a
  toggle whose label did not say what it did.

## Known limitations

- Windows only, and Claude Code is the only provider wired up so far.
- **The binaries are unsigned** for Windows itself, so SmartScreen will warn the
  first time: *More info → Run anyway*. The update signature above is Vastdeck's
  own integrity check and is unrelated to Authenticode.
- Windows 11 files a new tray icon under the hidden-icons chevron until you drag
  it onto the taskbar.
