/**
 * A textarea that grows with its content, to a cap, then scrolls itself.
 *
 * The cap matters as much as the growth: on a phone with the keyboard up there
 * is very little screen left, and an unbounded box eats the transcript it exists
 * to reply to.
 */

export interface GrowMetrics {
  /** What the content wants, from `scrollHeight` with the height released. */
  contentHeight: number;
  /** Resolved line height in pixels. */
  lineHeight: number;
  /** Padding and border — the part of the box that is not lines of text. */
  chrome: number;
  /** Lines to show before scrolling. */
  maxRows: number;
}

export interface GrowResult {
  height: number;
  /** Whether the box has more content than it can show. */
  scrolls: boolean;
}

/**
 * The height to set, and whether the box is now scrolling.
 *
 * Pure so the arithmetic is testable: a layout-less DOM reports zero for
 * `scrollHeight`, so a test driving the real element proves nothing.
 */
export function growHeight(metrics: GrowMetrics): GrowResult {
  const { contentHeight, lineHeight, chrome, maxRows } = metrics;

  const line = lineHeight > 0 ? lineHeight : 18;
  const rows = maxRows > 0 ? maxRows : 1;
  const oneRow = line + chrome;
  const cap = line * rows + chrome;

  // A field that has never been typed in reports whatever the browser felt like;
  // clamping up to one row stops it collapsing to nothing.
  const wanted = Math.max(contentHeight, oneRow);

  return { height: Math.min(wanted, cap), scrolls: wanted > cap };
}

/** Read what a textarea needs, releasing the height so the content can speak. */
export function measure(el: HTMLTextAreaElement, maxRows: number): GrowMetrics {
  const style = getComputedStyle(el);
  const lineHeight = Number.parseFloat(style.lineHeight);
  const paddingTop = Number.parseFloat(style.paddingTop) || 0;
  const paddingBottom = Number.parseFloat(style.paddingBottom) || 0;
  const borderTop = Number.parseFloat(style.borderTopWidth) || 0;
  const borderBottom = Number.parseFloat(style.borderBottomWidth) || 0;

  // Released first: with a height set, scrollHeight reports the height rather
  // than the content, and the box can only ever grow.
  const previous = el.style.height;
  el.style.height = "auto";
  const contentHeight = el.scrollHeight;
  el.style.height = previous;

  return {
    contentHeight,
    // A computed line height of "normal" parses as NaN.
    lineHeight: Number.isFinite(lineHeight) ? lineHeight : 0,
    chrome: paddingTop + paddingBottom + borderTop + borderBottom,
    maxRows,
  };
}

/**
 * Svelte action. Resizes on input and whenever the caller says the value moved,
 * which matters because the value is controlled: text arriving from outside the
 * field has to resize it too.
 */
export function autogrow(el: HTMLTextAreaElement, maxRows = 5) {
  let rows = maxRows;

  function apply() {
    const { height, scrolls } = growHeight(measure(el, rows));
    el.style.height = height + "px";
    el.style.overflowY = scrolls ? "auto" : "hidden";
  }

  apply();
  el.addEventListener("input", apply);

  return {
    update(next = 5) {
      rows = next;
      apply();
    },
    destroy() {
      el.removeEventListener("input", apply);
    },
  };
}
