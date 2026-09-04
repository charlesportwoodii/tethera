import { describe, expect, test } from "vitest";
import { Fleet } from "./fleet";
import type { Conversation } from "$bindings/Conversation";
import type { ServerRow } from "$bindings/ServerRow";

// `Timestamp` is epoch milliseconds.
function a_conversation(
  id: string,
  status: string,
  bound: boolean,
  lastActive: number,
): Conversation {
  return {
    id,
    profile: "claude",
    profile_label: "Claude Code",
    title: id,
    preview: status === "blocked" ? "Apply the parameter changes?" : null,
    cwd: "/home/dev",
    workspace: bound ? "ws_1" : null,
    started_at: lastActive,
    last_active: lastActive,
    turn_count: null,
    status,
    has_transcript: true,
    resumable: true,
    binding: bound ? "pn_1" : null,
  } as unknown as Conversation;
}

function a_row(label: string, offline: boolean, held: Conversation[]): ServerRow {
  return {
    entry: {
      server: { id: label, label, app_version: "1", os: "linux", arch: "aarch64" },
      conversations: held,
      last_seen_at: 1,
    },
    link: { kind: offline ? "offline" : "direct", rtt_ms: offline ? null : 38 },
    refusal: null,
  } as unknown as ServerRow;
}

describe("Fleet.states", () => {
  test("draws every session's own status while the machine is answering", () => {
    const row = a_row("noble2", false, [
      a_conversation("a", "blocked", true, 3),
      a_conversation("b", "working", true, 2),
    ]);

    expect(Fleet.states(row)).toEqual(["blocked", "working"]);
  });

  // Every status under a quiet machine is a memory of the last answer. Drawing
  // it filled would claim work may still be going, which nothing on the device
  // can support.
  test("reports every session as offline when the machine is not answering", () => {
    const row = a_row("bastion", true, [
      a_conversation("a", "working", true, 2),
      a_conversation("b", "blocked", true, 1),
    ]);

    expect(Fleet.states(row)).toEqual(["offline", "offline"]);
  });

  test("sorts what needs a person to the front", () => {
    const row = a_row("noble2", false, [
      a_conversation("a", "idle", true, 4),
      a_conversation("b", "blocked", true, 3),
      a_conversation("c", "working", true, 2),
      a_conversation("d", "stalled", true, 1),
    ]);

    expect(Fleet.states(row)).toEqual(["blocked", "stalled", "working", "idle"]);
  });
});

describe("Fleet.attention", () => {
  test("is set when something is waiting on a person", () => {
    const row = a_row("noble2", false, [a_conversation("a", "blocked", true, 1)]);

    expect(Fleet.attention(row)).toBe(true);
  });

  test("is set when something is stuck, which also needs a person", () => {
    const row = a_row("noble2", false, [a_conversation("a", "stalled", true, 1)]);

    expect(Fleet.attention(row)).toBe(true);
  });

  test("is not set for work in progress", () => {
    const row = a_row("noble2", false, [a_conversation("a", "working", true, 1)]);

    expect(Fleet.attention(row)).toBe(false);
  });

  // A machine that is not answering cannot be waiting on anybody: whatever the
  // remembered status says, nothing there can receive an answer.
  test("is not set for a machine that is not answering", () => {
    const row = a_row("bastion", true, [a_conversation("a", "blocked", true, 1)]);

    expect(Fleet.attention(row)).toBe(false);
  });
});

describe("Fleet.sentence", () => {
  test("puts what needs a person first and names it in words", () => {
    const row = a_row("noble2", false, [
      a_conversation("a", "blocked", true, 3),
      a_conversation("b", "working", true, 2),
      a_conversation("c", "working", true, 1),
    ]);

    expect(Fleet.sentence(row)).toBe("1 needs you · 2 working");
  });

  test("counts a stuck session as needing a person too", () => {
    const row = a_row("noble2", false, [
      a_conversation("a", "blocked", true, 2),
      a_conversation("b", "stalled", true, 1),
    ]);

    expect(Fleet.sentence(row)).toBe("2 need you");
  });

  test("says how long a quiet machine has been quiet, not what it was doing", () => {
    const row = a_row("bastion", true, [
      a_conversation("a", "working", true, 2),
      a_conversation("b", "idle", true, 1),
    ]);

    expect(Fleet.sentence(row)).toBe("2 sessions when it went quiet");
  });

  test("is empty for a machine running nothing", () => {
    expect(Fleet.sentence(a_row("pinecrest", false, []))).toBe("");
  });
});

describe("Fleet.waiting", () => {
  test("gathers what needs a person across every machine, newest first", () => {
    const rows = [
      a_row("thalira", false, [a_conversation("a", "blocked", true, 10)]),
      a_row("noble2", false, [
        a_conversation("b", "working", true, 30),
        a_conversation("c", "stalled", true, 20),
      ]),
    ];

    expect(Fleet.waiting(rows).map((held) => held.conversation.id)).toEqual(["c", "a"]);
  });

  test("leaves out a machine that is not answering", () => {
    const rows = [a_row("bastion", true, [a_conversation("a", "blocked", true, 10)])];

    expect(Fleet.waiting(rows)).toEqual([]);
  });
});

describe("Fleet.recent", () => {
  test("interleaves every machine's sessions newest first and caps them", () => {
    const rows = [
      a_row("thalira", false, [
        a_conversation("a", "working", true, 50),
        a_conversation("b", "idle", true, 10),
      ]),
      a_row("noble2", false, [a_conversation("c", "working", true, 30)]),
    ];

    expect(Fleet.recent(rows, 2).map((held) => held.conversation.id)).toEqual(["a", "c"]);
  });

  test("still lists a quiet machine's remembered work, which is all it has", () => {
    const rows = [a_row("bastion", true, [a_conversation("a", "idle", true, 10)])];

    expect(Fleet.recent(rows, 5).map((held) => held.conversation.id)).toEqual(["a"]);
  });
});
