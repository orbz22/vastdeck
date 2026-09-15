import type { DeletedSession } from "../lib/api";
import { basename, bytes, fullTime, relativeTime, shortPath } from "../lib/format";
import { TrashIcon, UndoIcon, WarnIcon } from "./Icons";

/**
 * Everything soft-deleted and still sitting in
 * `~/.claude/backups/vastdeck/`. Without this view a delete was only reversible
 * for as long as the undo toast was on screen.
 */
export default function DeletedList({
  items,
  onRestore,
  onPurge,
}: {
  items: DeletedSession[];
  onRestore: (item: DeletedSession) => void;
  onPurge: (item: DeletedSession) => void;
}) {
  return (
    <>
      {items.map((item) => (
        <div
          key={item.backupDir}
          className="group flex items-start gap-3 rounded-lg px-3 py-[10px] transition-colors hover:bg-elevated"
        >
          <span className="mt-[6px] grid w-2 shrink-0 place-items-center">
            <span className="block h-[6px] w-[6px] rounded-full border border-line-strong" />
          </span>

          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 items-center gap-2">
              <span className="truncate text-[13px] text-ink">
                {item.title || basename(item.workspace) || item.sessionId}
              </span>
              {!item.restorable && (
                <span
                  className="flex shrink-0 items-center gap-1 rounded border border-warn/35 px-1 py-px text-[9.5px] tracking-wide text-warn uppercase"
                  title="No manifest in this backup, so its original location is unknown"
                >
                  <WarnIcon size={10} />
                  no origin
                </span>
              )}
            </div>

            {item.preview && (
              <p className="mt-[3px] truncate text-[12px] text-muted">
                {item.preview}
              </p>
            )}

            <div className="mt-[5px] flex min-w-0 items-center gap-2 text-[11px] text-faint">
              {item.workspace && (
                <>
                  <span className="truncate font-mono" title={item.workspace}>
                    {shortPath(item.workspace)}
                  </span>
                  <span className="shrink-0 text-line-strong">·</span>
                </>
              )}
              <span className="shrink-0" title={fullTime(item.deletedAt)}>
                deleted {relativeTime(item.deletedAt)}
              </span>
              <span className="shrink-0 text-line-strong">·</span>
              <span className="shrink-0 tabular-nums">{bytes(item.sizeBytes)}</span>
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
            <button
              type="button"
              aria-label="Delete forever"
              title="Delete forever"
              onClick={() => onPurge(item)}
              className="grid h-[26px] w-[26px] place-items-center rounded-md text-muted transition-colors hover:bg-raised hover:text-danger"
            >
              <TrashIcon />
            </button>
            <button
              type="button"
              disabled={!item.restorable}
              onClick={() => onRestore(item)}
              title={
                item.restorable
                  ? "Put this session back where it came from"
                  : "This backup has no manifest, so its original location is unknown"
              }
              className="flex items-center gap-1.5 rounded-md border border-line-strong bg-elevated px-2.5 py-[5px] text-[12px] text-ink transition-colors hover:bg-raised disabled:cursor-default disabled:text-faint disabled:hover:bg-elevated"
            >
              <UndoIcon size={12} />
              Restore
            </button>
          </div>
        </div>
      ))}
    </>
  );
}
