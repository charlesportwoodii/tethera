import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown, type Block, type Inline } from "./markdown";

const kinds = (nodes: Inline[]) => nodes.map((n) => n.kind);
const blockKinds = (blocks: Block[]) => blocks.map((b) => b.kind);

describe("parseInline", () => {
  it("keeps plain text as one node", () => {
    expect(parseInline("just words")).toEqual([{ kind: "text", text: "just words" }]);
  });

  it("reads bold, emphasis, strikethrough and code", () => {
    const nodes = parseInline("a **b** c *d* e ~~f~~ g `h`");
    expect(kinds(nodes)).toEqual([
      "text",
      "strong",
      "text",
      "em",
      "text",
      "strike",
      "text",
      "code",
    ]);
  });

  it("lets a code span suppress the markers inside it", () => {
    // Otherwise `**` inside a shell snippet turns half a command bold.
    const nodes = parseInline("run `echo **not bold**` now");
    expect(nodes[1]).toEqual({ kind: "code", text: "echo **not bold**" });
  });

  it("does not read ** as two separate emphases", () => {
    expect(parseInline("**both**")).toEqual([
      { kind: "strong", children: [{ kind: "text", text: "both" }] },
    ]);
  });

  it("reads an http link", () => {
    expect(parseInline("see [docs](https://example.com/x)")).toEqual([
      { kind: "text", text: "see " },
      {
        kind: "link",
        children: [{ kind: "text", text: "docs" }],
        href: "https://example.com/x",
      },
    ]);
  });

  it("uses the URL as the label when a link has none", () => {
    const nodes = parseInline("[](https://example.com)");
    expect(nodes[0]).toEqual({
      kind: "link",
      children: [{ kind: "text", text: "https://example.com" }],
      href: "https://example.com",
    });
  });

  it("recurses into bold, so a code span inside it survives", () => {
    const nodes = parseInline("**run `cargo test` first**");
    expect(nodes).toHaveLength(1);
    const strong = nodes[0];
    expect(strong.kind).toBe("strong");
    if (strong.kind === "strong") {
      expect(kinds(strong.children)).toEqual(["text", "code", "text"]);
    }
  });

  it("recurses into link text", () => {
    const nodes = parseInline("[the **docs**](https://example.com)");
    const link = nodes[0];
    if (link.kind === "link") {
      expect(kinds(link.children)).toEqual(["text", "strong"]);
    }
  });

  it("never recurses into a code span", () => {
    const nodes = parseInline("`a **b** c`");
    // Code is literal. Reading markers inside it turns half a command bold.
    expect(nodes[0]).toEqual({ kind: "code", text: "a **b** c" });
  });

  it("stops nesting at a bounded depth rather than recursing without limit", () => {
    const deep = "**" + "~~".repeat(1) + "_a_" + "~~".repeat(1) + "**";
    // Terminates and produces only allowed kinds, whatever the nesting.
    const seen = new Set<string>();
    const walk = (nodes: Inline[]) => {
      for (const n of nodes) {
        seen.add(n.kind);
        if (n.kind !== "text" && n.kind !== "code") walk(n.children);
      }
    };
    walk(parseInline(deep));
    for (const kind of seen) {
      expect(["text", "code", "strong", "em", "strike", "link"]).toContain(kind);
    }
  });

  it("leaves an unmatched asterisk alone rather than eating it", () => {
    expect(parseInline("2 * 3 = 6")).toEqual([{ kind: "text", text: "2 * 3 = 6" }]);
  });
});

describe("parseInline — hostile input", () => {
  it("does not turn a javascript: URL into a link", () => {
    const nodes = parseInline("[click](javascript:alert(1))");
    // Rendered exactly as the agent wrote it. There is no link node, so no
    // component can be handed the scheme in the first place.
    expect(kinds(nodes)).toEqual(["text"]);
    expect(nodes[0]).toEqual({ kind: "text", text: "[click](javascript:alert(1))" });
  });

  it("does not turn a data: URL into a link", () => {
    expect(kinds(parseInline("[x](data:text/html;base64,PHNjcmlwdD4=)"))).toEqual(["text"]);
  });

  it("does not turn a vbscript: URL into a link", () => {
    expect(kinds(parseInline("[x](vbscript:msgbox)"))).toEqual(["text"]);
  });

  it("is not fooled by case or leading whitespace in a scheme", () => {
    expect(kinds(parseInline("[x](JaVaScRiPt:alert(1))"))).toEqual(["text"]);
  });

  it("keeps a script tag as text", () => {
    const nodes = parseInline("<script>alert(1)</script>");
    expect(nodes).toEqual([{ kind: "text", text: "<script>alert(1)</script>" }]);
  });

  it("keeps an img onerror as text", () => {
    const nodes = parseInline('<img src=x onerror="alert(1)">');
    expect(nodes).toEqual([{ kind: "text", text: '<img src=x onerror="alert(1)">' }]);
  });

  it("keeps an iframe as text", () => {
    const nodes = parseInline('<iframe src="https://evil.example"></iframe>');
    expect(kinds(nodes)).toEqual(["text"]);
  });

  it("never produces a node kind outside the closed set", () => {
    const hostile = [
      "<script>alert(1)</script>",
      '<img src=x onerror="alert(1)">',
      "[a](javascript:alert(1))",
      '<iframe src="x"></iframe>',
      "<svg onload=alert(1)>",
      "<a href='javascript:alert(1)'>x</a>",
    ].join("\n\n");
    const allowed = new Set(["text", "code", "strong", "em", "strike", "link"]);
    for (const block of parseMarkdown(hostile)) {
      if ("inline" in block) {
        for (const node of block.inline) {
          expect(allowed.has(node.kind)).toBe(true);
        }
      }
    }
  });
});

describe("parseMarkdown", () => {
  it("reads a heading and its level", () => {
    const [block] = parseMarkdown("### Routing");
    expect(block).toEqual({
      kind: "heading",
      level: 3,
      inline: [{ kind: "text", text: "Routing" }],
    });
  });

  it("joins the lines of a paragraph", () => {
    const [block] = parseMarkdown("one line\nand another");
    expect(block).toEqual({
      kind: "paragraph",
      inline: [{ kind: "text", text: "one line and another" }],
    });
  });

  it("separates paragraphs on a blank line", () => {
    expect(blockKinds(parseMarkdown("first\n\nsecond"))).toEqual(["paragraph", "paragraph"]);
  });

  it("reads a fenced block and keeps its whitespace", () => {
    const [block] = parseMarkdown("```rust\nfn main() {\n    ok();\n}\n```");
    expect(block).toEqual({ kind: "code", lang: "rust", text: "fn main() {\n    ok();\n}" });
  });

  it("takes an unterminated fence to the end of the message", () => {
    // Reading it as prose would lose the whitespace of a half-streamed block.
    const [block] = parseMarkdown("```\nstill writing");
    expect(block).toEqual({ kind: "code", lang: null, text: "still writing" });
  });

  it("does not read markup inside a fenced block", () => {
    const [block] = parseMarkdown("```\n# not a heading\n**not bold**\n```");
    expect(block.kind).toBe("code");
    if (block.kind === "code") {
      expect(block.text).toBe("# not a heading\n**not bold**");
    }
  });

  it("reads a bullet list", () => {
    const [block] = parseMarkdown("- one\n- two");
    expect(block.kind).toBe("list");
    if (block.kind === "list") {
      expect(block.ordered).toBe(false);
      expect(block.items).toHaveLength(2);
    }
  });

  it("reads an ordered list", () => {
    const [block] = parseMarkdown("1. one\n2. two");
    expect(block.kind).toBe("list");
    if (block.kind === "list") {
      expect(block.ordered).toBe(true);
      expect(block.items).toHaveLength(2);
    }
  });

  it("reads markup inside a list item", () => {
    const [block] = parseMarkdown("- run `cargo test`");
    if (block.kind === "list") {
      expect(kinds(block.items[0])).toEqual(["text", "code"]);
    }
  });

  it("reads a blockquote across lines", () => {
    const [block] = parseMarkdown("> first\n> second");
    expect(block).toEqual({
      kind: "quote",
      inline: [{ kind: "text", text: "first second" }],
    });
  });

  it("reads a horizontal rule", () => {
    expect(blockKinds(parseMarkdown("above\n\n---\n\nbelow"))).toEqual([
      "paragraph",
      "rule",
      "paragraph",
    ]);
  });

  it("does not swallow the block that follows a paragraph", () => {
    expect(blockKinds(parseMarkdown("intro\n- one\n- two"))).toEqual(["paragraph", "list"]);
  });

  it("returns nothing for an empty message rather than an empty paragraph", () => {
    expect(parseMarkdown("")).toEqual([]);
    expect(parseMarkdown("\n\n  \n")).toEqual([]);
  });

  it("handles CRLF", () => {
    expect(blockKinds(parseMarkdown("one\r\n\r\ntwo"))).toEqual(["paragraph", "paragraph"]);
  });
});
