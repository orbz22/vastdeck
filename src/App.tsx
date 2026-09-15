import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  modeLabel,
  type DeletedHandle,
  type DeletedSession,
  type LaunchMode,
  type ProviderInfo,
  type Session,
  type Settings,
  type WorkspaceStats,
} from "./lib/api";
import {
  basename,
  cost,
  fuzzyMatch,
  normalizeWorkspace,
  shortPath,
} from "./lib/format";
import TitleBar from "./components/TitleBar";
import Sidebar, { type View } from "./components/Sidebar";
import DeletedList from "./components/DeletedList";
import SessionRow from "./components/SessionRow";
import SettingsPanel from "./components/SettingsPanel";
import Toast, { type ToastState } from "./components/Toast";
import ConfirmDialog, { type ConfirmRequest } from "./components/ConfirmDialog";
import { SearchIcon } from "./components/Icons";

export default function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [stats, setStats] = useState<Record<string, WorkspaceStats>>({});
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [provider, setProvider] = useState("claude-code");
  const [view, setView] = useState<View>("sessions");
  const [deleted, setDeleted] = useState<DeletedSession[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [toast, setToast] = useState<ToastState | null>(null);
  const [confirm, setConfirm] = useState<ConfirmRequest | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const notify = useCallback((next: Omit<ToastState, "id">) => {
    setToast({ ...next, id: Date.now() });
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [list, provs] = await Promise.all([
        api.listSessions(),
        api.listProviders(),
      ]);
      setSessions(list.sessions);
      setStats(list.stats);
      setProviders(provs);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshDeleted = useCallback(async () => {
    try {
      setDeleted(await api.listDeleted());
    } catch {
      setDeleted([]);
    }
  }, []);

  useEffect(() => {
    api.getSettings().then(setSettings).catch(() => setSettings(null));
    refresh();
    refreshDeleted();
  }, [refresh, refreshDeleted]);

  const restoreDeleted = useCallback(
    async (item: DeletedSession) => {
      try {
        await api.restoreDeleted(item.backupDir);
        notify({ message: `Restored — ${item.title ?? basename(item.workspace)}` });
        refreshDeleted();
        refresh();
      } catch (e) {
        notify({ message: String(e), tone: "danger" });
      }
    },
    [notify, refresh, refreshDeleted],
  );

  const purgeDeleted = useCallback(
    (item: DeletedSession) => {
      setConfirm({
        title: "Delete forever?",
        body: "This removes the backup for good. There is no other copy of the conversation.",
        detail: item.title ?? basename(item.workspace),
        confirmLabel: "Delete forever",
        tone: "danger",
        onConfirm: async () => {
          try {
            await api.purgeDeleted(item.backupDir);
            refreshDeleted();
          } catch (e) {
            notify({ message: String(e), tone: "danger" });
          }
        },
      });
    },
    [notify, refreshDeleted],
  );

  const emptyTrash = useCallback(() => {
    setConfirm({
      title: "Empty the trash?",
      body: `All ${deleted.length} backed-up session${deleted.length === 1 ? "" : "s"} will be removed for good.`,
      confirmLabel: "Delete all",
      tone: "danger",
      onConfirm: async () => {
        try {
          const removed = await api.purgeAllDeleted();
          notify({ message: `Removed ${removed} backup${removed === 1 ? "" : "s"}` });
          refreshDeleted();
        } catch (e) {
          notify({ message: String(e), tone: "danger" });
        }
      },
    });
  }, [deleted.length, notify, refreshDeleted]);

  // Message counts need a full read of every transcript, so they arrive after
  // the list is already interactive and fill in place.
  useEffect(() => {
    if (loading) return;
    let cancelled = false;
    api
      .messageCounts()
      .then((counts) => {
        if (cancelled) return;
        setSessions((prev) =>
          prev.map((s) =>
            counts[s.id] != null ? { ...s, messageCount: counts[s.id] } : s,
          ),
        );
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [loading]);

  useEffect(() => {
    const unlisten = Promise.all([
      listen("sessions-changed", () => refresh()),
      listen("live-changed", () => refresh()),
    ]);
    return () => {
      unlisten.then((fns) => fns.forEach((fn) => fn()));
    };
  }, [refresh]);

  // "system" is resolved here rather than in CSS so there is exactly one set of
  // light tokens to keep in sync.
  useEffect(() => {
    const choice = settings?.theme ?? "system";
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => {
      const resolved = choice === "system" ? (media.matches ? "light" : "dark") : choice;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    if (choice !== "system") return;
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings?.theme]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
      if (e.key === "Escape" && document.activeElement === searchRef.current) {
        setQuery("");
        searchRef.current?.blur();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const persist = useCallback(
    (next: Settings) => {
      setSettings(next);
      // Saving also writes the Run key and builds or removes the tray icon, so a
      // failure here is something the user needs to see rather than a no-op.
      api.setSettings(next).catch((e) => {
        notify({ message: `Could not save settings: ${e}`, tone: "danger" });
      });
    },
    [notify],
  );

  const visible = useMemo(() => {
    const filtered = sessions
      .filter((s) => s.provider === provider)
      .filter((s) =>
        !query
          ? true
          : fuzzyMatch(query, `${s.title ?? ""} ${s.preview ?? ""} ${s.workspace}`),
      );
    const sort = settings?.sort ?? "recent";
    return [...filtered].sort((a, b) => {
      if (sort === "size") return b.sizeBytes - a.sizeBytes;
      if (sort === "name") {
        const an = a.title ?? basename(a.workspace);
        const bn = b.title ?? basename(b.workspace);
        return an.localeCompare(bn);
      }
      return b.updatedAt - a.updatedAt;
    });
  }, [sessions, provider, query, settings?.sort]);

  const groups = useMemo(() => {
    if (!settings?.groupByWorkspace) return null;
    const map = new Map<string, Session[]>();
    for (const session of visible) {
      const list = map.get(session.workspace);
      if (list) list.push(session);
      else map.set(session.workspace, [session]);
    }
    return [...map.entries()].sort(
      (a, b) =>
        Math.max(...b[1].map((s) => s.updatedAt)) -
        Math.max(...a[1].map((s) => s.updatedAt)),
    );
  }, [visible, settings?.groupByWorkspace]);

  const runLaunch = useCallback(
    async (session: Session, mode: LaunchMode) => {
      try {
        await api.launch(session, mode);
        notify({ message: `${modeLabel(mode)} — ${basename(session.workspace)}` });
        if (settings) {
          setSettings({
            ...settings,
            sessionModes: { ...settings.sessionModes, [session.id]: mode },
          });
        }
      } catch (e) {
        notify({ message: String(e), tone: "danger" });
      }
    },
    [notify, settings],
  );

  /** Skip-permissions is confirmed once per workspace, then remembered. */
  const launch = useCallback(
    (session: Session, mode: LaunchMode) => {
      const trusted = settings?.trustedWorkspaces ?? [];
      const needsConsent =
        mode === "skipPermissions" &&
        !trusted.includes(normalizeWorkspace(session.workspace));

      if (!needsConsent) {
        runLaunch(session, mode);
        return;
      }
      setConfirm({
        title: "Run without permission checks?",
        body:
          "Claude will edit files and run commands in this workspace without asking. Vastdeck will not ask again for this workspace.",
        detail: session.workspace,
        confirmLabel: "Run anyway",
        tone: "warn",
        onConfirm: () => {
          // The backend owns the trusted list. Re-reading it instead of
          // persisting a locally patched copy avoids a stale `settings` closure
          // — captured when the dialog opened — writing the whole object back
          // and dropping whatever was trusted in between.
          api
            .trustWorkspace(session.workspace)
            .then(() => api.getSettings())
            .then(setSettings)
            .catch(() => {});
          runLaunch(session, mode);
        },
      });
    },
    [persist, runLaunch, settings],
  );

  const focusSession = useCallback(
    async (session: Session) => {
      if (!session.live) return;
      const raised = await api.focus(session.live.pid).catch(() => false);
      if (!raised) {
        notify({
          message: "Could not find that terminal window — it may be on another desktop.",
          tone: "danger",
        });
      }
    },
    [notify],
  );

  const removeSession = useCallback(
    (session: Session) => {
      const permanent = settings?.hardDelete ?? false;
      setConfirm({
        title: permanent ? "Delete permanently?" : "Delete session?",
        body: permanent
          ? "The transcript will be removed outright. There is no other copy and no undo."
          : "The transcript moves to ~/.claude/backups/vastdeck and can be restored.",
        detail: session.title ?? basename(session.workspace),
        confirmLabel: permanent ? "Delete forever" : "Delete",
        tone: permanent ? "danger" : "neutral",
        onConfirm: async () => {
          try {
            const handle: DeletedHandle = await api.deleteSession(session);
            setSessions((prev) => prev.filter((s) => s.id !== session.id));
            refreshDeleted();
            notify({
              message: permanent ? "Deleted permanently" : "Session deleted",
              action: permanent
                ? undefined
                : {
                    label: "Undo",
                    run: async () => {
                      try {
                        await api.undoDelete(handle);
                        refresh();
                        refreshDeleted();
                      } catch (e) {
                        notify({ message: String(e), tone: "danger" });
                      }
                    },
                  },
            });
          } catch (e) {
            notify({ message: String(e), tone: "danger" });
          }
        },
      });
    },
    [notify, refresh, refreshDeleted, settings?.hardDelete],
  );

  const defaultModeFor = (session: Session): LaunchMode =>
    settings?.sessionModes?.[session.id] ?? settings?.defaultLaunchMode ?? "normal";

  const renderRow = (session: Session, showWorkspace: boolean) => (
    <SessionRow
      key={session.id}
      session={session}
      stats={stats[normalizeWorkspace(session.workspace)]}
      defaultMode={defaultModeFor(session)}
      showWorkspace={showWorkspace}
      onLaunch={(mode) => launch(session, mode)}
      onFocus={() => focusSession(session)}
      onDelete={() => removeSession(session)}
      onReveal={() => api.openPath(session.workspace).catch(() => {})}
    />
  );

  const liveCount = visible.filter((s) => s.live).length;

  return (
    <div className="flex h-full flex-col bg-bg">
      <TitleBar closeToTray={settings?.closeToTray ?? false} />

      <div className="flex min-h-0 flex-1">
        <Sidebar
          providers={providers}
          selected={provider}
          view={view}
          deletedCount={deleted.length}
          onSelectProvider={setProvider}
          onSelectView={setView}
          onOpenSettings={() => setShowSettings(true)}
        />

        <main className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-12 shrink-0 items-center justify-between gap-4 border-b border-line px-4">
            <div className="flex items-baseline gap-2.5">
              <h1 className="text-[13px] text-ink">
                {view === "deleted" ? "Deleted" : "Sessions"}
              </h1>
              <span className="text-[11.5px] tabular-nums text-faint">
                {view === "deleted" ? deleted.length : visible.length}
                {view === "sessions" && liveCount > 0 && ` · ${liveCount} running`}
              </span>
            </div>

            {view === "deleted" ? (
              deleted.length > 0 && (
                <button
                  type="button"
                  onClick={emptyTrash}
                  className="rounded-md border border-line-strong px-2.5 py-[5px] text-[12px] text-muted transition-colors hover:bg-raised hover:text-danger"
                >
                  Empty trash
                </button>
              )
            ) : (
            <div className="relative w-[280px]">
              <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
              <input
                ref={searchRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search sessions"
                spellCheck={false}
                className="w-full rounded-md border border-line bg-elevated py-[6px] pr-12 pl-8 text-[12px] text-ink outline-none placeholder:text-faint focus:border-focus"
              />
              <kbd className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 rounded border border-line-strong px-1 py-px font-mono text-[9.5px] text-faint">
                Ctrl K
              </kbd>
            </div>
            )}
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
            {view === "deleted" ? (
              deleted.length === 0 ? (
                <Empty
                  title="Nothing deleted"
                  body="Sessions you delete land here first and can be put back, unless permanent delete is switched on in Settings."
                />
              ) : (
                <DeletedList
                  items={deleted}
                  onRestore={restoreDeleted}
                  onPurge={purgeDeleted}
                />
              )
            ) : loading ? (
              <Skeleton />
            ) : error ? (
              <Empty title="Could not read your sessions" body={error} />
            ) : visible.length === 0 ? (
              query ? (
                <Empty
                  title="Nothing matches"
                  body={`No session matches "${query}".`}
                />
              ) : (
                <Empty
                  title="No sessions yet"
                  body="Run claude in any workspace and it will show up here."
                />
              )
            ) : groups ? (
              groups.map(([workspace, items]) => {
                const spend = cost(stats[normalizeWorkspace(workspace)]?.costUsd);
                return (
                  <section key={workspace} className="mb-3">
                    <div className="flex items-baseline gap-2 px-3 pt-2 pb-1">
                      <h2 className="truncate text-[12px] text-ink" title={workspace}>
                        {basename(workspace)}
                      </h2>
                      <span
                        className="truncate font-mono text-[10.5px] text-faint"
                        title={workspace}
                      >
                        {shortPath(workspace, 3)}
                      </span>
                      <span className="ml-auto flex shrink-0 items-baseline gap-2 text-[11px] tabular-nums text-faint">
                        {spend && (
                          <span title="Total spend recorded for this workspace">
                            {spend}
                          </span>
                        )}
                        <span>{items.length}</span>
                      </span>
                    </div>
                    {items.map((s) => renderRow(s, false))}
                  </section>
                );
              })
            ) : (
              visible.map((s) => renderRow(s, true))
            )}
          </div>
        </main>
      </div>

      {showSettings && settings && (
        <SettingsPanel
          settings={settings}
          onChange={persist}
          onClose={() => setShowSettings(false)}
        />
      )}
      <ConfirmDialog request={confirm} onClose={() => setConfirm(null)} />
      <Toast toast={toast} onDismiss={() => setToast(null)} />
    </div>
  );
}

function Empty({ title, body }: { title: string; body: string }) {
  return (
    <div className="grid h-full place-items-center px-8 text-center">
      <div className="max-w-[360px]">
        <p className="text-[13px] text-ink">{title}</p>
        <p className="mt-1.5 text-[12px] leading-relaxed text-muted">{body}</p>
      </div>
    </div>
  );
}

function Skeleton() {
  return (
    <div className="flex flex-col gap-1 px-1 pt-1">
      {Array.from({ length: 7 }).map((_, i) => (
        <div
          key={i}
          className="flex items-start gap-3 rounded-lg px-3 py-[10px]"
          style={{ opacity: 1 - i * 0.12 }}
        >
          <span className="mt-[6px] h-[6px] w-[6px] shrink-0 rounded-full bg-raised" />
          <div className="flex-1">
            <div className="h-[11px] w-[38%] rounded bg-raised" />
            <div className="mt-2 h-[10px] w-[62%] rounded bg-elevated" />
            <div className="mt-2 h-[9px] w-[26%] rounded bg-elevated" />
          </div>
        </div>
      ))}
    </div>
  );
}
