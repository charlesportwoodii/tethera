import type { BeginOutcome } from "$bindings/BeginOutcome";
import type { PairOutcome } from "$bindings/PairOutcome";

export type Outcome = BeginOutcome | PairOutcome;

/**
 * What to say when a pairing attempt did not end in a pairing.
 *
 * Two of these sentences are deliberately not the obvious one, and both are
 * about not asserting more than the machine actually told us.
 */
export class Refusal {
  /** ts-rs renders a unit variant as a bare string and a data variant as a
   * single-key object, so a refusal is one or the other. */
  static text(outcome: Outcome, label: string): string {
    if (typeof outcome === "string") {
      return Refusal.forName(outcome, label);
    }

    if ("id_mismatch" in outcome) {
      return `That code belongs to a different machine than the one that answered. Check you scanned ${label}'s screen.`;
    }

    if ("closed_by_machine" in outcome) {
      const code = outcome.closed_by_machine.code;

      return `${label} refused this connection for a reason this app does not recognise (code ${code}). It may be newer than this client.`;
    }

    return `Pairing with ${label} did not complete.`;
  }

  private static forName(name: string, label: string): string {
    switch (name) {
      case "window_closed":
        return `No pairing window is open on ${label}. Run \`tethera pair\` on the machine, then scan again.`;

      // Deliberately not an accusation. The machine's enrolment lookup fails
      // closed, so this refusal also arrives when its device table is
      // unreadable. Asserting a revocation would send somebody hunting one that
      // never happened.
      case "revoked":
        return `${label} would not accept this device. If you did not revoke it, check that machine.`;

      case "no_common_version":
        return `${label} and this app do not share a protocol version. One of them needs updating.`;

      case "at_capacity":
        return `${label} is already handling as many connections as it will. Try again shortly.`;

      case "unreachable":
        return `Nothing answered at ${label}'s address.`;

      // Deliberately not a count of guesses. Zero attempts left means the
      // attempts are spent *or* that no window was open when the code arrived,
      // and this app cannot tell which. Both are fixed by the same action.
      case "window_spent":
        return `This pairing window is finished. Run \`tethera pair\` on ${label} to open a new one.`;

      case "link_lost":
        return `The connection to ${label} dropped part way through. Nothing was paired.`;

      default:
        return `Pairing with ${label} did not complete (${name}).`;
    }
  }
}
