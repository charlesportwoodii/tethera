import { beforeEach, describe, expect, test, vi } from "vitest";

const current = vi.fn();
const onOpen = vi.fn();

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  getCurrent: () => current(),
  onOpenUrl: (handler: (urls: string[]) => void) => onOpen(handler),
}));

const { DeepLink } = await import("./deep_link");

const LAUNCH = "tethera://pair?s=sv_atlas&n=atlas";

describe("DeepLink", () => {
  beforeEach(() => {
    current.mockReset().mockResolvedValue([LAUNCH]);
    onOpen.mockReset().mockResolvedValue(() => {});

    // The guard is per process, and each test is its own process's worth of
    // behaviour.
    (DeepLink as unknown as { launchHandled: boolean }).launchHandled = false;
  });

  test("a cold launch link is handled", async () => {
    const seen: string[] = [];

    await new DeepLink((uri) => seen.push(uri)).start();

    expect(seen).toEqual([LAUNCH]);
  });

  /**
   * The bug this exists for: `getCurrent` keeps answering with the launch URL
   * for the life of the app. The screen that reads it is the server list, which
   * is also where pairing returns to — so re-reading it sends the app straight
   * back to pairing, forever, and nothing after a cold deep-link launch can be
   * reached.
   */
  test("the launch link is acted on once per process, not once per screen", async () => {
    const seen: string[] = [];
    const handle = (uri: string) => seen.push(uri);

    await new DeepLink(handle).start();
    await new DeepLink(handle).start();
    await new DeepLink(handle).start();

    expect(seen).toEqual([LAUNCH]);
    expect(current).toHaveBeenCalledTimes(1);
  });

  // A link that arrives while the app is running is a fresh act by a person,
  // however many times they do it. Only the launch value is one-shot.
  test("a link arriving while running is handled every time", async () => {
    const seen: string[] = [];
    // Boxed: assigning inside the mock's closure leaves TypeScript narrowing a
    // bare `let` to `never` at the call site.
    const held: { deliver: ((urls: string[]) => void) | null } = { deliver: null };

    onOpen.mockImplementation(async (handler: (urls: string[]) => void) => {
      held.deliver = handler;

      return () => {};
    });

    await new DeepLink((uri) => seen.push(uri)).start();

    held.deliver?.(["tethera://pair?s=sv_bramble&n=bramble"]);
    held.deliver?.(["tethera://pair?s=sv_keel&n=keel"]);

    expect(seen).toHaveLength(3);
  });

  test("a platform with no deep links does not stop the app", async () => {
    current.mockRejectedValue(new Error("scheme not registered"));

    await expect(new DeepLink(() => {}).start()).resolves.toBeUndefined();
  });
});
