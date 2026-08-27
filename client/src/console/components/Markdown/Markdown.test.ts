import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import Markdown from "./Markdown.svelte";

describe("Markdown", () => {
  it("renders bold and emphasis as elements, not asterisks", () => {
    const { container } = render(Markdown, { props: { source: "a **b** and *c*" } });
    expect(container.querySelector("strong")?.textContent).toBe("b");
    expect(container.querySelector("em")?.textContent).toBe("c");
    expect(container.textContent).not.toContain("**");
  });

  it("renders an inline code span", () => {
    const { container } = render(Markdown, { props: { source: "run `cargo test`" } });
    expect(container.querySelector("code")?.textContent).toBe("cargo test");
  });

  it("renders a fenced block, keeping its whitespace", () => {
    const { container } = render(Markdown, {
      props: { source: "```rust\nfn main() {\n    ok();\n}\n```" },
    });
    const pre = container.querySelector("pre");
    expect(pre?.textContent).toContain("    ok();");
  });

  it("labels a fenced block with its language", () => {
    const { getByText } = render(Markdown, { props: { source: "```rust\nfn main() {}\n```" } });
    expect(getByText("rust")).toBeInTheDocument();
  });

  it("renders a heading as a heading for assistive tech", () => {
    const { getByRole } = render(Markdown, { props: { source: "### Routing" } });
    expect(getByRole("heading", { name: "Routing", level: 3 })).toBeInTheDocument();
  });

  it("renders lists as lists", () => {
    const bullets = render(Markdown, { props: { source: "- one\n- two" } });
    expect(bullets.getAllByRole("listitem")).toHaveLength(2);
    expect(bullets.container.querySelector("ol")).toBeNull();
    bullets.unmount();

    const numbered = render(Markdown, { props: { source: "1. one\n2. two" } });
    expect(numbered.container.querySelector("ol")).toBeInTheDocument();
  });

  it("renders a blockquote and a rule", () => {
    const { container } = render(Markdown, { props: { source: "> quoted\n\n---" } });
    expect(container.querySelector("blockquote")?.textContent).toContain("quoted");
    expect(container.querySelector("hr")).toBeInTheDocument();
  });
});

describe("Markdown — links", () => {
  it("renders a link as a button, never an anchor", () => {
    const { container, getByRole } = render(Markdown, {
      props: { source: "see [docs](https://example.com/x)", onlink: () => {} },
    });
    // No href exists anywhere in this component, so there is nothing for a
    // scheme to be smuggled into.
    expect(container.querySelector("a")).toBeNull();
    expect(getByRole("button", { name: "docs" })).toBeInTheDocument();
  });

  it("hands the URL to the host rather than navigating", async () => {
    const onlink = vi.fn();
    const { getByRole } = render(Markdown, {
      props: { source: "[docs](https://example.com/x)", onlink },
    });
    await userEvent.click(getByRole("button"));
    expect(onlink).toHaveBeenCalledWith("https://example.com/x");
  });

  it("is inert when the host has not said what opening a link means", () => {
    const { getByRole } = render(Markdown, {
      props: { source: "[docs](https://example.com)" },
    });
    // Navigating a Tauri webview away from the app is a one-way trip.
    expect(getByRole("button")).toBeDisabled();
  });
});

describe("Markdown — hostile input", () => {
  const attacks = [
    ["a script tag", "<script>alert(1)</script>"],
    ["an img onerror", '<img src=x onerror="alert(1)">'],
    ["an iframe", '<iframe src="https://evil.example"></iframe>'],
    ["an svg onload", "<svg onload=alert(1)></svg>"],
    ["an anchor with a javascript href", "<a href=\"javascript:alert(1)\">x</a>"],
  ] as const;

  for (const [what, source] of attacks) {
    it("renders " + what + " as text, creating no element", () => {
      const { container } = render(Markdown, { props: { source } });
      expect(container.querySelector("script")).toBeNull();
      expect(container.querySelector("img")).toBeNull();
      expect(container.querySelector("iframe")).toBeNull();
      expect(container.querySelector("svg")).toBeNull();
      expect(container.querySelector("a")).toBeNull();
      // The reader sees exactly what the agent wrote.
      expect(container.textContent).toContain(source.slice(0, 12));
    });
  }

  it("does not make a link out of a javascript: URL", () => {
    const { container } = render(Markdown, {
      props: { source: "[click](javascript:alert(1))", onlink: () => {} },
    });
    expect(container.querySelector("button")).toBeNull();
    expect(container.textContent).toContain("javascript:alert(1)");
  });

  it("does not make a link out of a data: URL", () => {
    const { container } = render(Markdown, {
      props: { source: "[x](data:text/html;base64,PHNjcmlwdD4=)", onlink: () => {} },
    });
    expect(container.querySelector("button")).toBeNull();
  });

  it("emits no href attribute anywhere, whatever the input", () => {
    const source = [
      "[a](https://ok.example)",
      "[b](javascript:alert(1))",
      '<a href="javascript:alert(1)">c</a>',
      "<img src=x onerror=alert(1)>",
    ].join("\n\n");
    const { container } = render(Markdown, { props: { source, onlink: () => {} } });
    expect(container.querySelectorAll("[href]")).toHaveLength(0);
    expect(container.querySelectorAll("[src]")).toHaveLength(0);
  });

  it("emits no inline event handler attribute", () => {
    const { container } = render(Markdown, {
      props: { source: '<div onclick="alert(1)" onmouseover="alert(2)">x</div>' },
    });
    for (const el of container.querySelectorAll("*")) {
      for (const attr of el.getAttributeNames()) {
        expect(attr.startsWith("on")).toBe(false);
      }
    }
  });

  it("does not read markup inside a fenced block", () => {
    const { container } = render(Markdown, {
      props: { source: "```\n<script>alert(1)</script>\n```" },
    });
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("pre")?.textContent).toContain("<script>alert(1)</script>");
  });
});
