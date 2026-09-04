import type { Key } from "$bindings/Key";
import type { Mods } from "$bindings/Mods";

export interface KeyStreamProps {
  /**
   * One named key or modified character, as it is pressed.
   *
   * Intent, never bytes: the server owns the encoding, which is why a `^C` cap
   * on a phone does not have to know a terminal table.
   */
  onkey: (key: Key, mods: Mods) => void;
  /**
   * Printable characters, unmodified, as they are typed.
   *
   * Separate from `onkey` because text is what a soft keyboard produces and a
   * keypress is what a physical one does. Both reach the same pty.
   */
  ontext: (text: string) => void;
  /** Absent when the machine will not take input. */
  disabled?: boolean;
  /** Drawn in the field before anything is typed. */
  placeholder?: string;
}
