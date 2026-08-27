/** Every mark the system draws. Names are what the thing is, not what it looks like. */
export type IconName =
  | "back"
  | "plus"
  | "terminal"
  | "send"
  | "attach"
  | "chevron"
  | "collapse"
  | "expand"
  | "grip"
  | "scan"
  | "settings"
  | "retry"
  | "download"
  | "check"
  | "close";

export interface IconProps {
  name: IconName;
  /** Edge length in pixels. The stroke stays 1.6 at every size, by design. */
  size?: number;
  /** Given to the accessible name. Omit for a mark that repeats adjacent text. */
  label?: string;
}
