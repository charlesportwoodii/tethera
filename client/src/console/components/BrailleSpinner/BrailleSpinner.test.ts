import { afterEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import BrailleSpinner from "./BrailleSpinner.svelte";

const BRAILLE = /[\u2800-\u28FF]/;

afterEach(() => vi.restoreAllMocks());

describe("BrailleSpinner", () => {
  it("renders a braille cell", () => {
    const { getByRole } = render(BrailleSpinner, { props: {} });
    expect(getByRole("img").textContent).toMatch(BRAILLE);
  });

  it("starts on a different frame for a different offset", () => {
    const a = render(BrailleSpinner, { props: { offset: 0 } });
    const first = a.getByRole("img").textContent;
    a.unmount();

    const b = render(BrailleSpinner, { props: { offset: 3 } });
    // Without this two spinners on one screen march in lockstep, which reads as
    // one animation rather than two agents.
    expect(b.getByRole("img").textContent).not.toBe(first);
  });

  it("animates by interval", () => {
    const spy = vi.spyOn(globalThis, "setInterval");
    render(BrailleSpinner, { props: { interval: 120 } });
    expect(spy).toHaveBeenCalledWith(expect.any(Function), 120);
  });

  it("can be hidden from assistive tech when adjacent text already says it", () => {
    const { container } = render(BrailleSpinner, { props: { label: null } });
    expect(container.querySelector(".tc-braille")).toHaveAttribute("aria-hidden", "true");
  });

  it("does not start a timer when motion is reduced", () => {
    vi.spyOn(window, "matchMedia").mockReturnValue({
      matches: true,
      media: "(prefers-reduced-motion: reduce)",
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    } as unknown as MediaQueryList);
    const spy = vi.spyOn(globalThis, "setInterval");
    const { container } = render(BrailleSpinner, { props: {} });
    expect(spy).not.toHaveBeenCalled();
    expect(container.querySelector(".tc-braille")).toHaveAttribute("data-static", "true");
  });
});
