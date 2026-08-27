import { describe, expect, test } from "vitest";
import { Refusal } from "./refusal";

describe("Refusal", () => {
  // The machine's enrolment lookup fails closed, so this refusal also arrives
  // when its device table is unreadable. Asserting a revocation would send
  // somebody hunting one that never happened.
  test("a revoked refusal reports what was observed rather than accusing", () => {
    const text = Refusal.text("revoked", "atlas");

    expect(text).toContain("would not accept this device");
    expect(text.toLowerCase()).not.toContain("has revoked");
  });

  // Zero attempts left covers "the guesses are spent" and "no window was open",
  // and the client cannot tell which. Naming a number would be wrong in the
  // second case.
  test("a spent window names the action rather than counting guesses", () => {
    const text = Refusal.text("window_spent", "atlas");

    expect(text).toContain("tethera pair");
    expect(text).not.toMatch(/\d+\s+attempts/);
  });

  test("an unknown close code reports the code and blames neither side", () => {
    const text = Refusal.text({ closed_by_machine: { code: 47 } }, "atlas");

    expect(text).toContain("47");
    expect(text).toContain("newer");
  });

  test("at capacity tells the person to wait rather than to re-pair", () => {
    const text = Refusal.text("at_capacity", "atlas");

    expect(text).toContain("Try again");
    expect(text.toLowerCase()).not.toContain("pair");
  });

  // A variant added to the Rust enum but not here must still produce something
  // a person can act on, rather than "undefined".
  test("an unrecognised outcome still names itself", () => {
    const text = Refusal.text("some_future_reason" as never, "atlas");

    expect(text).toContain("some_future_reason");
  });
});
