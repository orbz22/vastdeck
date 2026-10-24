Vastdeck 0.3.0 adds Codex CLI and Antigravity CLI beside Claude Code and OpenCode. It also nests workspaces inside their parent folder, makes search match whole words, and fixes two problems with resuming Claude Code sessions.

## In this release

### Codex CLI sessions

- **List:** Codex now has its own sidebar entry. Its threads are grouped by workspace, show the first prompt and the branch, and are marked as running while `codex resume` has them open.
- **Resume modes:**
  - Skip permissions: `--dangerously-bypass-approvals-and-sandbox`
  - Accept edits: `--sandbox workspace-write`
  - Plan: `--sandbox read-only`
  - Fork: `codex fork`
- **Delete and restore:**
  - Delete moves the thread to Codex's own archive.
  - Restore takes it back out with `codex unarchive`.
  - Only a permanent delete, or purging the backup, runs `codex delete`.
  - Vastdeck reads Codex's database read-only and never writes to it.

### Antigravity CLI conversations

- **List:** `agy` conversations are listed the same way, and are marked as running while `agy --conversation` has them open. Antigravity IDE conversations in the same index are left out.
- **Resume modes:**
  - Skip permissions: `--dangerously-skip-permissions`
  - Accept edits: `--mode accept-edits`
  - Plan: `--mode plan`
  - Fork is not offered, because `agy` has no fork.
- **Delete and restore:** `agy` has no delete command. Delete moves the conversation's own files into the backup folder, and restore moves them back. The index `agy` shares with the IDE is never written to.

### Everything else

- **Nested workspaces:** a workspace inside another workspace's folder now shows indented under it, labelled with its relative path.
- **Search matches whole words:**
  - Every word you type has to appear in the title, the prompt, or the folder, branch or id. Before, almost any word matched most sessions.
  - Title matches come first.
  - A typo still finds a title when nothing else matches.
- **Sessions moved with `/cd` resume in the right folder:** they used to open their old directory and fail with "workspace no longer exists".
- **Transcripts are saved again for sessions resumed from Vastdeck:** this only affected a Vastdeck started from inside a Claude Code session, whose resumed sessions showed "Transcript saving is off — inherited CLAUDE_CODE_CHILD_SESSION marker".

## Also worth knowing

The first launch after updating rescans every Claude Code transcript once, because the cached scan now also records where a session moved with `/cd`.

A conversation deleted from Vastdeck stays in Antigravity's own index, so `agy`'s conversation picker may still list it until it is restored.

## Which download

| | |
|---|---|
| **`Vastdeck_0.3.0_x64-setup.exe`** | Installer. No admin rights — installs for your user only, with a Start-menu entry and an uninstaller. Start here. |
| **`Vastdeck_0.3.0_x64_portable.zip`** | One folder, nothing installed. Unzip and run; settings stay in `data\` beside the executable. |
| `Vastdeck_0.3.0_x64_en-US.msi` | For deploying through group policy. Needs admin. |

`latest.json` is the update manifest the app reads. You do not need to download it.

## Known limitations

- Windows only.
- **Session size:** OpenCode sessions show no size. Codex and Antigravity show the size of their stored file, not of the whole conversation.
- **Running marker:** for Codex and Antigravity it covers only sessions opened by resuming them. A session started fresh with `codex` or `agy`, or continued with `agy -c`, is not detected.
- **OpenCode running marker:** an OpenCode session counts as running while a window titled `OC | <title>` is open.
- **The binaries are unsigned** for Windows itself, so SmartScreen will warn the first time: *More info → Run anyway*. The update signature is Vastdeck's own integrity check and is unrelated to Authenticode.
- **Portable copies are not updated in place:** the updater works by running an installer, so it offers the release page instead.
- **Tray icon:** Windows 11 files a new tray icon under the hidden-icons chevron until you drag it onto the taskbar.
