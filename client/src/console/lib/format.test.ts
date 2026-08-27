import { describe, expect, it } from "vitest";
import { fileExtension, formatBytes, formatDuration, formatTokens } from "./format";

describe("formatBytes", () => {
  it("uses binary units", () => {
    expect(formatBytes(8396)).toBe("8.2 KB");
    expect(formatBytes(5368709120n)).toBe("5.0 GB");
  });

  it("leaves small sizes in bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("drops the decimal once the number is large enough not to need it", () => {
    expect(formatBytes(50 * 1024)).toBe("50 KB");
  });

  it("says so for a size the machine never measured", () => {
    expect(formatBytes(null)).toBe("unknown size");
    expect(formatBytes(undefined)).toBe("unknown size");
  });

  it("says so rather than printing NaN", () => {
    expect(formatBytes(Number.NaN)).toBe("unknown size");
    expect(formatBytes(-1)).toBe("unknown size");
  });
});

describe("formatTokens", () => {
  it("keeps one decimal in the thousands", () => {
    expect(formatTokens(18412)).toBe("18.4k");
  });

  it("drops a trailing zero", () => {
    expect(formatTokens(2000)).toBe("2k");
  });

  it("stops using decimals past a hundred thousand", () => {
    expect(formatTokens(184000)).toBe("184k");
  });

  it("moves to millions", () => {
    expect(formatTokens(1_450_000)).toBe("1.5M");
  });

  it("leaves counts under a thousand exact", () => {
    expect(formatTokens(412)).toBe("412");
  });
});

describe("formatDuration", () => {
  it("counts in seconds under a minute", () => {
    expect(formatDuration(41)).toBe("41s");
  });

  it("adds minutes", () => {
    expect(formatDuration(134)).toBe("2m 14s");
  });

  it("drops seconds once it is measured in hours", () => {
    expect(formatDuration(7860)).toBe("2h 11m");
  });
});

describe("fileExtension", () => {
  it("takes the extension", () => {
    expect(fileExtension("pairing-routes.md")).toBe("MD");
  });

  it("does not mistake a name for an extension", () => {
    expect(fileExtension("Makefile")).toBe("FILE");
  });

  it("treats a dotfile as having no extension", () => {
    expect(fileExtension(".gitignore")).toBe("FILE");
  });

  it("ignores a trailing dot", () => {
    expect(fileExtension("weird.")).toBe("FILE");
  });

  it("truncates a long extension rather than blowing out the badge", () => {
    expect(fileExtension("archive.tarball")).toBe("TARB");
  });
});
