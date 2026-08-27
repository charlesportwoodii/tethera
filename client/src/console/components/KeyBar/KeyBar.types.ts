import type { Key } from "$bindings/Key";
import type { Mods } from "$bindings/Mods";

/**
 * Modifier bits on `Mods`.
 *
 * The wire types `Mods` as a bare number, so the bit meanings are a convention
 * shared with the server rather than something the type enforces. Named here so
 * there is one place to correct if the server disagrees.
 */
export const MOD = {
  none: 0,
  shift: 1,
  alt: 2,
  ctrl: 4,
  meta: 8,
} as const;

/** One key on the bar. */
export interface KeyCap {
  /** What the person sees. */
  label: string;
  /** What is sent. Intent, never bytes — the server does the encoding. */
  key: Key;
  mods?: Mods;
  /** Two columns wide, for a key worth hitting. */
  wide?: boolean;
}

export interface KeyBarProps {
  rows?: KeyCap[][];
  /**
   * Sends one keypress as intent.
   *
   * There is deliberately no raw-bytes path: the wire has none, because it buys
   * nothing and opens an injection surface. A bar that emitted "^C" as a string
   * would push the encoding decision onto whoever caught it, which is the thing
   * the protocol is arranged to prevent.
   */
  onkey?: (key: Key, mods: Mods) => void;
}

/** The keys a terminal on a phone cannot do without. */
export const DEFAULT_KEYS: KeyCap[][] = [
  [
    { label: "esc", key: "escape" },
    { label: "tab", key: "tab" },
    { label: "↑", key: "up" },
    { label: "↓", key: "down" },
    { label: "←", key: "left" },
    { label: "→", key: "right" },
  ],
  [
    { label: "^C", key: { char: "c" }, mods: MOD.ctrl, wide: true },
    { label: "^D", key: { char: "d" }, mods: MOD.ctrl, wide: true },
    { label: "^L", key: { char: "l" }, mods: MOD.ctrl, wide: true },
    { label: "^Z", key: { char: "z" }, mods: MOD.ctrl, wide: true },
    { label: "⏎", key: "enter", wide: true },
  ],
];
