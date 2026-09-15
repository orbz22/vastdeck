import { useEffect, useState } from "react";
import {
  api,
  LAUNCH_MODES,
  type LaunchMode,
  type Settings,
  type TerminalChoice,
} from "../lib/api";
import { CloseIcon, WarnIcon } from "./Icons";

const TERMINALS: { id: TerminalChoice; label: string }[] = [
  { id: "auto", label: "Auto (Windows Terminal → pwsh → powershell → cmd)" },
  { id: "windowsTerminal", label: "Windows Terminal" },
  { id: "pwsh", label: "PowerShell 7 (pwsh)" },
  { id: "powerShell", label: "Windows PowerShell" },
  { id: "cmd", label: "Command Prompt" },
];

export default function SettingsPanel({
  settings,
  onChange,
  onClose,
}: {
  settings: Settings;
  onChange: (next: Settings) => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const [dataDir, setDataDir] = useState<[string, boolean] | null>(null);
  useEffect(() => {
    api.dataLocation().then(setDataDir).catch(() => setDataDir(null));
  }, []);

  const patch = (part: Partial<Settings>) => onChange({ ...settings, ...part });
  const riskyDefault = settings.defaultLaunchMode === "skipPermissions";

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/40" onClick={onClose}>
      <aside
        className="vd-rise flex h-full w-[420px] flex-col border-l border-line-strong bg-surface"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex h-12 shrink-0 items-center justify-between border-b border-line px-4">
          <h2 className="text-[13px] text-ink">Settings</h2>
          <button
            type="button"
            aria-label="Close settings"
            onClick={onClose}
            className="grid h-7 w-7 place-items-center rounded-md text-muted hover:bg-raised hover:text-ink"
          >
            <CloseIcon />
          </button>
        </header>

        <div className="flex-1 overflow-y-auto px-4 py-4">
          <Section title="Terminal">
            <Field label="Open sessions in">
              <Select
                value={settings.terminal}
                options={TERMINALS.map((t) => [t.id, t.label])}
                onChange={(v) => patch({ terminal: v as TerminalChoice })}
              />
            </Field>
            <Field
              label="A session opens as"
              hint="Windows Terminal only; the other terminals always open their own window."
            >
              <Select
                value={settings.wtNewTab ? "tab" : "window"}
                options={[
                  ["tab", "A tab in the current Windows Terminal window"],
                  ["window", "A new Windows Terminal window"],
                ]}
                onChange={(v) => patch({ wtNewTab: v === "tab" })}
              />
            </Field>
            <Field label="Path to claude" hint="Leave empty to find it on PATH.">
              <input
                value={settings.claudePath ?? ""}
                // JSX attribute strings are literal, so a single backslash here
                // is exactly one backslash on screen.
                placeholder="C:\Users\you\.local\bin\claude.exe"
                onChange={(e) => patch({ claudePath: e.target.value || null })}
                className="w-full rounded-md border border-line-strong bg-elevated px-2.5 py-[6px] font-mono text-[11.5px] text-ink outline-none placeholder:text-faint focus:border-focus"
              />
            </Field>
          </Section>

          <Section title="Launching">
            <Field label="What a click does">
              <Select
                value={settings.defaultLaunchMode}
                options={LAUNCH_MODES.map((m) => [m.id, m.label])}
                onChange={(v) => patch({ defaultLaunchMode: v as LaunchMode })}
              />
            </Field>
            {riskyDefault && (
              <Note tone="warn">
                Every plain click will now run with all permission checks
                bypassed, in whichever workspace the row points at.
              </Note>
            )}
            <Note>
              Shift-click a session for skip permissions, Alt-click to fork —
              whatever the default is.
            </Note>
            {settings.trustedWorkspaces.length > 0 && (
              <div className="mt-3 flex items-center justify-between gap-3">
                <span className="text-[12px] text-muted">
                  {settings.trustedWorkspaces.length} workspace
                  {settings.trustedWorkspaces.length === 1 ? "" : "s"} trusted for
                  skip permissions
                </span>
                <button
                  type="button"
                  onClick={() => patch({ trustedWorkspaces: [] })}
                  className="shrink-0 rounded-md border border-line-strong px-2 py-[4px] text-[11.5px] text-muted hover:bg-raised hover:text-ink"
                >
                  Reset
                </button>
              </div>
            )}
          </Section>

          <Section title="Deleting">
            <Toggle
              label="Delete permanently"
              hint="Off: sessions move to ~/.claude/backups/vastdeck and can be undone."
              checked={settings.hardDelete}
              onChange={(v) => patch({ hardDelete: v })}
            />
            {settings.hardDelete && (
              <Note tone="warn">
                Transcripts will be removed outright. There is no undo and no
                other copy of a conversation.
              </Note>
            )}
          </Section>

          <Section title="System">
            <Toggle
              label="Close to the system tray"
              hint="Closing hides the window instead of quitting. Click the tray icon to bring it back, or quit from its menu."
              checked={settings.closeToTray}
              onChange={(v) => patch({ closeToTray: v })}
            />
            <Toggle
              label="Start with Windows"
              hint="Adds Vastdeck to your account's startup entries."
              checked={settings.startWithWindows}
              onChange={(v) => patch({ startWithWindows: v })}
            />
            {dataDir && (
              <Field
                label={dataDir[1] ? "Data folder (portable)" : "Data folder"}
                hint={
                  dataDir[1]
                    ? "A portable.txt sits next to the executable, so everything stays in one folder."
                    : "Put a portable.txt next to the executable to keep everything beside it instead."
                }
              >
                <p className="truncate rounded-md border border-line bg-elevated px-2.5 py-[6px] font-mono text-[11px] text-muted" title={dataDir[0]}>
                  {dataDir[0]}
                </p>
              </Field>
            )}
            {settings.startWithWindows && !settings.closeToTray && (
              <Note>
                Vastdeck will open its window at every login. Turn on close to
                tray as well and it starts quietly in the notification area
                instead.
              </Note>
            )}
          </Section>

          <Section title="Appearance">
            <Field label="Theme">
              <Select
                value={settings.theme}
                options={[
                  ["system", "Follow system"],
                  ["dark", "Dark"],
                  ["light", "Light"],
                ]}
                onChange={(v) => patch({ theme: v })}
              />
            </Field>
            <Toggle
              label="Group sessions by workspace"
              checked={settings.groupByWorkspace}
              onChange={(v) => patch({ groupByWorkspace: v })}
            />
            <Field label="Sort">
              <Select
                value={settings.sort}
                options={[
                  ["recent", "Most recent"],
                  ["name", "Name"],
                  ["size", "Size"],
                ]}
                onChange={(v) => patch({ sort: v })}
              />
            </Field>
          </Section>
        </div>
      </aside>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-7">
      <h3 className="mb-3 text-[10px] font-semibold tracking-[0.12em] text-faint uppercase">
        {title}
      </h3>
      <div className="flex flex-col gap-3">{children}</div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[12px] text-ink">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-[11px] text-faint">{hint}</span>}
    </label>
  );
}

function Select({
  value,
  options,
  onChange,
}: {
  value: string;
  options: [string, string][];
  onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full rounded-md border border-line-strong bg-elevated px-2.5 py-[6px] text-[12px] text-ink outline-none focus:border-focus"
    >
      {options.map(([id, label]) => (
        <option key={id} value={id}>
          {label}
        </option>
      ))}
    </select>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="flex w-full items-start gap-3 text-left"
    >
      <span
        className={`mt-[2px] flex h-[18px] w-[30px] shrink-0 items-center rounded-full border transition-colors ${
          checked ? "border-transparent bg-invert" : "border-line-strong bg-elevated"
        }`}
      >
        <span
          className={`block h-[12px] w-[12px] rounded-full transition-transform ${
            checked ? "translate-x-[15px] bg-invert-ink" : "translate-x-[3px] bg-faint"
          }`}
        />
      </span>
      <span className="min-w-0">
        <span className="block text-[12px] text-ink">{label}</span>
        {hint && <span className="block text-[11px] text-faint">{hint}</span>}
      </span>
    </button>
  );
}

function Note({
  children,
  tone,
}: {
  children: React.ReactNode;
  tone?: "warn";
}) {
  const warn = tone === "warn";
  return (
    <p
      className={`flex gap-2 rounded-md border px-2.5 py-2 text-[11.5px] leading-relaxed ${
        warn
          ? "border-warn/30 bg-warn-soft text-warn"
          : "border-line bg-elevated text-muted"
      }`}
    >
      {warn && <WarnIcon className="mt-[2px] shrink-0" size={12} />}
      <span>{children}</span>
    </p>
  );
}
