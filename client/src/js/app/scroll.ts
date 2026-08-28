/**
 * Where a reader is in a transcript, as two decisions.
 *
 * Both are pure over the box's own numbers so the thresholds and the reasons
 * for them sit in one place rather than inside a scroll handler, where the only
 * way to check either is to scroll a phone.
 */
export class Scroll {
  /**
   * How near the top history is asked for.
   *
   * Not zero. A page takes a round trip to the machine and a read of a file
   * that is tens of megabytes, so a fetch that starts when the reader arrives
   * at the top is a fetch they watch. Roughly a third of a phone screen, which
   * is far enough ahead to hide the wait and near enough that a passing scroll
   * does not trigger it.
   */
  static readonly NEAR_TOP = 240;

  /**
   * How far from the bottom still counts as watching the tail.
   *
   * Arriving turns scroll the view only from inside this. Yanking somebody down
   * while they are reading history is the single most annoying thing a
   * transcript can do.
   */
  static readonly SLACK = 80;

  /** Whether the reader is close enough to the top to want the page before it. */
  static atTop(scrollTop: number): boolean {
    return scrollTop < this.NEAR_TOP;
  }

  /**
   * Whether the reader is watching the tail.
   *
   * A box shorter than its own viewport has no slack at all, which makes this
   * true — and it has to be: nothing scrolls, so there is no gesture that would
   * ever turn following back on.
   */
  static following(scrollTop: number, scrollHeight: number, clientHeight: number): boolean {
    return scrollHeight - scrollTop - clientHeight < this.SLACK;
  }
}
