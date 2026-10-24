export function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  if (diff < 0) return "just now";
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (diff < minute) return "just now";
  if (diff < hour) return `${Math.floor(diff / minute)}m ago`;
  if (diff < day) return `${Math.floor(diff / hour)}h ago`;
  if (diff < 7 * day) return `${Math.floor(diff / day)}d ago`;
  if (diff < 365 * day) {
    return new Date(ms).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  }
  return new Date(ms).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
  });
}

export function fullTime(ms: number): string {
  return new Date(ms).toLocaleString();
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  const mb = n / (1024 * 1024);
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

export function cost(usd: number | null | undefined): string | null {
  if (usd == null || usd <= 0) return null;
  if (usd < 1) return `$${usd.toFixed(2)}`;
  return `$${Math.round(usd)}`;
}

export function basename(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Drops the drive letter and keeps the tail, which is the part that identifies a project. */
export function shortPath(path: string, maxSegments = 4): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length <= maxSegments) return parts.join("\\");
  return "…\\" + parts.slice(-maxSegments).join("\\");
}

export function normalizeWorkspace(path: string): string {
  return path.replace(/\\/g, "/").toLowerCase().replace(/\/+$/, "");
}

export interface WorkspaceGroup<T> {
  workspace: string;
  items: T[];
  /** Workspaces that sit inside this one, nested the same way. */
  children: WorkspaceGroup<T>[];
  /** Newest `updatedAt` across this group and everything nested in it. */
  latest: number;
}

/**
 * Groups items by workspace and files each group under the nearest other
 * workspace whose folder contains it, so `A\B` shows inside `A`. Groups at
 * every level are ordered newest first.
 */
export function nestWorkspaces<T extends { workspace: string; updatedAt: number }>(
  items: T[],
): WorkspaceGroup<T>[] {
  const byKey = new Map<string, WorkspaceGroup<T>>();
  for (const item of items) {
    const key = normalizeWorkspace(item.workspace);
    const group = byKey.get(key);
    if (group) {
      group.items.push(item);
      group.latest = Math.max(group.latest, item.updatedAt);
    } else {
      byKey.set(key, {
        workspace: item.workspace,
        items: [item],
        children: [],
        latest: item.updatedAt,
      });
    }
  }

  const roots: WorkspaceGroup<T>[] = [];
  for (const [key, group] of byKey) {
    let parent: WorkspaceGroup<T> | undefined;
    for (let i = key.lastIndexOf("/"); i > 0 && !parent; i = key.lastIndexOf("/", i - 1)) {
      parent = byKey.get(key.slice(0, i));
    }
    (parent ? parent.children : roots).push(group);
  }

  const order = (groups: WorkspaceGroup<T>[]): number => {
    let latest = 0;
    for (const group of groups) {
      group.latest = Math.max(group.latest, order(group.children));
      latest = Math.max(latest, group.latest);
    }
    groups.sort((a, b) => b.latest - a.latest);
    return latest;
  };
  order(roots);
  return roots;
}

export interface Searchable {
  id: string;
  workspace: string;
  title?: string | null;
  aiTitle?: string | null;
  preview?: string | null;
  gitBranch?: string | null;
}

/**
 * How well `query` matches: every word must appear whole in some field, and a
 * word counts for the best field it is found in — the title above the last
 * prompt above the folder, branch and id. Returns 0 when any word is missing.
 */
export function searchScore(query: string, item: Searchable): number {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return 1;
  const fields: [string, number][] = [
    [`${item.title ?? ""} ${item.aiTitle ?? ""}`, 4],
    [item.preview ?? "", 2],
    [`${item.workspace} ${item.gitBranch ?? ""} ${item.id}`, 1],
  ];
  const lowered = fields.map(([text, weight]) => [text.toLowerCase(), weight] as const);
  let score = 0;
  for (const word of words) {
    const best = Math.max(0, ...lowered.filter(([t]) => t.includes(word)).map(([, w]) => w));
    if (best === 0) return 0;
    score += best;
  }
  return score;
}

/** Subsequence match, the same forgiving behaviour an editor's quick-open has. */
export function fuzzyMatch(needle: string, haystack: string): boolean {
  if (!needle) return true;
  const n = needle.toLowerCase();
  const h = haystack.toLowerCase();
  if (h.includes(n)) return true;
  let i = 0;
  for (const ch of h) {
    if (ch === n[i]) i++;
    if (i === n.length) return true;
  }
  return false;
}
