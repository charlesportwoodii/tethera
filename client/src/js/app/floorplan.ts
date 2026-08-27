import type { AgentStatus } from "$bindings/AgentStatus";
import type { Pane } from "$bindings/Pane";
import type { TabLayout } from "$bindings/TabLayout";

/**
 * One pane, placed as a fraction of the tab it sits in.
 *
 * Percentages rather than cells, because the map is drawn at whatever width
 * the phone has and the cell grid it came from is the desk's.
 */
export interface Placement {
  pane: Pane;
  /** Percentages of the tab's own area, ready for a style attribute. */
  left: number;
  top: number;
  width: number;
  height: number;
  /** `C1`, `C2`… in reading order, for a pane running an agent. Null otherwise. */
  ordinal: string | null;
  /** What to call this rectangle: its ordinal, else its command, else its label. */
  name: string;
  /** The command or title under the name in the large map. Null when there is nothing to add. */
  detail: string | null;
  status: AgentStatus | null;
  /** This pane is currently filling the tab on its own. */
  zoomed: boolean;
}

/** Agent state per pane id, the same shape `TabStrip` takes per tab id. */
export type PaneStatus = Partial<Record<string, AgentStatus>>;

/**
 * The tab's panes as a drawable map.
 *
 * All the arithmetic lives here rather than in the component so the rules that
 * matter — that a partial layout draws nothing, and that the agent ordinals
 * follow reading order — can be tested without rendering anything.
 */
export class Floorplan {
  /**
   * Place every pane, or none of them.
   *
   * Returns an empty list unless the layout and the pane list describe exactly
   * the same set. A map missing one pane is not an incomplete picture: the
   * rects are normalised against the area they cover, so the survivors stretch
   * over the gap and the result looks entirely plausible. Refusing is the only
   * honest answer, and the caller draws nothing.
   */
  static place(
    panes: Pane[],
    layout: TabLayout | null,
    status: PaneStatus = {},
  ): Placement[] {
    if (layout === null || layout.slots.length === 0) {
      return [];
    }

    if (layout.slots.length !== panes.length) {
      return [];
    }

    const byId = new Map(panes.map((pane) => [pane.id as unknown as string, pane]));
    const paired = layout.slots.map((slot) => ({
      slot,
      pane: byId.get(slot.pane as unknown as string),
    }));

    if (paired.some((entry) => entry.pane === undefined)) {
      return [];
    }

    const left = Math.min(...paired.map((entry) => entry.slot.rect.x));
    const top = Math.min(...paired.map((entry) => entry.slot.rect.y));
    const right = Math.max(...paired.map((entry) => entry.slot.rect.x + entry.slot.rect.width));
    const bottom = Math.max(...paired.map((entry) => entry.slot.rect.y + entry.slot.rect.height));

    const across = right - left;
    const down = bottom - top;

    // A tab with no extent cannot be divided into one, and dividing by it would
    // put Infinity into a style attribute.
    if (across <= 0 || down <= 0) {
      return [];
    }

    // Reading order, which is what makes C1 the top-left agent rather than
    // whichever pane the backend happened to list first.
    const ordered = [...paired].sort((a, b) =>
      a.slot.rect.y === b.slot.rect.y
        ? a.slot.rect.x - b.slot.rect.x
        : a.slot.rect.y - b.slot.rect.y,
    );

    const zoomed = layout.zoomed === null ? null : (layout.zoomed as unknown as string);
    let agents = 0;

    return ordered.map((entry) => {
      const pane = entry.pane as Pane;
      const rect = entry.slot.rect;
      const id = pane.id as unknown as string;
      const ordinal = pane.agent === null ? null : `C${(agents += 1)}`;

      return {
        pane,
        left: ((rect.x - left) / across) * 100,
        top: ((rect.y - top) / down) * 100,
        width: (rect.width / across) * 100,
        height: (rect.height / down) * 100,
        ordinal,
        name: ordinal ?? pane.foreground_command ?? pane.label,
        detail: Floorplan.detail(pane, ordinal),
        status: status[id] ?? null,
        zoomed: zoomed === id,
      };
    });
  }

  /**
   * The line under the name.
   *
   * Never a repeat of the name above it. An agent's rectangle is titled `C2`
   * and has room to say what C2 is doing; a shell is already titled by its
   * command, so repeating it fills the space with nothing.
   */
  private static detail(pane: Pane, ordinal: string | null): string | null {
    if (ordinal === null) {
      return pane.title;
    }

    return pane.title ?? pane.foreground_command ?? pane.label;
  }

  /**
   * How many agents this tab is running.
   *
   * Counted from the panes rather than the placements, so it answers on a tab
   * whose geometry the backend will not report.
   */
  static agents(panes: Pane[]): number {
    return panes.filter((pane) => pane.agent !== null).length;
  }

  /**
   * The pane a terminal should open into.
   *
   * The first pane herdr does not report an agent for, because chat is the
   * better window onto an agent and dropping somebody into a TUI is what this
   * screen exists to avoid. Null when every pane belongs to an agent, which is
   * the caller's cue to offer a split rather than make one.
   */
  static shell(panes: Pane[]): Pane | null {
    return panes.find((pane) => pane.agent === null) ?? null;
  }
}
