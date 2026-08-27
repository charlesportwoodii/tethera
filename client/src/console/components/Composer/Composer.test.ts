import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Composer from "./Composer.svelte";

describe("Composer", () => {
  it("cannot send an empty message", () => {
    const { getByRole } = render(Composer, { props: { value: "" } });
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("treats whitespace as empty", () => {
    const { getByRole } = render(Composer, { props: { value: "   " } });
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("sends what is in the field", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "1", onsend } });
    await userEvent.click(getByRole("button", { name: "Send" }));
    expect(onsend).toHaveBeenCalledWith("1");
  });

  it("does not send on Enter — a soft keyboard has no other way to make a newline", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Enter}");
    // Enter-sends does not merely prefer one action on a phone: it makes a
    // newline unreachable in a field built to grow to five lines.
    expect(onsend).not.toHaveBeenCalled();
  });

  it("does not send on shift+Enter either", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
    expect(onsend).not.toHaveBeenCalled();
  });

  it("sends on ctrl+Enter, for a hardware keyboard", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(onsend).toHaveBeenCalledWith("hello");
  });

  it("sends on meta+Enter too", async () => {
    const onsend = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "hello", onsend } });
    getByRole("textbox").focus();
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onsend).toHaveBeenCalledWith("hello");
  });

  it("is a textarea, so a long message wraps instead of scrolling sideways", () => {
    const { getByRole } = render(Composer, { props: { value: "" } });
    const field = getByRole("textbox");
    expect(field.tagName).toBe("TEXTAREA");
    // Starts at one line and grows from there rather than reserving the cap.
    expect(field).toHaveAttribute("rows", "1");
  });

  it("keeps a newline typed with Enter, rather than swallowing it", async () => {
    const oninput = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "", oninput } });
    const field = getByRole("textbox");
    field.focus();
    await userEvent.keyboard("a{Enter}b");
    const typed = oninput.mock.calls.map((c) => c[0]).join("|");
    expect(typed).toContain("\n");
  });

  it("hides attach entirely when the host cannot take uploads", () => {
    const { queryByRole } = render(Composer, { props: { value: "" } });
    // Absent, not disabled: a dead control promises something the machine cannot do.
    expect(queryByRole("button", { name: "Attach a file" })).toBeNull();
  });

  it("reports typing to the caller rather than keeping its own copy", async () => {
    const oninput = vi.fn();
    const { getByRole } = render(Composer, { props: { value: "", oninput } });
    await userEvent.type(getByRole("textbox"), "a");
    expect(oninput).toHaveBeenCalledWith("a");
  });

  it("holds the send while the agent is mid-turn, and says why", () => {
    const { getByRole } = render(Composer, { props: { value: "next", busy: true } });
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
    expect(getByRole("textbox")).toHaveAttribute(
      "placeholder",
      "Reply — it will queue until the agent stops",
    );
  });

  it("leaves the field usable while busy, so a reply can be typed ahead", () => {
    const { getByRole } = render(Composer, { props: { value: "", busy: true } });
    expect(getByRole("textbox")).toBeEnabled();
  });

  it("holds the send until an upload has landed", () => {
    const { getByRole } = render(Composer, {
      props: {
        value: "here it is",
        onattach: () => {},
        attachments: [{ id: "a", name: "shot.png", progress: 0.6 }],
      },
    });
    // Sending mid-upload sends a message referring to a file the host does not have.
    expect(getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("sends once the upload is done", () => {
    const { getByRole } = render(Composer, {
      props: {
        value: "here it is",
        onattach: () => {},
        attachments: [{ id: "a", name: "shot.png" }],
      },
    });
    expect(getByRole("button", { name: "Send" })).toBeEnabled();
  });

  it("lights the clip while anything is attached", () => {
    const { getByRole } = render(Composer, {
      props: { value: "", onattach: () => {}, attachments: [{ id: "a", name: "a.png" }] },
    });
    expect(getByRole("button", { name: "Attach a file" }).className).toContain("is-on");
  });

  it("removes an attachment by id", async () => {
    const onremoveattachment = vi.fn();
    const { getByRole } = render(Composer, {
      props: {
        value: "",
        onattach: () => {},
        attachments: [{ id: "log-1", name: "nat-punch.log" }],
        onremoveattachment,
      },
    });
    await userEvent.click(getByRole("button", { name: "Remove nat-punch.log" }));
    expect(onremoveattachment).toHaveBeenCalledWith("log-1");
  });
});
