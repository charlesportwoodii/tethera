import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import NavBar from "./NavBar.svelte";

describe("NavBar", () => {
  it("puts the title in a heading", () => {
    const { getByRole } = render(NavBar, { props: { title: "Servers" } });
    expect(getByRole("heading", { name: "Servers" })).toBeInTheDocument();
  });

  it("omits the back control on a root screen", () => {
    const { queryByRole } = render(NavBar, { props: { title: "Servers" } });
    expect(queryByRole("button", { name: "Back" })).toBeNull();
  });

  it("calls onback when there is somewhere to go", async () => {
    const onback = vi.fn();
    const { getByRole } = render(NavBar, { props: { title: "atlas", onback } });
    await userEvent.click(getByRole("button", { name: "Back" }));
    expect(onback).toHaveBeenCalledOnce();
  });

  it("omits the subtitle rather than rendering an empty line", () => {
    const { container } = render(NavBar, { props: { title: "Servers" } });
    expect(container.querySelector(".tc-nav__sub")).toBeNull();
  });
});
