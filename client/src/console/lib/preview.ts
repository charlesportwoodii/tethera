import type { PreviewKind } from "$console/types/files";

/**
 * Extensions worth reading as code even when the MIME type is unhelpful.
 *
 * A server that says `application/octet-stream` for a `.rs` file is common, and
 * the reader would rather see the source than be told there is no preview.
 */
const CODE_EXTENSIONS = new Set([
  "c", "cjs", "cpp", "cs", "css", "go", "h", "hpp", "java", "js", "json", "jsx",
  "kt", "lua", "mjs", "php", "py", "rb", "rs", "scss", "sh", "sql", "svelte",
  "swift", "toml", "ts", "tsx", "vue", "yaml", "yml", "zig",
]);

const TEXT_EXTENSIONS = new Set(["csv", "log", "txt"]);

function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  return name.slice(dot + 1).toLowerCase();
}

/**
 * How to show a file, from what the fetch head said and what the name suggests.
 *
 * MIME wins where it is specific. The extension is the fallback rather than the
 * primary, because a name is attacker-influenced in a way a served type is less
 * likely to be — but neither decides anything more dangerous than which
 * component renders text, so this is a legibility call, not a security one.
 */
export function previewKind(name: string, mime?: string | null): PreviewKind {
  const type = (mime ?? "").toLowerCase();
  const ext = extension(name);

  // An image is the one kind that is not text, so it is decided first and only
  // from the served type: rendering bytes as an image on the strength of a
  // filename is how a decoder gets fed something it did not expect.
  if (type.startsWith("image/")) {
    // SVG is a document that can carry script, not a picture. It reads as code.
    return type.includes("svg") ? "code" : "image";
  }

  if (type.includes("markdown") || ext === "md" || ext === "markdown") return "markdown";
  if (type.includes("diff") || type.includes("patch") || ext === "diff" || ext === "patch") {
    return "diff";
  }
  if (CODE_EXTENSIONS.has(ext)) return "code";
  if (type.includes("json") || type.includes("xml") || type.includes("yaml")) return "code";
  if (TEXT_EXTENSIONS.has(ext)) return "text";
  if (type.startsWith("text/")) return "text";

  return "none";
}

/** How much of a file a preview reads before it lets go of the stream. */
export const PREVIEW_BYTES = 64 * 1024;

/**
 * Whether a file is worth previewing at all.
 *
 * A phone should never pull a multi-gigabyte dump to decide it cannot show it,
 * and the head carries the length, so this is answerable before the first chunk.
 */
export function isPreviewable(name: string, mime?: string | null, len?: number | null): boolean {
  if (previewKind(name, mime) === "none") return false;
  // An image has to arrive whole to decode, so its size is a real limit. Text is
  // read to PREVIEW_BYTES and stopped, so its length does not matter.
  if (previewKind(name, mime) === "image") {
    return len === null || len === undefined ? false : len <= 8 * 1024 * 1024;
  }
  return true;
}
