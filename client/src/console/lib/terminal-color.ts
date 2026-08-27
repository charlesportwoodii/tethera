import type { Color } from "$bindings/Color";
import type { Style } from "$bindings/Style";
import { ATTR } from "./terminal";

/**
 * Terminal colour to CSS.
 *
 * The first sixteen slots read the design system's own tokens rather than a
 * fixed palette, so a pane looks like it belongs to the app instead of like a
 * screenshot of a different program. Everything above that is the standard
 * xterm-256 arrangement, which is fixed by convention and not ours to restyle.
 */

/** Slots 0–15, as custom properties so a theme can move them. */
const BASE: readonly string[] = [
  "var(--tc-term-black, #12181f)",
  "var(--tc-term-red, #ff5c8a)",
  "var(--tc-term-green, #3ddc97)",
  "var(--tc-term-yellow, #ffc14d)",
  "var(--tc-term-blue, #4c9aff)",
  "var(--tc-term-magenta, #c58bc0)",
  "var(--tc-term-cyan, #63b8b0)",
  "var(--tc-term-white, #d6ddea)",
  "var(--tc-term-bright-black, #5f6a7d)",
  "var(--tc-term-bright-red, #ff85a6)",
  "var(--tc-term-bright-green, #6fe8b4)",
  "var(--tc-term-bright-yellow, #ffd37a)",
  "var(--tc-term-bright-blue, #7fb2e8)",
  "var(--tc-term-bright-magenta, #d9a8d4)",
  "var(--tc-term-bright-cyan, #8ad2cb)",
  "var(--tc-term-bright-white, #ffffff)",
];

const CUBE = [0, 95, 135, 175, 215, 255] as const;

/** One of the 256 indexed slots as a CSS colour. */
export function indexedColor(index: number): string {
  const i = Math.max(0, Math.min(255, Math.floor(index)));
  if (i < 16) return BASE[i];
  if (i < 232) {
    // A 6×6×6 cube starting at 16.
    const n = i - 16;
    const r = CUBE[Math.floor(n / 36) % 6];
    const g = CUBE[Math.floor(n / 6) % 6];
    const b = CUBE[n % 6];
    return "rgb(" + r + " " + g + " " + b + ")";
  }
  // Twenty-four greys, evenly spaced.
  const level = 8 + (i - 232) * 10;
  return "rgb(" + level + " " + level + " " + level + ")";
}

/** A wire colour as CSS, or null for "whatever the pane's own colour is". */
export function cssColor(color: Color): string | null {
  if (color === "default") return null;
  if ("indexed" in color) return indexedColor(color.indexed);
  const [r, g, b] = color.rgb;
  return "rgb(" + r + " " + g + " " + b + ")";
}

export interface RunStyle {
  /** Resolved foreground, or null for the pane's own ink. */
  fg: string | null;
  /** Resolved background, or null for the pane's own ground. */
  bg: string | null;
  /** Class names for the attributes that are not colours. */
  classes: string;
}

/**
 * Resolve one style into what a span needs.
 *
 * `reverse` is applied here rather than in CSS because swapping requires knowing
 * both resolved colours, and a class cannot see them. `hidden` wins over
 * everything: text the pane hid must not be readable by selecting it.
 */
export function runStyle(style: Style): RunStyle {
  let fg = cssColor(style.fg);
  let bg = cssColor(style.bg);

  if ((style.attrs & ATTR.reverse) !== 0) {
    // Unset sides fall back to the pane's own ground and ink, which is what
    // reverse means when one side was never specified.
    const swapFg = bg ?? "var(--tc-term-bg)";
    const swapBg = fg ?? "var(--tc-term-fg)";
    fg = swapFg;
    bg = swapBg;
  }

  const classes: string[] = [];
  if ((style.attrs & ATTR.bold) !== 0) classes.push("is-bold");
  if ((style.attrs & ATTR.dim) !== 0) classes.push("is-dim");
  if ((style.attrs & ATTR.italic) !== 0) classes.push("is-italic");
  if ((style.attrs & ATTR.underline) !== 0) classes.push("is-underline");
  if ((style.attrs & ATTR.blink) !== 0) classes.push("is-blink");
  if ((style.attrs & ATTR.strike) !== 0) classes.push("is-strike");
  if ((style.attrs & ATTR.hidden) !== 0) classes.push("is-hidden");

  return { fg, bg, classes: classes.join(" ") };
}
