import { describe, expect, test, vi } from "vitest";
import { get } from "svelte/store";
import { SessionManager } from "./session_manager";
import type { AgentProfile } from "$bindings/AgentProfile";

function aProfile(id: string, label: string): AgentProfile {
  return {
    id,
    label,
    description: null,
    version: "2.1.4",
    supports_resume: true,
    provides_transcript: true,
  } as unknown as AgentProfile;
}

const CLAUDE = aProfile("claude", "Claude Code");
const CODEX = aProfile("codex", "Codex");

function invoking(profiles: AgentProfile[], canStart = true, recent: string[] = []) {
  return vi.fn().mockImplementation((command: string) => {
    if (command === "list_agent_profiles") {
      return Promise.resolve(profiles);
    }

    if (command === "can_start_sessions") {
      return Promise.resolve(canStart);
    }

    if (command === "recent_cwds") {
      return Promise.resolve(recent);
    }

    return Promise.resolve(null);
  });
}

describe("SessionManager", () => {
  test("choosing a machine loads what it can run", async () => {
    const manager = new SessionManager(invoking([CLAUDE, CODEX]));

    await manager.chooseServer("sv_atlas");

    expect(get(manager.profiles)).toHaveLength(2);
    expect(get(manager.canStart)).toBe(true);
  });

  // Switching machines is switching catalogs. A `ProfileId` belongs to one
  // machine, so keeping it would hand the next machine an id it has never heard
  // of and the start would be refused for a reason nothing on screen explains.
  test("changing machine clears the harness", async () => {
    const manager = new SessionManager(invoking([CLAUDE, CODEX]));
    await manager.chooseServer("sv_atlas");
    manager.chooseProfile(CLAUDE);

    await manager.chooseServer("sv_bramble");

    expect(get(manager.draft).profile).toBeNull();
  });

  test("a machine offering one harness pre-selects it", async () => {
    const manager = new SessionManager(invoking([CLAUDE]));

    await manager.chooseServer("sv_atlas");

    expect(get(manager.draft).profile?.id).toBe("claude");
  });

  test("a machine that cannot start sessions says so", async () => {
    const manager = new SessionManager(invoking([CLAUDE], false));

    await manager.chooseServer("sv_atlas");

    expect(get(manager.canStart)).toBe(false);
  });

  test("a start needs a machine, a harness and a directory", () => {
    const base = { serverId: null, profile: null, cwd: "", prompt: "" };

    expect(SessionManager.isComplete(base)).toBe(false);
    expect(SessionManager.isComplete({ ...base, serverId: "sv_atlas" })).toBe(false);
    expect(
      SessionManager.isComplete({ ...base, serverId: "sv_atlas", profile: CLAUDE }),
    ).toBe(false);
    expect(
      SessionManager.isComplete({
        ...base,
        serverId: "sv_atlas",
        profile: CLAUDE,
        cwd: "/home/charl",
      }),
    ).toBe(true);
  });

  // Whitespace is not a directory. Accepting it would open a pane somewhere the
  // person did not choose.
  test("a directory of spaces is not a directory", () => {
    expect(
      SessionManager.isComplete({
        serverId: "sv_atlas",
        profile: CLAUDE,
        cwd: "   ",
        prompt: "",
      }),
    ).toBe(false);
  });

  test("start hands the machine the ids it gave us", async () => {
    const invoke = invoking([CLAUDE]);
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");
    manager.setCwd("/home/charl/projects/tethera");
    manager.setPrompt("Read the pairing contract");

    await manager.start();

    expect(invoke).toHaveBeenCalledWith("start_conversation", {
      id: "sv_atlas",
      profile: "claude",
      cwd: "/home/charl/projects/tethera",
      prompt: "Read the pairing contract",
    });
  });

  // An empty box is no first message, not an empty one. Sending "" would make
  // the agent answer nothing.
  test("an empty first message is sent as absent", async () => {
    const invoke = invoking([CLAUDE]);
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");
    manager.setCwd("/home/charl");
    manager.setPrompt("   ");

    await manager.start();

    expect(invoke).toHaveBeenCalledWith(
      "start_conversation",
      expect.objectContaining({ prompt: null }),
    );
  });

  test("an incomplete draft starts nothing", async () => {
    const invoke = invoking([CLAUDE]);
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");
    await manager.start();

    expect(invoke).not.toHaveBeenCalledWith("start_conversation", expect.anything());
  });

  // A machine that will not answer where it has been worked can still start a
  // session. Letting that failure through would empty the harness list beside it
  // and make the whole screen look broken.
  test("a machine that will not list its directories still offers its harnesses", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "list_agent_profiles") return Promise.resolve([CLAUDE, CODEX]);
      if (command === "can_start_sessions") return Promise.resolve(true);
      if (command === "recent_cwds") return Promise.reject(new Error("no route"));

      return Promise.resolve(null);
    });
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");

    expect(get(manager.profiles)).toHaveLength(2);
    expect(get(manager.recent)).toEqual([]);
  });

  // The preview describes one machine, one harness and one directory. A late
  // answer applied to a changed form would name a workspace this start will not
  // use, which is worse than showing nothing.
  test("a preview that arrives after the directory changed is discarded", async () => {
    // Held in a box rather than a bare `let`: assigning inside the executor
    // closure leaves TypeScript narrowing the variable to `never` at the call
    // site below.
    const gate: { release: ((value: unknown) => void) | null } = { release: null };
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "list_agent_profiles") return Promise.resolve([CLAUDE]);
      if (command === "can_start_sessions") return Promise.resolve(true);
      if (command === "recent_cwds") return Promise.resolve([]);
      if (command === "preview_conversation") {
        return new Promise((resolve) => {
          gate.release = resolve;
        });
      }

      return Promise.resolve(null);
    });
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");
    manager.setCwd("/home/charl/one");

    const asked = manager.refreshPreview();

    manager.setCwd("/home/charl/two");
    gate.release?.({
      workspace_label: "tethera-4",
      tab_label: "claude",
      creates_workspace: true,
      will_have_transcript: true,
    });
    await asked;

    expect(get(manager.preview)).toBeNull();
  });

  test("a refused start is reported rather than swallowed", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "list_agent_profiles") return Promise.resolve([CLAUDE]);
      if (command === "can_start_sessions") return Promise.resolve(true);

      return Promise.reject(new Error("this machine cannot read agent transcripts yet"));
    });
    const manager = new SessionManager(invoke);

    await manager.chooseServer("sv_atlas");
    manager.setCwd("/home/charl");
    await manager.start();

    const state = get(manager.state);

    expect(state.step).toBe("failed");
    expect(state.step === "failed" && state.reason).toContain("cannot read agent transcripts");
  });
});
