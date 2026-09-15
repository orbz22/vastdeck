import type { ProviderInfo } from "../lib/api";
import { GearIcon, TrashIcon } from "./Icons";

export type View = "sessions" | "deleted";

export default function Sidebar({
  providers,
  selected,
  view,
  deletedCount,
  onSelectProvider,
  onSelectView,
  onOpenSettings,
}: {
  providers: ProviderInfo[];
  selected: string;
  view: View;
  deletedCount: number;
  onSelectProvider: (id: string) => void;
  onSelectView: (view: View) => void;
  onOpenSettings: () => void;
}) {
  return (
    <nav className="flex w-[196px] shrink-0 flex-col border-r border-line bg-surface">
      <div className="px-3 pt-4 pb-2">
        <span className="px-2 text-[10px] font-semibold tracking-[0.12em] text-faint uppercase">
          CLI
        </span>
      </div>

      <div className="flex flex-col gap-0.5 px-2">
        {providers.map((provider) => {
          const active = view === "sessions" && provider.id === selected;
          const available = provider.installed;
          return (
            <button
              key={provider.id}
              type="button"
              disabled={!available}
              onClick={() => {
                onSelectView("sessions");
                onSelectProvider(provider.id);
              }}
              className={[
                "flex items-center justify-between rounded-md px-2.5 py-[7px] text-left transition-colors",
                active
                  ? "bg-raised text-ink"
                  : available
                    ? "text-muted hover:bg-elevated hover:text-ink"
                    : "cursor-default text-faint",
              ].join(" ")}
            >
              <span className="truncate text-[13px]">{provider.name}</span>
              {available ? (
                <span className="ml-2 text-[11px] tabular-nums text-faint">
                  {provider.sessionCount}
                </span>
              ) : (
                <span className="ml-2 rounded border border-line px-1 text-[9px] tracking-wide text-faint uppercase">
                  soon
                </span>
              )}
            </button>
          );
        })}
      </div>

      <div className="mt-auto flex flex-col gap-0.5 p-2">
        <button
          type="button"
          onClick={() => onSelectView("deleted")}
          className={[
            "flex items-center justify-between rounded-md px-2.5 py-[7px] text-[13px] transition-colors",
            view === "deleted"
              ? "bg-raised text-ink"
              : "text-muted hover:bg-elevated hover:text-ink",
          ].join(" ")}
        >
          <span className="flex items-center gap-2">
            <TrashIcon />
            Deleted
          </span>
          {deletedCount > 0 && (
            <span className="ml-2 text-[11px] tabular-nums text-faint">
              {deletedCount}
            </span>
          )}
        </button>

        <button
          type="button"
          onClick={onOpenSettings}
          className="flex w-full items-center gap-2 rounded-md px-2.5 py-[7px] text-[13px] text-muted transition-colors hover:bg-elevated hover:text-ink"
        >
          <GearIcon />
          Settings
        </button>
      </div>
    </nav>
  );
}
