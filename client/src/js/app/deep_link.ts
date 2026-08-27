import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

/**
 * Pairing links arriving from outside the app.
 *
 * Both paths are wired, and both are needed. `onOpenUrl` reports links that
 * arrive while the app is already running; it does **not** fire when the app is
 * launched cold by the link, which is the ordinary case for somebody who has
 * just walked up to a machine and scanned its code. `getCurrent` covers that.
 *
 * Wiring only `onOpenUrl` produces a deep link that works every time you test it
 * with the app open and silently does nothing for a person opening the app for
 * the first time.
 */
export class DeepLink {
  /**
   * Whether the launch link has already been acted on.
   *
   * Static, because how the process started is one fact about the process, not
   * about whichever screen happens to be mounted. `getCurrent` keeps answering
   * with the launch URL for the life of the app, so a screen that reads it on
   * every mount re-handles the same link forever: pair, return to the list, and
   * the list reads the launch link again and navigates straight back to pairing.
   * Nothing after a cold deep-link launch can ever be reached.
   */
  private static launchHandled = false;

  private stopper: (() => void) | null = null;

  constructor(private readonly onUri: (uri: string) => void) {}

  async start(): Promise<void> {
    try {
      if (!DeepLink.launchHandled) {
        DeepLink.launchHandled = true;

        const initial = await getCurrent();

        if (initial && initial.length > 0) {
          this.onUri(initial[0]);
        }
      }

      this.stopper = await onOpenUrl((urls) => {
        if (urls.length > 0) {
          this.onUri(urls[0]);
        }
      });
    } catch (error) {
      // A desktop build with the scheme not yet registered, or a platform that
      // refuses runtime registration. The rest of the app still works, and the
      // paste field is the way in.
      console.warn("deep links unavailable", error);
    }
  }

  stop(): void {
    if (this.stopper) {
      this.stopper();
      this.stopper = null;
    }
  }
}
