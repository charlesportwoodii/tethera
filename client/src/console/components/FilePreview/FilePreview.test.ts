import { describe, expect, it } from "vitest";
import { render } from "@testing-library/svelte";
import FilePreview from "./FilePreview.svelte";

describe("FilePreview", () => {
  it("waits rather than claiming there is nothing to show", () => {
    const { container, getByText } = render(FilePreview, {
      props: { name: "notes.md", mime: "text/markdown" },
    });
    // Conflating "still arriving" with "unreadable" tells the reader a file is
    // broken while its first chunk is in flight.
    expect(container.querySelector(".tc-fp__waiting")).toBeInTheDocument();
    expect(getByText(/Reading the first part/)).toBeInTheDocument();
  });

  it("renders markdown through the transcript's own parser", () => {
    const { container } = render(FilePreview, {
      props: { name: "notes.md", mime: "text/markdown", text: "a **b** c" },
    });
    expect(container.querySelector("strong")?.textContent).toBe("b");
  });

  it("renders source as preformatted text, wrapped", () => {
    const { container } = render(FilePreview, {
      props: { name: "main.rs", mime: null, text: "fn main() {\n    ok();\n}" },
    });
    const pre = container.querySelector("pre");
    expect(pre?.textContent).toContain("    ok();");
  });

  it("tones a diff by line", () => {
    const { container } = render(FilePreview, {
      props: {
        name: "fix.patch",
        mime: null,
        text: "@@ -1 +1 @@\n-old\n+new\n context",
      },
    });
    expect(container.querySelectorAll(".is-add")).toHaveLength(1);
    expect(container.querySelectorAll(".is-del")).toHaveLength(1);
    expect(container.querySelectorAll(".is-hunk")).toHaveLength(1);
  });

  it("shows an image only from a served image type", () => {
    const served = render(FilePreview, {
      props: { name: "shot.png", mime: "image/png", imageUrl: "blob:x" },
    });
    expect(served.container.querySelector("img")).toBeInTheDocument();
    served.unmount();

    const guessed = render(FilePreview, {
      props: { name: "shot.png", mime: null, imageUrl: "blob:x" },
    });
    // No served type means no decode, whatever the name says.
    expect(guessed.container.querySelector("img")).toBeNull();
  });

  it("gives an alt text so the image is not anonymous", () => {
    const { container } = render(FilePreview, {
      props: { name: "shot.png", mime: "image/png", imageUrl: "blob:x" },
    });
    expect(container.querySelector("img")).toHaveAttribute("alt", "shot.png");
  });

  it("says plainly when there is nothing it can render", () => {
    const { getByText, container } = render(FilePreview, {
      props: { name: "core.dump", mime: "application/octet-stream" },
    });
    expect(getByText(/No preview for this kind of file/)).toBeInTheDocument();
    expect(container.querySelector(".tc-fp__waiting")).toBeNull();
  });

  it("says when it is showing only the head of a file", () => {
    const { getByText } = render(FilePreview, {
      props: { name: "huge.log", mime: "text/plain", text: "line", truncated: true },
    });
    // Otherwise a long file read short is indistinguishable from a short file.
    expect(getByText(/first part only/)).toBeInTheDocument();
  });

  it("does not claim truncation while it is still waiting", () => {
    const { queryByText } = render(FilePreview, {
      props: { name: "huge.log", mime: "text/plain", truncated: true },
    });
    expect(queryByText(/first part only/)).toBeNull();
  });

  it("takes an explicit kind over what it would guess", () => {
    const { container } = render(FilePreview, {
      props: { name: "notes.md", mime: "text/markdown", text: "a **b** c", kind: "text" },
    });
    expect(container.querySelector("strong")).toBeNull();
    expect(container.querySelector("pre")?.textContent).toContain("**b**");
  });

  it("renders an SVG as source, never as an image", () => {
    const { container } = render(FilePreview, {
      props: {
        name: "logo.svg",
        mime: "image/svg+xml",
        text: "<svg onload=\"alert(1)\"></svg>",
        imageUrl: "blob:x",
      },
    });
    // An SVG is a document that can carry script.
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("svg")).toBeNull();
    expect(container.querySelector("pre")?.textContent).toContain("onload");
  });
});
