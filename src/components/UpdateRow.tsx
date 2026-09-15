import { useCallback, useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { api, type AppInfo } from "../lib/api";
import { WarnIcon } from "./Icons";

const RELEASES = "https://github.com/orbz22/vastdeck/releases/latest";

type State =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "current" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; percent: number | null }
  | { kind: "ready" }
  | { kind: "error"; message: string };

/**
 * Update from the GitHub release, verified against the public key baked into
 * the build. A portable copy gets the release page instead — the updater works
 * by running an installer, and a portable folder has nothing to install into.
 */
export default function UpdateRow() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [state, setState] = useState<State>({ kind: "idle" });

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  const runCheck = useCallback(async () => {
    setState({ kind: "checking" });
    try {
      const update = await check();
      setState(update ? { kind: "available", update } : { kind: "current" });
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, []);

  const install = useCallback(async (update: Update) => {
    setState({ kind: "downloading", percent: null });
    try {
      let total = 0;
      let done = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          done += event.data.chunkLength;
          setState({
            kind: "downloading",
            percent: total > 0 ? Math.round((done / total) * 100) : null,
          });
        } else if (event.event === "Finished") {
          setState({ kind: "ready" });
        }
      });
      setState({ kind: "ready" });
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, []);

  if (!info) return null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <span className="min-w-0">
          <span className="block text-[12px] text-ink">Version</span>
          <span className="block font-mono text-[11px] text-faint">
            {info.version}
            {info.portable && " · portable"}
          </span>
        </span>
        {renderAction()}
      </div>
      {state.kind === "available" && info.portable && (
        <Note>
          Portable copies update by downloading the new zip — the installer the
          updater runs has nothing to install into here.
        </Note>
      )}
      {state.kind === "available" && state.update.body && !info.portable && (
        <p className="max-h-20 overflow-y-auto rounded-md border border-line bg-elevated px-2.5 py-2 text-[11.5px] leading-relaxed text-muted">
          {state.update.body}
        </p>
      )}
      {state.kind === "error" && (
        <Note tone="warn">{state.message}</Note>
      )}
    </div>
  );

  function renderAction() {
    switch (state.kind) {
      case "checking":
        return <Label>Checking…</Label>;
      case "current":
        return (
          <span className="flex shrink-0 items-center gap-2">
            <Label>Up to date</Label>
            <Button onClick={runCheck}>Check</Button>
          </span>
        );
      case "available":
        return info!.portable ? (
          <Button onClick={() => api.openPath(RELEASES).catch(() => {})}>
            {state.update.version} — open releases
          </Button>
        ) : (
          <Button primary onClick={() => install(state.update)}>
            Update to {state.update.version}
          </Button>
        );
      case "downloading":
        return (
          <Label>
            {state.percent === null ? "Downloading…" : `Downloading ${state.percent}%`}
          </Label>
        );
      case "ready":
        return (
          <Button primary onClick={() => relaunch()}>
            Restart to finish
          </Button>
        );
      case "error":
        return <Button onClick={runCheck}>Try again</Button>;
      default:
        return <Button onClick={runCheck}>Check</Button>;
    }
  }
}

const Label = ({ children }: { children: React.ReactNode }) => (
  <span className="shrink-0 text-[11.5px] text-faint">{children}</span>
);

function Button({
  children,
  onClick,
  primary,
}: {
  children: React.ReactNode;
  onClick: () => void;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "shrink-0 rounded-md px-2.5 py-[5px] text-[12px] transition-colors",
        primary
          ? "bg-invert text-invert-ink hover:opacity-90"
          : "border border-line-strong text-muted hover:bg-raised hover:text-ink",
      ].join(" ")}
    >
      {children}
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
      <span className="min-w-0 break-words">{children}</span>
    </p>
  );
}
