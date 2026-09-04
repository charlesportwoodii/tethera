import type { Conversation } from "$bindings/Conversation";
import type { ServerRow } from "$bindings/ServerRow";
import type { GlyphState } from "$console/types/state";
import { Conversations } from "./conversations";

/** One conversation, and the machine it is running on. */
export interface Waiting {
  row: ServerRow;
  conversation: Conversation;
}

/**
 * What a screen showing every machine at once needs to know.
 *
 * Here rather than in `ServerTile` because none of it is presentational: the
 * ordering is a product decision, and what the counts can honestly say depends
 * on what the sweep fetched, which is a question about the wire.
 */
export class Fleet {
  /**
   * How the strip reads, urgent first.
   *
   * Sorted rather than left in the server's order, which is by recency. A
   * blocked session that started on Tuesday would otherwise sit at the end of a
   * twelve-mark strip, which is the one place nobody looks.
   */
  private static readonly ORDER: GlyphState[] = [
    "blocked",
    "stalled",
    "working",
    "idle",
    "done",
    "offline",
  ];

  /** Which marks mean a person has to do something. */
  private static readonly NEEDS_A_PERSON: GlyphState[] = ["blocked", "stalled"];

  /** The states named in the sentence, in the order they are read. */
  private static readonly COUNTED: GlyphState[] = ["working", "idle", "done"];

  static reachable(row: ServerRow): boolean {
    return row.link.kind !== "offline";
  }

  static states(row: ServerRow): GlyphState[] {
    const reachable = Fleet.reachable(row);

    return row.entry.conversations
      .map((held) => Conversations.glyph(held, reachable))
      .sort((a, b) => Fleet.ORDER.indexOf(a) - Fleet.ORDER.indexOf(b));
  }

  static attention(row: ServerRow): boolean {
    return Fleet.states(row).some((state) => Fleet.NEEDS_A_PERSON.includes(state));
  }

  /**
   * The one line under the strip.
   *
   * The strip cannot be read by a screen reader, and cannot be read at all by
   * somebody who has not learnt the shapes, so the counts say it in words. Only
   * the states actually present are named: a sentence listing every state a
   * machine does not have is longer and says less.
   */
  static sentence(row: ServerRow): string {
    const states = Fleet.states(row);

    if (states.length === 0) {
      return "";
    }

    // A quiet machine is dated, not described. Every status under it is a
    // memory of the last answer, so counting them by state would report work as
    // though somebody had just measured it.
    if (!Fleet.reachable(row)) {
      const many = states.length === 1 ? "session" : "sessions";

      return `${states.length} ${many} when it went quiet`;
    }

    const needed = states.filter((state) => Fleet.NEEDS_A_PERSON.includes(state)).length;
    const parts: string[] = [];

    if (needed > 0) {
      parts.push(`${needed} ${needed === 1 ? "needs" : "need"} you`);
    }

    for (const state of Fleet.COUNTED) {
      const count = states.filter((candidate) => candidate === state).length;

      if (count > 0) {
        parts.push(`${count} ${state}`);
      }
    }

    return parts.join(" · ");
  }

  /**
   * Everything waiting on a person, across every machine, newest first.
   *
   * A machine that is not answering contributes nothing: whatever its
   * remembered status says, nothing there can receive an answer, and offering
   * one would be a control that cannot work.
   */
  static waiting(rows: ServerRow[]): Waiting[] {
    return rows
      .filter((row) => Fleet.reachable(row))
      .flatMap((row) =>
        row.entry.conversations
          .filter((held) => Fleet.NEEDS_A_PERSON.includes(Conversations.glyph(held, true)))
          .map((conversation) => ({ row, conversation })),
      )
      .sort(Fleet.newestFirst);
  }

  /**
   * Everything running, across every machine, newest first.
   *
   * Running, not remembered: a session with no pane is not doing anything, and a
   * machine that is not answering has nothing anybody can see. Both belong on
   * the machine page, which indexes what a machine *has*; this band answers the
   * only question a dashboard is for, which is what is happening now.
   *
   * Uncapped, because that count is already bounded by what a sweep carries per
   * machine, and truncating the answer to "what is running" would leave a
   * session going with nothing on the screen to say so.
   */
  static active(rows: ServerRow[]): Waiting[] {
    return rows
      .filter((row) => Fleet.reachable(row))
      .flatMap((row) =>
        row.entry.conversations
          .filter((held) => Conversations.isLive(held))
          .map((conversation) => ({ row, conversation })),
      )
      .sort(Fleet.newestFirst);
  }

  private static newestFirst(a: Waiting, b: Waiting): number {
    return (
      Conversations.when(b.conversation).getTime() - Conversations.when(a.conversation).getTime()
    );
  }
}
