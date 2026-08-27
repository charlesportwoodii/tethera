export interface Attachment {
  id: string;
  name: string;
  /** 0 to 1 while uploading; null once it has landed. */
  progress?: number | null;
}

export interface ComposerProps {
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  /**
   * The agent is mid-turn. The field stays usable — a reply queues — but sending
   * is held until it stops, which is what the placeholder says.
   */
  busy?: boolean;
  /**
   * Lines to grow to before the field scrolls itself. Four or five is the usual
   * ceiling on a phone: unbounded growth eats the transcript it replies to, and
   * with the keyboard up there is little screen left to eat.
   */
  maxRows?: number;
  attachments?: Attachment[];
  /** Absent when the host cannot take uploads — the control disappears rather than greying. */
  onattach?: (() => void) | null;
  onremoveattachment?: (id: string) => void;
  oninput?: (value: string) => void;
  onsend?: (value: string) => void;
}
