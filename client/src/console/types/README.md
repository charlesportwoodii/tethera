# Gaps between the design and the bindings

Written while building the component library against `src/js/bindings`. Each
item was a field the components needed and the wire contract did not carry.

**All five are now closed.** The protocol landed in
`docs/superpowers/specs/2026-08-25-tethera-protocol-design.md`; regenerate with
`mise run bindings` if `src/js/bindings` looks stale.

## 1. `TranscriptEntry` has no role and no timestamp — CLOSED

`TranscriptEntry` is now `Turn`, which is what this file, `Turn.svelte` and the
protocol spec all already called it:

```ts
export type Turn = { cursor: Cursor, id: TurnId, at: Timestamp, role: Role, parts: Array<Part> };
```

`role` is `"operator" | "agent"` and `at` is epoch millis. `Turn` also carries a
`cursor`, which is how a live tail resumes — the protocol deliberately carries no
sequence number, because ordering is page order and dedupe is by `id`.

`Timestamp` is `number`, not `bigint`. That was a real bug: these values arrive
as JSON, and `JSON.parse` never produces a bigint, so the old binding described a
runtime value that could not occur. `Device.paired_at` and `Part.file.size` are
fixed the same way.

## 2. There is no link state — CLOSED, but not the way this asked

```ts
export type Link = { kind: LinkKind, rtt_ms: number | null };
export type LinkKind = "direct" | "relayed" | "unknown" | "offline";
```

The type moved into the common crate as this file suggested, and is in the
bindings. **The server does not send it.** This file said "the gateway is the
only party that knows this", and that is the one point the protocol design
disagrees with: the server can describe the connection *it* sees, which is a
different connection from the one the phone holds when a relay sits between
them. The client measures it from its own Iroh endpoint. The type exists in
`common` so both halves share one vocabulary, not because it crosses the wire.

Note the added `unknown`. A path that has not settled is not `direct`,
`relayed` or `offline`, and the predecessor's UI drew it as "direct" and was
wrong about half the time it appeared. `rtt_ms` is nullable for the same reason:
absent is not zero.

## 3. `Part::question` carries no option descriptions — CLOSED

```ts
export type QuestionOption = { label: string, description: string | null };
```

The shape-tolerant branch this used to need is gone with `AskBlock` itself.
`QuestionFlow` reads `description` straight off the binding.

## 4. `Part::question` carries no fingerprint — CLOSED

`Question` now carries `fingerprint`, and `Request::answer_question` sends it
back. The server refuses `stale` if the pane has moved on rather than answering
a different question blind.

`Part::question` also carries `answered: AnswerRecord | null`, so a historical
question renders with what was chosen and when.

## 5. `Agent` is a closed enum of two — CLOSED

`Agent` no longer crosses the wire. The catalog does:

```ts
export type AgentProfile = {
  id: ProfileId, label: string, description: string | null,
  version: string | null, supports_resume: boolean,
  provides_transcript: boolean, modes: Array<AgentMode>,
};
export type AgentMode = {
  id: ModeId, label: string, description: string | null,
  draws_permission_menu: boolean,
};
```

`HarnessPicker` should type against `AgentProfile` and hand a `ProfileId` back.
Adding a third agent is now a server-side trait implementation and a catalog
row, not a client release. `Agent.ts` and `AgentSpawn.ts` are gone; the enum
survives only as the server CLI's own argument type, which is the right shape
for something a person types at the machine.

Two fields worth knowing about:

- `version` — because "Claude Code" alone does not say whether this build's
  records can be read. It is `null` when the machine has not probed it, rather
  than a guess.
- `provides_transcript` — whether starting this profile will have a readable
  transcript at all. `false` means offer the terminal, not an empty
  conversation. Both agents report `false` today: no transcript reader exists
  yet, and the catalog says so rather than promising a screen that would render
  blank.

## Not gaps, but worth knowing

`Conversation` gained three fields the tree needs, so a home screen across
several machines costs one request rather than one per conversation:

- `preview` — one line of the most recent meaningful text, or the pending
  question's prompt when blocked
- `workspace` — so the tree can group without walking up through a pane that may
  not exist
- `has_transcript` — the per-conversation counterpart of the field above

`Workspace` gained `tab_count` and `Tab` gained `index`, `conversation` and
`foreground_command`, so a tab row draws its own glyph and subtitle without a
`ListPanes` per tab.

## 6. There is no live agent stats record

`ThinkingRow` needs elapsed, tokens in and out, tool count, context used and its
window, and — where there is room — model and turn cost. Claude Code knows every
one of them. None are on `TranscriptEntry`.

**Needed:** a per-pane record the client can poll or subscribe to, separate from
the transcript. Carrying these as transcript parts means the numbers only move
when a message lands, which turns a live row into a spinner with decoration.

**This is the one to decide first.** It changes what the gateway watches, not how
the client draws.

## 7. There is no in-flight activity string

"Reading src/lib/deeplink.ts" is what makes the thinking row feel alive. It is the
tool call currently running, which is not a transcript entry until it finishes.

**Needed:** the in-flight tool name and its principal argument on the same live
record as the stats above.

## 8. Questions are one at a time, and untagged

```ts
{ "question": { prompt: string, options: Array<string>, fallback_text: string } }
```

`QuestionFlow` answers a **set**: several questions, each with a header, a
`multiSelect` flag, and options carrying a label plus a description — and one
fingerprint for the set, so a single reply answers all of them.

**Needed:** `Part::questions` carrying `Array<{ header, question, options: Array<{ label, detail }>, multi_select }>`
plus `fingerprint`. Until then the component takes the whole set as a prop and the
caller adapts whatever the wire sends.

## 9. Files have no type or size before they are fetched

`FileViewer` decides between rendered, source, diff, image and no-preview from a
MIME type, and refuses to pull a 5 GB dump to a phone at all.

**Needed:** `mime` and `size` on the asset listing, and a byte-range fetch so a
preview reads the first few kilobytes rather than the file.
