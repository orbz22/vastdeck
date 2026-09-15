import { useEffect } from "react";
import { UndoIcon } from "./Icons";

export interface ToastState {
  id: number;
  message: string;
  tone?: "default" | "danger";
  action?: { label: string; run: () => void };
}

export default function Toast({
  toast,
  onDismiss,
}: {
  toast: ToastState | null;
  onDismiss: () => void;
}) {
  useEffect(() => {
    if (!toast) return;
    // Ten seconds: long enough to notice an accidental delete and undo it.
    const timer = setTimeout(onDismiss, 10_000);
    return () => clearTimeout(timer);
  }, [toast, onDismiss]);

  if (!toast) return null;

  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-5 z-50 flex justify-center">
      <div className="vd-rise pointer-events-auto flex items-center gap-3 rounded-lg border border-line-strong bg-elevated py-2 pr-2 pl-3.5 shadow-[0_14px_40px_rgba(0,0,0,0.45)]">
        <span
          className={`text-[12px] ${toast.tone === "danger" ? "text-danger" : "text-ink"}`}
        >
          {toast.message}
        </span>
        {toast.action && (
          <button
            type="button"
            onClick={() => {
              toast.action?.run();
              onDismiss();
            }}
            className="flex items-center gap-1.5 rounded-md border border-line-strong px-2 py-[3px] text-[12px] text-ink transition-colors hover:bg-raised"
          >
            <UndoIcon size={12} />
            {toast.action.label}
          </button>
        )}
      </div>
    </div>
  );
}
