import { useEffect, useRef, useState } from "react";
import { LAUNCH_MODES, type LaunchMode } from "../lib/api";
import { ChevronDown, WarnIcon } from "./Icons";

/**
 * Split button: the left half runs the remembered mode, the chevron opens the
 * full list. Skip-permissions is deliberately never the left half's default —
 * it has to be chosen, here or in Settings.
 */
export default function LaunchButton({
  primaryLabel,
  primaryAction,
  onPick,
  live,
}: {
  primaryLabel: string;
  primaryAction: () => void;
  onPick: (mode: LaunchMode) => void;
  live: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative flex shrink-0">
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          primaryAction();
        }}
        className="rounded-l-md border border-r-0 border-line-strong bg-elevated px-2.5 py-[5px] text-[12px] text-ink transition-colors hover:bg-raised"
      >
        {primaryLabel}
      </button>
      <button
        type="button"
        aria-label="More launch modes"
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        className="grid w-6 place-items-center rounded-r-md border border-line-strong bg-elevated text-muted transition-colors hover:bg-raised hover:text-ink"
      >
        <ChevronDown />
      </button>

      {open && (
        <div
          className="vd-rise absolute top-full right-0 z-30 mt-1 w-[248px] overflow-hidden rounded-lg border border-line-strong bg-elevated py-1 shadow-[0_12px_32px_rgba(0,0,0,0.38)]"
          onClick={(e) => e.stopPropagation()}
        >
          {live && (
            <p className="border-b border-line px-3 py-2 text-[11px] leading-snug text-muted">
              This session is already running. Resuming it again starts a second
              process on the same transcript.
            </p>
          )}
          {LAUNCH_MODES.map((mode) => (
            <button
              key={mode.id}
              type="button"
              onClick={() => {
                setOpen(false);
                onPick(mode.id);
              }}
              className="flex w-full items-start gap-2 px-3 py-[7px] text-left transition-colors hover:bg-raised"
            >
              {mode.danger && (
                <WarnIcon className="mt-[3px] shrink-0 text-warn" size={12} />
              )}
              <span className={mode.danger ? "" : "pl-[20px]"}>
                <span
                  className={`block text-[12px] ${mode.danger ? "text-warn" : "text-ink"}`}
                >
                  {mode.label}
                </span>
                <span className="block font-mono text-[10px] text-faint">
                  {mode.hint}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
