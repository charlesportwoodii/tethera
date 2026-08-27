import { describe, expect, test } from "vitest";
import { Conversations } from "./conversations";
import type { Conversation } from "$bindings/Conversation";

// `Timestamp` is epoch milliseconds.
function a(bound: boolean, status = "working", lastActive: number | null = null): Conversation {
  return {
    id: "cv_1",
    profile: "claude",
    profile_label: "claude",
    title: "Pairing deep link",
    preview: "Which route should own tethera://pair?",
    cwd: "/home/charl/projects/tethera",
    workspace: "tethera-3",
    started_at: 1,
    last_active: lastActive,
    turn_count: 7,
    status,
    has_transcript: true,
    binding: bound ? "pn_1" : null,
  } as unknown as Conversation;
}

describe("Conversations", () => {
  // `binding` is the herdr pane. Its absence is what makes a conversation a
  // resume candidate rather than something to open, so the split has to key off
  // it and not off status.
  test("a conversation is live only while a pane is attached", () => {
    expect(Conversations.isLive(a(true))).toBe(true);
    expect(Conversations.isLive(a(false))).toBe(false);
  });

  test("the two sections partition the list with nothing lost", () => {
    const all = [a(true), a(false), a(true)];

    expect(Conversations.live(all)).toHaveLength(2);
    expect(Conversations.dormant(all)).toHaveLength(1);
    expect(Conversations.live(all).length + Conversations.dormant(all).length).toBe(all.length);
  });

  // The machine already decides this - an unbound conversation is reported Done
  // by the server, whatever its records say. Overriding it here suppressed the
  // colour that distinguishes working from blocked on every row.
  test("the machine's status is passed through rather than re-derived", () => {
    expect(Conversations.glyph(a(true, "working"), true)).toBe("working");
    expect(Conversations.glyph(a(true, "blocked"), true)).toBe("blocked");
    expect(Conversations.glyph(a(true, "idle"), true)).toBe("idle");
  });

  // The server reports an unbound conversation as Done, and Done is a filled
  // mark. Passed through, every session ever opened on a machine drew the same
  // mark as the handful actually running on it.
  test("a conversation with no pane attached has no status to draw", () => {
    expect(Conversations.glyph(a(false, "done"), true)).toBe("offline");
    expect(Conversations.glyph(a(false, "working"), true)).toBe("offline");
  });

  // Every status under a machine that stopped answering is a memory of its last
  // answer. Drawn as it stands, a session that was Working keeps a live-looking
  // dot for as long as the screen is open, for work that may have finished,
  // failed, or still be running.
  test("a status from a machine that is not answering reads offline", () => {
    expect(Conversations.glyph(a(true, "working"), false)).toBe("offline");
    expect(Conversations.glyph(a(true, "blocked"), false)).toBe("offline");
    expect(Conversations.glyph(a(false, "done"), false)).toBe("offline");
  });

  test("age reads in the largest unit that still says something", () => {
    const now = Date.now();

    expect(Conversations.age(a(true, "working", now - 30_000))).toBe("now");
    expect(Conversations.age(a(true, "working", now - 300_000))).toBe("5m");
    expect(Conversations.age(a(true, "working", now - 7_200_000))).toBe("2h");
    expect(Conversations.age(a(true, "working", now - 172_800_000))).toBe("2d");
  });

  // Timestamps are milliseconds. Reading them as seconds put every conversation
  // tens of thousands of years ahead, which showed as "now" on every row and a
  // date header reading "NOV 18, 58621".
  test("a timestamp is read as milliseconds, not seconds", () => {
    const anHourAgo = Date.now() - 3_600_000;

    expect(Conversations.age(a(true, "working", anHourAgo))).toBe("1h");
    expect(Conversations.when(a(true, "working", anHourAgo)).getFullYear()).toBe(
      new Date().getFullYear(),
    );
  });

  // Falling back to started_at rather than showing nothing: a conversation that
  // has never been active still began at a time worth reporting.
  test("age falls back to when it started", () => {
    const conversation = a(true, "idle", null);

    expect(Conversations.age(conversation)).not.toBeNull();
  });

  test("a conversation with no title of its own is named by its directory", () => {
    const conversation = { ...a(true), title: null } as Conversation;

    expect(Conversations.title(conversation)).toBe("/home/charl/projects/tethera");
  });
});

describe("Conversations.byDay", () => {
  const now = new Date(2026, 7, 26, 14, 0, 0);

  function at(date: Date): Conversation {
    return { ...a(false), last_active: date.getTime() } as Conversation;
  }

  test("today and yesterday are named, not dated", () => {
    expect(Conversations.dayLabel(new Date(2026, 7, 26, 9, 0), now)).toBe("Today");
    expect(Conversations.dayLabel(new Date(2026, 7, 25, 23, 0), now)).toBe("Yesterday");
  });

  // Ten minutes apart across midnight is a different day to a person and a
  // rounding error to a clock. The boundary has to be the calendar's.
  test("the boundary is the calendar day, not twenty-four hours", () => {
    const justBeforeMidnight = new Date(2026, 7, 25, 23, 50);
    const justAfterMidnight = new Date(2026, 7, 26, 0, 10);

    expect(Conversations.dayLabel(justBeforeMidnight, now)).toBe("Yesterday");
    expect(Conversations.dayLabel(justAfterMidnight, now)).toBe("Today");
  });

  // The year is noise on the rows somebody is most likely looking for, and
  // essential on the ones they are not.
  test("the year appears only when it is not this one", () => {
    expect(Conversations.dayLabel(new Date(2026, 7, 20), now)).not.toContain("2026");
    expect(Conversations.dayLabel(new Date(2025, 7, 20), now)).toContain("2025");
  });

  test("consecutive rows from the same day share one heading", () => {
    const groups = Conversations.byDay(
      [
        at(new Date(2026, 7, 26, 12, 0)),
        at(new Date(2026, 7, 26, 9, 0)),
        at(new Date(2026, 7, 25, 18, 0)),
      ],
      now,
    );

    expect(groups.map((group) => group.label)).toEqual(["Today", "Yesterday"]);
    expect(groups[0].items).toHaveLength(2);
    expect(groups[1].items).toHaveLength(1);
  });

  test("every conversation lands in exactly one group", () => {
    const all = [
      at(new Date(2026, 7, 26, 12, 0)),
      at(new Date(2026, 7, 25, 18, 0)),
      at(new Date(2026, 7, 20, 8, 0)),
    ];

    const grouped = Conversations.byDay(all, now).flatMap((group) => group.items);

    expect(grouped).toHaveLength(all.length);
  });

  test("an empty list produces no headings", () => {
    expect(Conversations.byDay([], now)).toEqual([]);
  });
});
