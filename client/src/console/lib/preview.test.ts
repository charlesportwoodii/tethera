import { describe, expect, it } from "vitest";
import { isPreviewable, previewKind, PREVIEW_BYTES } from "./preview";

describe("previewKind", () => {
  it("reads markdown from either the type or the name", () => {
    expect(previewKind("notes.md", null)).toBe("markdown");
    expect(previewKind("notes", "text/markdown")).toBe("markdown");
  });

  it("reads a diff as a diff", () => {
    expect(previewKind("fix.patch", null)).toBe("diff");
    expect(previewKind("x", "text/x-diff")).toBe("diff");
  });

  it("reads source as code even when the type is unhelpful", () => {
    // A server calling a .rs file octet-stream is common, and the reader would
    // rather see the source than be told there is no preview.
    expect(previewKind("main.rs", "application/octet-stream")).toBe("code");
    expect(previewKind("deeplink.ts", null)).toBe("code");
  });

  it("reads structured text as code", () => {
    expect(previewKind("x", "application/json")).toBe("code");
    expect(previewKind("x", "application/yaml")).toBe("code");
  });

  it("reads a served image as an image", () => {
    expect(previewKind("shot.png", "image/png")).toBe("image");
  });

  it("does not read an image from the filename alone", () => {
    // Feeding a decoder bytes on the strength of a name is how it gets something
    // it did not expect.
    expect(previewKind("shot.png", null)).toBe("none");
  });

  it("treats SVG as code, not as a picture", () => {
    // An SVG is a document that can carry script.
    expect(previewKind("logo.svg", "image/svg+xml")).toBe("code");
  });

  it("reads plain text", () => {
    expect(previewKind("out.log", null)).toBe("text");
    expect(previewKind("x", "text/plain")).toBe("text");
  });

  it("gives up honestly on a binary", () => {
    expect(previewKind("core.dump", "application/octet-stream")).toBe("none");
    expect(previewKind("Makefile", null)).toBe("none");
  });
});

describe("isPreviewable", () => {
  it("refuses what it cannot render", () => {
    expect(isPreviewable("core.dump", "application/octet-stream", 10)).toBe(false);
  });

  it("accepts text of any length, because it reads only the head", () => {
    expect(isPreviewable("huge.log", "text/plain", 5_000_000_000)).toBe(true);
  });

  it("refuses an image too large to decode on a phone", () => {
    expect(isPreviewable("shot.png", "image/png", 20 * 1024 * 1024)).toBe(false);
    expect(isPreviewable("shot.png", "image/png", 1024)).toBe(true);
  });

  it("refuses an image of unknown length rather than starting a transfer", () => {
    // An image has to arrive whole to decode, so an unknown length is a blank
    // cheque.
    expect(isPreviewable("shot.png", "image/png", null)).toBe(false);
  });

  it("reads a bounded head", () => {
    expect(PREVIEW_BYTES).toBe(65536);
  });
});
