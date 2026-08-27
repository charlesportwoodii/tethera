export interface MarkdownProps {
  /** The agent's own text, verbatim. */
  source: string;
  /**
   * Called when a link is tapped, with its URL.
   *
   * Absent means links are inert. That is the default on purpose: a link that
   * navigates a Tauri webview away from the app is a one-way trip with no back
   * button, so the host has to say what opening one means — normally handing it
   * to the system browser.
   */
  onlink?: ((href: string) => void) | null;
}
