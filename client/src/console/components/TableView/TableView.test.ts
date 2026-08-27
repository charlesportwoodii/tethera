import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import TableView from "./TableView.svelte";

describe("TableView", () => {
  it("renders headers as column headers, not as a first row", () => {
    const { getAllByRole } = render(TableView, {
      props: { columns: ["test", "result"], rows: [["host_not_path", "FAILED"]] },
    });
    const headers = getAllByRole("columnheader");
    expect(headers).toHaveLength(2);
    expect(headers[0]).toHaveAttribute("scope", "col");
  });

  it("renders every cell", () => {
    const { getByText } = render(TableView, {
      props: {
        columns: ["test", "result"],
        rows: [
          ["host_not_path", "FAILED"],
          ["claude_default", "ok"],
        ],
      },
    });
    expect(getByText("host_not_path")).toBeInTheDocument();
    expect(getByText("claude_default")).toBeInTheDocument();
  });

  it("survives a ragged row rather than throwing", () => {
    const { getAllByRole } = render(TableView, {
      props: { columns: ["a", "b", "c"], rows: [["1"]] },
    });
    // The agent produced it; refusing to draw it would lose the content.
    expect(getAllByRole("cell")).toHaveLength(1);
  });

  it("takes a caption when the table needs naming", () => {
    const { getByText } = render(TableView, {
      props: { columns: ["a"], rows: [["1"]], caption: "Test results" },
    });
    expect(getByText("Test results")).toBeInTheDocument();
  });
});
