export function formatBytes(n: number | null | undefined): string {
  if (n === null || n === undefined || isNaN(n)) return "—";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n;
  let u = -1;
  do {
    v /= 1024;
    u++;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[u]}`;
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "Unknown date";
  // Stored as "YYYY-MM-DD HH:MM:SS" (local) — display as-is.
  return iso.replace("T", " ");
}

export function formatDateGroup(iso: string | null | undefined): string {
  if (!iso) return "Unknown date";
  return iso.slice(0, 10);
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  const m = Math.floor(s / 60);
  return `${m}m ${Math.floor(s % 60)}s`;
}

export function shortPath(p: string, max = 48): string {
  if (p.length <= max) return p;
  const head = p.slice(0, Math.floor(max / 2) - 1);
  const tail = p.slice(-Math.floor(max / 2));
  return `${head}…${tail}`;
}

export function exifTaken(photo: { taken_at: string | null }): string {
  return photo.taken_at ? formatDateTime(photo.taken_at) : "—";
}
