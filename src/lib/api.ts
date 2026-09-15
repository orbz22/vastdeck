import { invoke } from "@tauri-apps/api/core";

export type LaunchMode =
  | "normal"
  | "skipPermissions"
  | "acceptEdits"
  | "plan"
  | "fork";

export interface LiveInfo {
  pid: number;
  status: string;
  name: string | null;
  startedAt: number | null;
}

export interface Session {
  id: string;
  provider: string;
  path: string;
  workspace: string;
  title: string | null;
  preview: string | null;
  gitBranch: string | null;
  cliVersion: string | null;
  permissionMode: string | null;
  createdAt: number | null;
  updatedAt: number;
  sizeBytes: number;
  messageCount: number | null;
  live: LiveInfo | null;
}

export interface WorkspaceStats {
  costUsd: number | null;
  linesAdded: number | null;
  linesRemoved: number | null;
  lastSessionId: string | null;
}

export interface SessionList {
  sessions: Session[];
  stats: Record<string, WorkspaceStats>;
}

export interface ProviderInfo {
  id: string;
  name: string;
  installed: boolean;
  sessionCount: number;
}

export interface AppInfo {
  version: string;
  portable: boolean;
}

export interface DeletedSession {
  backupDir: string;
  sessionId: string;
  workspace: string;
  title: string | null;
  preview: string | null;
  deletedAt: number;
  sizeBytes: number;
  restorable: boolean;
}

export interface DeletedHandle {
  sessionId: string;
  backupDir: string | null;
  originalPath: string;
  originalEnvDir: string | null;
}

export type TerminalChoice =
  | "auto"
  | "windowsTerminal"
  | "pwsh"
  | "powerShell"
  | "cmd";

export interface Settings {
  terminal: TerminalChoice;
  wtNewTab: boolean;
  closeToTray: boolean;
  startWithWindows: boolean;
  defaultLaunchMode: LaunchMode;
  hardDelete: boolean;
  theme: string;
  groupByWorkspace: boolean;
  sort: string;
  sessionModes: Record<string, LaunchMode>;
  trustedWorkspaces: string[];
  claudePath: string | null;
}

export const api = {
  listSessions: () => invoke<SessionList>("list_sessions"),
  messageCounts: () => invoke<Record<string, number>>("message_counts"),
  listProviders: () => invoke<ProviderInfo[]>("list_providers"),
  launch: (session: Session, mode: LaunchMode) =>
    invoke<string>("launch_session", { session, mode }),
  focus: (pid: number) => invoke<boolean>("focus_session", { pid }),
  deleteSession: (session: Session) =>
    invoke<DeletedHandle>("delete_session", { session }),
  deleteForever: (session: Session) =>
    invoke<DeletedHandle>("delete_session_permanently", { session }),
  undoDelete: (handle: DeletedHandle) =>
    invoke<void>("undo_delete", { handle }),
  listDeleted: () => invoke<DeletedSession[]>("list_deleted"),
  restoreDeleted: (backupDir: string) =>
    invoke<void>("restore_deleted", { backupDir }),
  purgeDeleted: (backupDir: string) =>
    invoke<void>("purge_deleted", { backupDir }),
  purgeAllDeleted: () => invoke<number>("purge_all_deleted"),
  getSettings: () => invoke<Settings>("get_settings"),
  /** Returns `[directory, isPortable]`. */
  dataLocation: () => invoke<[string, boolean]>("data_location"),
  appInfo: () => invoke<AppInfo>("app_info"),
  setSettings: (settings: Settings) =>
    invoke<void>("set_settings", { settings }),
  trustWorkspace: (workspace: string) =>
    invoke<void>("trust_workspace", { workspace }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
};

export const LAUNCH_MODES: {
  id: LaunchMode;
  label: string;
  hint: string;
  danger?: boolean;
}[] = [
  { id: "normal", label: "Resume", hint: "Permission prompts stay on" },
  {
    id: "skipPermissions",
    label: "Skip permissions",
    hint: "--dangerously-skip-permissions",
    danger: true,
  },
  { id: "acceptEdits", label: "Accept edits", hint: "Auto-approve file edits" },
  { id: "plan", label: "Plan", hint: "Read-only, plan first" },
  { id: "fork", label: "Fork", hint: "Branch off, keep the original" },
];

export function modeLabel(mode: LaunchMode): string {
  return LAUNCH_MODES.find((m) => m.id === mode)?.label ?? "Resume";
}
