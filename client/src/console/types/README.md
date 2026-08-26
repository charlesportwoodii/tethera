# Gaps between the design and the bindings

Written while building the component library against `src/js/bindings`. Each
item is a field the components need and the wire contract does not carry yet.
None of them are blocking — every component takes the value as a prop — but each
one is a decision someone has to make in the common crate rather than here.

## 1. `TranscriptEntry` has no role and no timestamp

```ts
export type TranscriptEntry = { id: string, parts: Array<Part> };
```

The transcript is a timeline: every turn shows a time in the gutter, and your own
turns are marked differently from the agent's. Neither fact is on the wire.

**Needed:** `role: "you" | "agent"` and `at: number` (epoch millis, matching the
`bigint` timestamps on `Device`).

Until then `Turn` takes `role` and `at` as props and the caller supplies them.

## 2. There is no link state

Every connected screen shows whether the path is direct or relayed, and the
round-trip time. The gateway is the only party that knows this.

**Needed:** something like `Link { kind: "direct" | "relayed", rtt_ms: number | null }`
on whatever the client polls for server health.

## 3. `Part::question` carries no option descriptions

```ts
{ "question": { prompt: string, options: Array<string>, fallback_text: string } }
```

The design shows a short description under each option, because that is what the
agent's own question modal renders. As typed, an option is one string.

**Needed:** either `options: Array<{ label: string, detail: string | null }>`, or a
documented convention for splitting one string.

`AskBlock` accepts both shapes so the component does not block the decision.

## 4. `Part::question` carries no fingerprint

The gateway refuses an answer if the pane has moved on to a different question.
That guard needs the fingerprint to reach the client and come back with the answer.

**Needed:** `fingerprint: string` on the question part, and on the answer call.

## 5. `Agent` is a closed enum of two

```ts
export type Agent = "claude" | "codex";
```

That is correct for today and it is what `HarnessPicker` types against. Worth
knowing: adding a third agent is a common-crate change and a client rebuild, not
data. If agents should be discoverable at runtime, the catalogue needs to carry
name and version as strings and `Agent` becomes an id rather than an enum.
