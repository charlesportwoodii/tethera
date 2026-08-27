/** The two things a workspace screen can show. */
export type WorkspaceView = "chat" | "terminal";

export interface ViewToggleProps {
  view: WorkspaceView;
  /**
   * Absent when this workspace has no readable transcript.
   *
   * `Conversation.has_transcript` answers this. A toggle offering a chat view
   * that turns out to be empty is the control-that-refuses-on-press failure, so
   * the screen goes straight to the terminal instead.
   */
  chatAvailable?: boolean;
  /** Marks the chat side when something is waiting there. */
  chatBadge?: "waiting" | "working" | null;
  onchange?: (view: WorkspaceView) => void;
}
