import type { AssetId } from "$bindings/AssetId";
import type { Part } from "$bindings/Part";

export interface PartViewProps {
  part: Part;
  /** Already formatted; only used by the file part. */
  at?: string | null;
  /** How long a question has been waiting — already formatted. */
  waiting?: string | null;
  /** Diffs and tool calls stay folded until the caller says otherwise. */
  expanded?: boolean;
  /**
   * Opens the question flow.
   *
   * There is no onanswer, and that is the point: the transcript announces a
   * question, it never answers one. QuestionFlow is the single place an answer
   * is composed and sent, so there is one path to get the fingerprint, the
   * per-ask ordering and the multi-select rules right instead of two.
   *
   * Null leaves the announcement with no way in, which is what a caller that
   * cannot answer — a replay, a pane that has moved on — should pass.
   */
  onexpandquestion?: (() => void) | null;
  /**
   * A data or object URL for an image the caller has already fetched.
   *
   * Null means either not an image or not loaded yet. Both fall back to the
   * card, which is the honest thing to show while bytes are still moving: an
   * image has to arrive whole to decode, so there is no partial picture.
   */
  imageUrl?: string | null;
  onopenfile?: (asset: AssetId, name: string) => void;
  /**
   * A link inside agent prose. Absent leaves links inert, which is the safe
   * default in a webview that has nowhere to go back to.
   */
  onlink?: ((href: string) => void) | null;
  ontool?: (name: string) => void;
  ontoggle?: () => void;
}
