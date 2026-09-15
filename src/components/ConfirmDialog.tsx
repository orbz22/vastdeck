import { useEffect } from "react";
import { WarnIcon } from "./Icons";

export interface ConfirmRequest {
  title: string;
  body: string;
  detail?: string;
  confirmLabel: string;
  tone?: "neutral" | "warn" | "danger";
  onConfirm: () => void;
}

export default function ConfirmDialog({
  request,
  onClose,
}: {
  request: ConfirmRequest | null;
  onClose: () => void;
}) {
  useEffect(() => {
    if (!request) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "Enter") {
        request.onConfirm();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [request, onClose]);

  if (!request) return null;
  const warn = request.tone === "warn";
  const danger = request.tone === "danger";

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/45 px-6"
      onClick={onClose}
    >
      <div
        className="vd-rise w-full max-w-[420px] rounded-xl border border-line-strong bg-elevated p-5 shadow-[0_24px_64px_rgba(0,0,0,0.5)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-3">
          {(warn || danger) && (
            <WarnIcon
              className={`mt-[2px] shrink-0 ${danger ? "text-danger" : "text-warn"}`}
              size={16}
            />
          )}
          <div className="min-w-0">
            <h2 className="text-[14px] text-ink">{request.title}</h2>
            <p className="mt-1.5 text-[12.5px] leading-relaxed text-muted">
              {request.body}
            </p>
            {request.detail && (
              <p className="mt-2 truncate rounded-md border border-line bg-surface px-2 py-1.5 font-mono text-[11px] text-faint">
                {request.detail}
              </p>
            )}
          </div>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-line-strong px-3 py-[6px] text-[12px] text-muted transition-colors hover:bg-raised hover:text-ink"
          >
            Cancel
          </button>
          <button
            type="button"
            autoFocus
            onClick={() => {
              request.onConfirm();
              onClose();
            }}
            className={[
              "rounded-md px-3 py-[6px] text-[12px] transition-opacity hover:opacity-90",
              danger
                ? "bg-danger text-white"
                : warn
                  ? "bg-warn text-black"
                  : "bg-invert text-invert-ink",
            ].join(" ")}
          >
            {request.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
