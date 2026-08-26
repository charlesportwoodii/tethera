// The token names, for anything that needs to reach them from script rather
// than from CSS — a canvas, a chart, a Tauri window colour.
//
// Values deliberately live in _tokens.scss only. Reading them through
// getComputedStyle keeps one definition rather than two that drift.

export const TOKENS = [
  "--tc-bg",
  "--tc-surface",
  "--tc-surface-2",
  "--tc-surface-3",
  "--tc-ink",
  "--tc-ink-2",
  "--tc-ink-3",
  "--tc-rule",
  "--tc-rule-2",
  "--tc-accent",
  "--tc-accent-ink",
  "--tc-ok",
  "--tc-working",
  "--tc-attn",
  "--tc-term-bg",
  "--tc-term-fg",
  "--tc-term-dim",
  "--tc-term-chrome",
] as const;

export type TokenName = (typeof TOKENS)[number];

/** Resolve a token against an element, defaulting to the document root. */
export function token(name: TokenName, el: Element | null = null): string {
  const target = el ?? document.documentElement;
  return getComputedStyle(target).getPropertyValue(name).trim();
}
