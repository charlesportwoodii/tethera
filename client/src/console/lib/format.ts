/**
 * The formatters the system shares. They live here rather than inside a
 * component because two components formatting bytes differently is a bug nobody
 * notices until a file card and a file viewer disagree about the same file.
 */

/** Binary units, because that is what a file manager on either desktop reports. */
export function formatBytes(bytes: number | bigint | null | undefined): string {
  // The wire sends a null size for a file it has not stat'd. Saying so beats
  // printing 0 B, which reads as an empty file.
  if (bytes === null || bytes === undefined) return "unknown size";
  const n = typeof bytes === "bigint" ? Number(bytes) : bytes;
  if (!Number.isFinite(n) || n < 0) return "unknown size";
  if (n < 1024) return n + " B";
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return (v < 10 ? v.toFixed(1) : Math.round(v).toString()) + " " + units[i];
}

/**
 * One decimal, rounded half away from zero, with a trailing zero dropped.
 *
 * Not `toFixed`: 1.45 is stored just below 1.45 in binary, so `(1.45).toFixed(1)`
 * is "1.4". Rounding the scaled integer is deterministic and matches what a
 * reader expects to see.
 */
function oneDecimal(value: number): string {
  return (Math.round(value * 10) / 10).toFixed(1).replace(/\.0$/, "");
}

/** Tokens, at the precision a person can act on: 18.4k, not 18,412. */
export function formatTokens(tokens: number): string {
  if (!Number.isFinite(tokens) || tokens < 0) return "—";
  if (tokens < 1000) return String(Math.round(tokens));
  const thousands = tokens / 1000;
  if (thousands < 100) return oneDecimal(thousands) + "k";
  if (thousands < 1000) return Math.round(thousands) + "k";
  return oneDecimal(thousands / 1000) + "M";
}

/**
 * Elapsed time, counting up. Seconds below a minute, then minutes and seconds,
 * then hours and minutes — a run measured in hours does not need its seconds.
 */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.floor(seconds);
  if (s < 60) return s + "s";
  const m = Math.floor(s / 60);
  if (m < 60) return m + "m " + (s % 60) + "s";
  const h = Math.floor(m / 60);
  return h + "h " + (m % 60) + "m";
}

/** The extension badge on a file. Not simply the last dot-separated part. */
export function fileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  // No dot at all, a leading dot (.gitignore is the whole name), or a trailing
  // dot means there is no extension to show.
  if (dot <= 0 || dot === name.length - 1) return "FILE";
  return name.slice(dot + 1, dot + 5).toUpperCase();
}
