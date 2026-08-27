import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import TodoList from "./TodoList.svelte";
import type { TodoItem } from "$bindings/TodoItem";

const ITEMS: TodoItem[] = [
  { text: "Read the pairing contract", status: "done" },
  { text: "Find how the old app routed it", status: "done" },
  { text: "Decide who owns the rewrite", status: "in_progress" },
  { text: "Write the test first", status: "pending" },
];

describe("TodoList", () => {
  it("counts what is done against the whole plan", () => {
    const { getByText } = render(TodoList, { props: { items: ITEMS } });
    expect(getByText("2 of 4")).toBeInTheDocument();
  });

  it("carries each status as data so a parent can read the plan", () => {
    const { getAllByRole } = render(TodoList, { props: { items: ITEMS } });
    const statuses = getAllByRole("listitem").map((n) => n.getAttribute("data-status"));
    expect(statuses).toEqual(["done", "done", "in_progress", "pending"]);
  });

  it("is a list, so its length is announced", () => {
    const { getAllByRole } = render(TodoList, { props: { items: ITEMS } });
    expect(getAllByRole("listitem")).toHaveLength(4);
  });

  it("copes with an empty plan without dividing by it", () => {
    const { getByText } = render(TodoList, { props: { items: [] } });
    expect(getByText("0 of 0")).toBeInTheDocument();
  });
});
