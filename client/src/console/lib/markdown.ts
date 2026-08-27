/**
 * Markdown to a closed set of nodes.
 *
 * **There is no HTML string anywhere in this file, and no consumer of it uses
 * `{@html}`.** That is the whole security design, and it is stronger than
 * sanitising: with no HTML parse step, injecting an element is not something a
 * sanitiser has to catch — it is something the pipeline cannot express. Agent
 * replies quote files, web pages and command output, so a README containing
 * `<img src=x onerror=...>` reaches this code through an ordinary session, and
 * `csp: null` in tauri.conf.json means script that runs here runs beside the iroh
 * endpoint and the identity key.
 *
 * Anything this parser does not recognise stays text. That is the correct way to
 * fail: an unrendered asterisk is a cosmetic bug, an executed tag is not.
 */

export type Inline =
  | { kind: "text"; text: string }
  /** Never recursed into. Code is literal by definition. */
  | { kind: "code"; text: string }
  | { kind: "strong"; children: Inline[] }
  | { kind: "em"; children: Inline[] }
  | { kind: "strike"; children: Inline[] }
  | { kind: "link"; children: Inline[]; href: string };

export type Block =
  | { kind: "paragraph"; inline: Inline[] }
  | { kind: "heading"; level: number; inline: Inline[] }
  | { kind: "code"; lang: string | null; text: string }
  | { kind: "list"; ordered: boolean; items: Inline[][] }
  | { kind: "quote"; inline: Inline[] }
  | { kind: "rule" };

/**
 * Schemes a link may carry.
 *
 * A URL that is not one of these is not a link at all — it renders as plain
 * text. No `href` attribute is emitted anywhere regardless, so `javascript:` has
 * nothing to be smuggled into; this check exists so the reader is never offered a
 * control that would go somewhere unexpected.
 */
const SAFE_SCHEME = /^(https?:\/\/|mailto:)/i;

const FENCE = /^\s*(?:```|~~~)\s*([A-Za-z0-9_+-]*)\s*$/;
const HEADING = /^(#{1,6})\s+(.*)$/;
const RULE = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/;
const BULLET = /^\s*[-*+]\s+(.*)$/;
const ORDERED = /^\s*\d+[.)]\s+(.*)$/;
const QUOTE = /^\s*>\s?(.*)$/;

/**
 * One pass, ordered so the tightest binding wins: code spans first, because a
 * backtick suppresses every other marker inside it, then strong before emphasis
 * so `**` is not read as two `*`.
 */
const INLINE =
  /`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|~~([^~]+)~~|\*([^*\n]+)\*|_([^_\n]+)_|\[([^\]\n]*)\]\(([^)\s]+)\)/g;

/**
 * How deep nesting may go.
 *
 * Each recursion is over a strictly shorter substring, so this is a guard
 * against pathological input rather than against non-termination. Four covers
 * anything an agent writes — bold containing a code span containing nothing.
 */
const MAX_DEPTH = 4;

/**
 * Parse one run of inline markup.
 *
 * Emphasis, strong, strikethrough and link text recurse, so a code span inside
 * bold survives — `**run `cargo test` first**` is one strong node containing
 * text, code and text. A code span never recurses: code is literal, and reading
 * markers inside it would turn half a shell command bold.
 */
export function parseInline(src: string, depth = 0): Inline[] {
  const out: Inline[] = [];
  let last = 0;

  // The regex is stateful, and a nested call shares it. Capturing every match
  // before recursing keeps the two from interleaving on lastIndex.
  const matches: RegExpExecArray[] = [];
  INLINE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = INLINE.exec(src)) !== null) {
    matches.push(match);
  }

  const inner = (body: string): Inline[] =>
    depth < MAX_DEPTH ? parseInline(body, depth + 1) : [{ kind: "text", text: body }];

  for (const found of matches) {
    if (found.index > last) {
      out.push({ kind: "text", text: src.slice(last, found.index) });
    }

    const whole = found[0];
    const code = found[1];
    const strongStars = found[2];
    const strongUnderscores = found[3];
    const strike = found[4];
    const emStars = found[5];
    const emUnderscores = found[6];
    const linkText = found[7];
    const href = found[8];

    if (code !== undefined) {
      out.push({ kind: "code", text: code });
    } else if (strongStars !== undefined || strongUnderscores !== undefined) {
      out.push({ kind: "strong", children: inner(strongStars ?? strongUnderscores) });
    } else if (strike !== undefined) {
      out.push({ kind: "strike", children: inner(strike) });
    } else if (emStars !== undefined || emUnderscores !== undefined) {
      out.push({ kind: "em", children: inner(emStars ?? emUnderscores) });
    } else if (href !== undefined) {
      if (SAFE_SCHEME.test(href)) {
        out.push({
          kind: "link",
          children: linkText === "" ? [{ kind: "text", text: href }] : inner(linkText),
          href,
        });
      } else {
        // Not a link. Kept verbatim so the reader sees exactly what the agent
        // wrote rather than a control that goes somewhere unexpected.
        out.push({ kind: "text", text: whole });
      }
    }

    last = found.index + whole.length;
  }

  if (last < src.length) {
    out.push({ kind: "text", text: src.slice(last) });
  }

  return merge(out.length > 0 ? out : [{ kind: "text", text: src }]);
}

/** The text of a node and everything under it, for a one-line preview. */
export function inlineText(nodes: Inline[]): string {
  return nodes
    .map((node) => (node.kind === "text" || node.kind === "code" ? node.text : inlineText(node.children)))
    .join("");
}

/** The plain text of a whole message, for a preview row or a notification. */
export function plainText(blocks: Block[]): string {
  return blocks
    .map((block) => {
      if (block.kind === "code") return block.text;
      if (block.kind === "rule") return "";
      if (block.kind === "list") return block.items.map(inlineText).join(" ");
      return inlineText(block.inline);
    })
    .filter((line) => line !== "")
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Fold neighbouring text nodes together.
 *
 * Rejecting a link leaves the text of the whole attempt plus whatever followed
 * it, and an unmatched marker does the same. Merging keeps the output one node
 * per run of prose rather than a node per near-miss.
 */
function merge(nodes: Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const node of nodes) {
    const last = out[out.length - 1];
    if (node.kind === "text" && last !== undefined && last.kind === "text") {
      out[out.length - 1] = { kind: "text", text: last.text + node.text };
      continue;
    }
    out.push(node);
  }
  return out;
}

export function parseMarkdown(src: string): Block[] {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    if (line.trim() === "") {
      i += 1;
      continue;
    }

    const fence = FENCE.exec(line);
    if (fence) {
      const lang = fence[1] === "" ? null : fence[1];
      const body: string[] = [];
      i += 1;
      // An unterminated fence runs to the end of the message. Reading it as code
      // is the safe choice: the alternative renders a half-written block as prose
      // and loses its whitespace.
      while (i < lines.length && !FENCE.test(lines[i])) {
        body.push(lines[i]);
        i += 1;
      }
      if (i < lines.length) i += 1;
      blocks.push({ kind: "code", lang, text: body.join("\n") });
      continue;
    }

    if (RULE.test(line)) {
      blocks.push({ kind: "rule" });
      i += 1;
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      blocks.push({
        kind: "heading",
        level: heading[1].length,
        inline: parseInline(heading[2].trim()),
      });
      i += 1;
      continue;
    }

    if (QUOTE.test(line)) {
      const body: string[] = [];
      while (i < lines.length) {
        const quoted = QUOTE.exec(lines[i]);
        if (!quoted) break;
        body.push(quoted[1]);
        i += 1;
      }
      blocks.push({ kind: "quote", inline: parseInline(body.join(" ").trim()) });
      continue;
    }

    const bullet = BULLET.exec(line);
    const ordered = ORDERED.exec(line);
    if (bullet || ordered) {
      const isOrdered = ordered !== null && bullet === null;
      const items: Inline[][] = [];
      while (i < lines.length) {
        const asBullet = BULLET.exec(lines[i]);
        const asOrdered = ORDERED.exec(lines[i]);
        const item = isOrdered
          ? asOrdered
            ? asOrdered[1]
            : null
          : asBullet
            ? asBullet[1]
            : null;
        if (item === null) break;
        items.push(parseInline(item.trim()));
        i += 1;
      }
      blocks.push({ kind: "list", ordered: isOrdered, items });
      continue;
    }

    // A paragraph runs until a blank line or the start of another block.
    const body: string[] = [];
    while (i < lines.length) {
      const next = lines[i];
      if (
        next.trim() === "" ||
        FENCE.test(next) ||
        HEADING.test(next) ||
        RULE.test(next) ||
        BULLET.test(next) ||
        ORDERED.test(next) ||
        QUOTE.test(next)
      ) {
        break;
      }
      body.push(next.trim());
      i += 1;
    }
    blocks.push({ kind: "paragraph", inline: parseInline(body.join(" ")) });
  }

  return blocks;
}
