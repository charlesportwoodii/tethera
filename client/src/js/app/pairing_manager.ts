import { writable, type Readable, type Writable } from "svelte/store";
import type { BeginOutcome } from "$bindings/BeginOutcome";
import type { PairOutcome } from "$bindings/PairOutcome";
import type { FoundServer } from "$bindings/FoundServer";
import type { ServerEntry } from "$bindings/ServerEntry";
import type { Invoke } from "./server_manager";
import type { Outcome } from "./refusal";

export type PairingState =
  | { step: "idle" }
  | { step: "reaching" }
  | { step: "found"; found: FoundServer; attemptsLeft: number | null }
  | { step: "paired"; entry: ServerEntry }
  | { step: "refused"; outcome: Outcome; label: string };

/**
 * One pairing attempt.
 *
 * The enrolment stream lives in Rust and stays open across the person typing,
 * because the machine counts attempts against a window. Only the `found` step
 * holds it; every other step has closed it.
 */
export class PairingManager {
  private readonly stateStore: Writable<PairingState>;
  public readonly state: Readable<PairingState>;

  constructor(private readonly invoke: Invoke) {
    this.stateStore = writable({ step: "idle" });
    this.state = { subscribe: this.stateStore.subscribe };
  }

  async begin(uri: string): Promise<void> {
    this.stateStore.set({ step: "reaching" });

    let outcome: BeginOutcome;

    try {
      outcome = (await this.invoke("pair_begin", { uri })) as BeginOutcome;
    } catch (error) {
      // An Err at the boundary is a malformed link or a fault, never one of the
      // expected refusals - those arrive as outcomes.
      this.stateStore.set({
        step: "refused",
        outcome: "unreachable",
        label: String(error),
      });

      return;
    }

    if (typeof outcome === "object" && "found" in outcome) {
      this.stateStore.set({ step: "found", found: outcome.found, attemptsLeft: null });

      return;
    }

    if (typeof outcome === "object" && "already_paired" in outcome) {
      this.stateStore.set({ step: "paired", entry: outcome.already_paired });

      return;
    }

    this.stateStore.set({ step: "refused", outcome, label: this.label() });
  }

  async submit(code: string): Promise<void> {
    const outcome = (await this.invoke("pair_submit", { code })) as PairOutcome;

    if (typeof outcome === "object" && "paired" in outcome) {
      this.stateStore.set({ step: "paired", entry: outcome.paired });

      return;
    }

    // The only outcome that leaves the attempt open. The count comes back on
    // the state so the screen can say how many are left without inventing one.
    if (typeof outcome === "object" && "wrong_code" in outcome) {
      const left = outcome.wrong_code.attempts_left;

      this.stateStore.update((held) =>
        held.step === "found" ? { ...held, attemptsLeft: left } : held,
      );

      return;
    }

    this.stateStore.set({ step: "refused", outcome, label: this.label() });
  }

  async cancel(): Promise<void> {
    await this.invoke("pair_cancel");
    this.stateStore.set({ step: "idle" });
  }

  /** The machine's own name where one is known, so a refusal can name it. */
  private label(): string {
    let label = "that machine";

    const stop = this.stateStore.subscribe((held) => {
      if (held.step === "found") {
        label = held.found.server.label;
      }
    });
    stop();

    return label;
  }
}
