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
