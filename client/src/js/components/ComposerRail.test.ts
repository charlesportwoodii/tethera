import { describe, expect, test, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import ComposerRail from "./ComposerRail.svelte";

describe("ComposerRail", () => {
  // The key shows where it goes, not where you are. Labelled with the current
  // mode it reads as the disabled half of a segmented control.
  test("in chat the mode key offers the terminal", () => {
    const { getByLabelText } = render(ComposerRail, { props: { mode: "chat" } });

    expect(getByLabelText("Open the terminal")).toBeTruthy();
  });

  test("in the terminal the mode key offers chat", () => {
    const { getByLabelText } = render(ComposerRail, { props: { mode: "terminal" } });

    expect(getByLabelText("Back to chat")).toBeTruthy();
  });

  test("the control keys belong to the terminal and appear nowhere else", () => {
    const chat = render(ComposerRail, { props: { mode: "chat", onkey: vi.fn() } });

    expect(chat.queryByText("^C")).toBeNull();

    const term = render(ComposerRail, { props: { mode: "terminal", onkey: vi.fn() } });

    expect(term.getByText("^C")).toBeTruthy();
  });

  // Intent and modifier, never bytes. A rail that emitted "^C" as a string
  // would push the encoding onto whoever caught it, which is the decision the
  // protocol is arranged to keep on the server.
  test("a control key sends its intent and its modifier", () => {
    const onkey = vi.fn();

    const { getByText } = render(ComposerRail, {
      props: { mode: "terminal", onkey },
    });

    void fireEvent.click(getByText("^C"));

    expect(onkey).toHaveBeenCalledWith({ char: "c" }, 4);
  });

  test("an arrow sends a named key with no modifier", () => {
    const onkey = vi.fn();

    const { getByText } = render(ComposerRail, {
      props: { mode: "terminal", onkey },
    });

    void fireEvent.click(getByText("↑"));

    expect(onkey).toHaveBeenCalledWith("up", 0);
  });

  // A machine that will not take input gets no keys rather than keys that do
  // nothing when pressed.
  test("no keys are drawn when the machine will not take input", () => {
    const { queryByText } = render(ComposerRail, {
      props: { mode: "terminal", onkey: null },
    });

    expect(queryByText("^C")).toBeNull();
  });

  test("the floorplan key is absent when the machine reports no geometry", () => {
    const { queryByLabelText } = render(ComposerRail, {
      props: { mode: "chat", onmap: null },
    });

    expect(queryByLabelText("Pane layout")).toBeNull();
  });

  // A badge reading "1" tells nobody anything, and it is the common case.
  test("the pane count is badged only once there is more than one", () => {
    const alone = render(ComposerRail, {
      props: { mode: "chat", onmap: vi.fn(), mapBadge: 1 },
    });

    expect(alone.queryByText("1")).toBeNull();

    const several = render(ComposerRail, {
      props: { mode: "chat", onmap: vi.fn(), mapBadge: 4 },
    });

    expect(several.getByText("4")).toBeTruthy();
  });

  // Absent, not disabled. A machine with no window to move — a pty backend —
  // has nothing to focus, and neither does a session that is not running in a
  // pane right now.
  test("the focus key is absent when there is nothing to focus", () => {
    const { queryByLabelText } = render(ComposerRail, {
      props: { mode: "chat", onfocus: null },
    });

    expect(queryByLabelText(/Show this session/)).toBeNull();
  });

  test("pressing the focus key asks the desk to move", async () => {
    const onfocus = vi.fn(async () => {});

    const { getByLabelText } = render(ComposerRail, {
      props: { mode: "chat", onfocus },
    });

    await fireEvent.click(getByLabelText(/Show this session/));

    expect(onfocus).toHaveBeenCalled();
  });

  // The result of this press is on a screen somebody is not looking at, so the
  // button has to say it happened. Without it a press that worked and a press
  // that did nothing are the same experience.
  test("the focus key confirms, because its result is on another screen", async () => {
    const onfocus = vi.fn(async () => {});

    const { getByLabelText, container } = render(ComposerRail, {
      props: { mode: "chat", onfocus },
    });

    await fireEvent.click(getByLabelText(/Show this session/));

    expect(container.querySelector(".done")).not.toBeNull();
  });

  // The terminal already moves the desk when a tab is tapped, so a second
  // control doing the same thing there is a second way to ask one question.
  test("the focus key is chat only", () => {
    const { queryByLabelText } = render(ComposerRail, {
      props: { mode: "terminal", onfocus: vi.fn() },
    });

    expect(queryByLabelText(/Show this session/)).toBeNull();
  });
});
