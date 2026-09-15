import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CloseIcon,
  DeckMark,
  MaximizeIcon,
  MinimizeIcon,
  RestoreIcon,
} from "./Icons";

/**
 * The window is undecorated, so the title bar and its buttons are ours to draw.
 *
 * The drag region deliberately does NOT wrap the buttons. Tauri swallows
 * mousedown anywhere inside a `data-tauri-drag-region` subtree to start the
 * drag, so a button nested under one never receives its click — it simply does
 * nothing. Only the left-hand strip is draggable.
 */
export default function TitleBar({ closeToTray = false }: { closeToTray?: boolean }) {
  const [maximized, setMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    win.isMaximized().then(setMaximized);
    win
      .onResized(() => {
        win.isMaximized().then(setMaximized);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, [win]);

  return (
    <header className="flex h-9 shrink-0 items-center justify-between border-b border-line bg-surface select-none">
      <div
        data-tauri-drag-region
        className="flex h-full flex-1 items-center gap-2 pl-3"
      >
        <DeckMark className="pointer-events-none text-ink" size={14} />
        <span className="pointer-events-none text-[11px] font-semibold tracking-[0.14em] text-muted uppercase">
          Vastdeck
        </span>
      </div>

      <div className="flex h-full">
        <WindowButton onClick={() => win.minimize()} label="Minimize">
          <MinimizeIcon />
        </WindowButton>
        <WindowButton onClick={() => win.toggleMaximize()} label="Maximize">
          {maximized ? <RestoreIcon /> : <MaximizeIcon />}
        </WindowButton>
        {/* Always a plain close: the Rust side turns it into a hide when the
            tray setting is on, so Alt+F4 behaves identically. */}
        <WindowButton
          onClick={() => win.close()}
          label={closeToTray ? "Close to tray" : "Close"}
          danger
        >
          <CloseIcon />
        </WindowButton>
      </div>
    </header>
  );
}

function WindowButton({
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
      onClick={onClick}
      className={`grid h-full w-11 place-items-center text-muted transition-colors hover:text-ink ${
        danger ? "hover:bg-danger hover:text-white" : "hover:bg-raised"
      }`}
    >
      {children}
    </button>
  );
}
