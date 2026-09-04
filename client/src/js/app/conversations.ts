import type { Conversation } from "$bindings/Conversation";
import type { AgentStatus } from "$bindings/AgentStatus";
import type { GlyphState } from "$console/types/state";
import { Parts } from "./parts";

/**
 * How a conversation reads on a screen.
 *
 * The mapping lives here rather than in a component because both the server list
 * and the machine page draw the same twig, and two copies would drift the moment
 * a status is added.
 */
export class Conversations {
  /**
   * A conversation is live when a pane is attached to it.
   *
   * `binding` is the herdr pane. Its absence is what makes a conversation a
   * candidate for resuming rather than opening: history reads either way, but
   * nothing is running behind it.
   */
  static isLive(conversation: Conversation): boolean {
    return conversation.binding !== null && conversation.binding !== undefined;
  }

  static live(all: Conversation[]): Conversation[] {
    return all.filter((held) => Conversations.isLive(held));
  }

  static dormant(all: Conversation[]): Conversation[] {
    return all.filter((held) => !Conversations.isLive(held));
  }

  /**
   * Whether releasing this conversation is an act the machine can perform.
   *
   * Releasing closes the pane — `ClosePane`, which the machine implements and
   * advertises as `pane_close`. It does not end the agent: `StopConversation`
   * reaches a port that answers "needs backend", and `conversation_stop` is
   * deliberately not advertised, because closing a pane and ending an agent are
   * different acts and only the first is written.
   *
   * So the transcript survives, the conversation becomes unbound, and the
   * machine decides for itself whether it can be started again. That is why the
   * control says Release rather than Stop, and why its undo is Resume.
   */
  static releasable(conversation: Conversation, reachable: boolean): boolean {
    return reachable && Conversations.isLive(conversation);
  }

  /**
   * The machine's own answer, passed through — but only while it is answering.
   *
   * The status is deliberately not second-guessed. The server already decides
   * it: a bound conversation is Working, Idle or Blocked from its transcript
   * tail, and an unbound one is Done "whatever its records say, because nothing
   * is running". Re-deriving it on the client would be a second judgement that
   * drifts from the first, and it is the same reasoning that puts the preview on
   * the wire rather than making every client compute one.
   *
   * Two things put a row past having a status worth drawing, and both read as
   * the same hollow mark, because they say the same thing to somebody scanning
   * the column: nothing is running behind this row.
   *
   * - The machine is not answering. Every status under it is a memory of the
   *   last answer, and drawing it filled claims work may have finished, failed
   *   or still be going — which nothing on the device can support.
   * - No pane is attached. The server reports an unbound conversation as `Done`
   *   whatever its records say, and `Done` is a filled disc, so every session
   *   ever opened on a machine drew the same green mark as the ones running on
   *   it now.
   *
   * `reachable` is required rather than defaulted, because a caller that forgets
   * is exactly the caller that would show a live-looking dot for a machine that
   * went quiet.
   */
  static glyph(conversation: Conversation, reachable: boolean): GlyphState {
    if (!reachable || !Conversations.isLive(conversation)) {
      return "offline";
    }

    return conversation.status as AgentStatus as GlyphState;
  }

  /** "3m", "2h", "5d". Absent rather than "never" when nothing is recorded. */
  static age(conversation: Conversation): string | null {
    const last = conversation.last_active ?? conversation.started_at;

    if (last === null || last === undefined) {
      return null;
    }

    // `Timestamp` is epoch milliseconds, not seconds. Reading it as seconds put
    // every conversation tens of thousands of years in the future, which read
    // on screen as "now" for all of them and a date header of "NOV 18, 58621".
    const seconds = Math.floor((Date.now() - Number(last)) / 1000);

    if (seconds < 60) {
      return "now";
    }

    if (seconds < 3600) {
      return `${Math.floor(seconds / 60)}m`;
    }

    if (seconds < 86400) {
      return `${Math.floor(seconds / 3600)}h`;
    }

    return `${Math.floor(seconds / 86400)}d`;
  }

  /** The workspace and the agent, which is what tells two rows apart. */
  static meta(conversation: Conversation): string {
    const workspace = conversation.workspace as unknown as string | null;

    return [workspace, conversation.profile_label].filter(Boolean).join(" · ");
  }

  /** A conversation with no title of its own falls back to where it is running. */
  static title(conversation: Conversation): string {
    return conversation.title ?? conversation.cwd;
  }

  /**
   * The machine's own one line, as plain text.
   *
   * The server decides what is meaningful — the pending question's prompt when
   * blocked, the agent's last words otherwise — and it arrives as markdown,
   * because that is what the agent wrote. Stripped here rather than at each
   * call site so two screens cannot render the same sentence differently.
   */
  static preview(conversation: Conversation): string {
    return Parts.plain(conversation.preview);
  }

  /** When this conversation last did anything, as a local Date. */
  static when(conversation: Conversation): Date {
    const at = conversation.last_active ?? conversation.started_at;

    // Milliseconds already. See the note in `age`.
    return new Date(Number(at));
  }

  /**
   * Groups conversations under day headings, newest first.
   *
   * A paginated index of a machine that has been running for months is a wall of
   * rows; the date is the only thing that makes a position in it meaningful.
   * Order within a day is preserved from the server, which already sorts.
   */
  static byDay(all: Conversation[], now: Date = new Date()): DayGroup[] {
    const groups: DayGroup[] = [];

    for (const held of all) {
      const when = Conversations.when(held);
      const label = Conversations.dayLabel(when, now);
      const last = groups.at(-1);

      // Compared against the previous group rather than looked up in a map, so
      // a server that returns rows out of order produces two dated groups
      // rather than one that silently reorders its contents.
      if (last && last.label === label) {
        last.items.push(held);

        continue;
      }

      // Position, not the label: two groups may legitimately share a label, and
      // the key has to tell them apart.
      groups.push({ key: `${groups.length}:${label}`, label, items: [held] });
    }

    return groups;
  }

  /**
   * "Today", "Yesterday", "Sat 23 Aug", "23 Aug 2025".
   *
   * The year appears only when it is not this one: it is noise on the rows
   * somebody is most likely to be looking for.
   */
  static dayLabel(when: Date, now: Date = new Date()): string {
    const days = Conversations.daysBetween(when, now);

    if (days === 0) {
      return "Today";
    }

    if (days === 1) {
      return "Yesterday";
    }

    const sameYear = when.getFullYear() === now.getFullYear();

    return when.toLocaleDateString(undefined, {
      weekday: sameYear ? "short" : undefined,
      day: "numeric",
      month: "short",
      year: sameYear ? undefined : "numeric",
    });
  }

  // Calendar days apart, not elapsed hours. 23:50 and 00:10 are a different day
  // to a person and twenty minutes to a clock.
  private static daysBetween(when: Date, now: Date): number {
    const a = new Date(when.getFullYear(), when.getMonth(), when.getDate());
    const b = new Date(now.getFullYear(), now.getMonth(), now.getDate());

    return Math.round((b.getTime() - a.getTime()) / 86_400_000);
  }
}

export interface DayGroup {
  /**
   * Unique across the returned list, which the label is not.
   *
   * Rows arriving out of date order produce two groups reading "Today", by
   * design. Keyed on the label, an `{#each}` over these hits Svelte's
   * duplicate-key error and renders the whole block as nothing — silently, and
   * only once real data has more than one day in it.
   */
  key: string;
  label: string;
  items: Conversation[];
}
