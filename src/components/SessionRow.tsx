import type { LaunchMode, Session, WorkspaceStats } from "../lib/api";
import { basename, bytes, cost, fullTime, relativeTime, shortPath } from "../lib/format";
import LaunchButton from "./LaunchButton";
import { FolderIcon, TrashIcon } from "./Icons";

const MODE_BADGE: Record<string, string> = {
  bypassPermissions: "skip perms",
  acceptEdits: "accept edits",
  plan: "plan",
  auto: "auto",
  manual: "manual",
  dontAsk: "dont ask",
};

export default function SessionRow({
  session,
  stats,
  defaultMode,
  showWorkspace,
  onLaunch,
  onFocus,
  onDelete,
  onReveal,
}: {
  session: Session;
  stats?: WorkspaceStats;
  defaultMode: LaunchMode;
  showWorkspace: boolean;
  onLaunch: (mode: LaunchMode) => void;
  onFocus: () => void;
  onDelete: () => void;
  onReveal: () => void;
}) {
  const live = session.live;
  const busy = live?.status === "busy";
  const title = session.title || live?.name || basename(session.workspace);
  const modeBadge = session.permissionMode
    ? MODE_BADGE[session.permissionMode]
    : undefined;
  const price = cost(stats?.costUsd);

  // Modifier-clicks give the two modes worth a shortcut without opening the menu.
  const handleRowClick = (e: React.MouseEvent) => {
    if (e.shiftKey) return onLaunch("skipPermissions");
    if (e.altKey) return onLaunch("fork");
    if (live) return onFocus();
    onLaunch(defaultMode);
  };

  return (
    <div
      onClick={handleRowClick}
      className="group relative flex items-start gap-3 rounded-lg px-3 py-[10px] transition-colors hover:bg-elevated"
    >
      <span className="mt-[6px] grid w-2 shrink-0 place-items-center">
        <span
          className={[
            "block h-[6px] w-[6px] rounded-full",
            live
              ? busy
                ? "vd-pulse bg-live-busy"
                : "bg-live-idle"
              : "border border-line-strong",
          ].join(" ")}
          title={live ? `Running (${live.status}) · pid ${live.pid}` : "Not running"}
        />
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[13px] text-ink">{title}</span>
          {live && (
            <span
              className={`shrink-0 rounded border px-1 py-px text-[9.5px] tracking-wide uppercase ${
                busy
                  ? "border-live-busy/40 text-live-busy"
                  : "border-live-idle/40 text-live-idle"
              }`}
            >
              {live.status}
            </span>
          )}
          {modeBadge && (
            // Deliberately neutral: nearly every session here ran with
            // permissions skipped, so colouring it as a warning would mark
            // almost every row and mean nothing. Amber is reserved for the
            // launch menu, where it flags something about to happen.
            <span className="shrink-0 rounded border border-line-strong px-1 py-px text-[9.5px] tracking-wide text-faint uppercase">
              {modeBadge}
            </span>
          )}
          {session.gitBranch && session.gitBranch !== "HEAD" && (
            <span className="shrink-0 truncate font-mono text-[10px] text-faint">
              {session.gitBranch}
            </span>
          )}
        </div>

        {session.preview && (
          <p className="mt-[3px] truncate text-[12px] text-muted">
            {session.preview}
          </p>
        )}

        <div className="mt-[5px] flex min-w-0 items-center gap-2 text-[11px] text-faint">
          {showWorkspace && (
            <>
              <span
                className="truncate font-mono"
                title={session.workspace}
              >
                {shortPath(session.workspace)}
              </span>
              <Dot />
            </>
          )}
          <span className="shrink-0" title={fullTime(session.updatedAt)}>
            {relativeTime(session.updatedAt)}
          </span>
          {session.messageCount != null && (
            <>
              <Dot />
              <span className="shrink-0 tabular-nums">
                {session.messageCount} msgs
              </span>
            </>
          )}
          <Dot />
          <span className="shrink-0 tabular-nums">{bytes(session.sizeBytes)}</span>
          {/* Cost is recorded per workspace, not per session, so it belongs on
              the group header — repeating it on every row would read as the
              price of that one conversation. */}
          {showWorkspace && price && (
            <>
              <Dot />
              <span
                className="shrink-0 tabular-nums"
                title="Total spend recorded for this workspace"
              >
                {price}
              </span>
            </>
          )}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton label="Open workspace folder" onClick={onReveal}>
          <FolderIcon />
        </IconButton>
        <IconButton label="Delete session" onClick={onDelete} danger>
          <TrashIcon />
        </IconButton>
        <LaunchButton
          primaryLabel={live ? "Focus" : "Resume"}
          primaryAction={() => (live ? onFocus() : onLaunch(defaultMode))}
          onPick={onLaunch}
          live={!!live}
        />
      </div>
    </div>
  );
}

const Dot = () => <span className="shrink-0 text-line-strong">·</span>;

function IconButton({
  children,
  onClick,
  label,
  danger,
}: {
  children: React.ReactNode;
  onClick: () => void;
  label: string;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className={`grid h-[26px] w-[26px] place-items-center rounded-md text-muted transition-colors hover:bg-raised ${
        danger ? "hover:text-danger" : "hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}
