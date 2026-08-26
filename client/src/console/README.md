# Console

The design system for the Tethera phone client. Every export is a building block:
nothing here fetches, routes, holds a session, or knows what a screen is.
Composing these into screens is a separate job.

## Layout

    src/console/
      tokens/        _tokens.scss   colour, type, radii, rail geometry
                     _mixins.scss   the three fragments worth sharing
                     _rail.scss     trunk / elbow / glyph geometry, in one place
                     tokens.ts      token names for script that needs them
      types/         state.ts       GlyphState, LinkState, TurnRole, DrawerHeight
                     README.md      **what the wire does not carry yet — read this**
      components/    <Name>/<Name>.svelte
                            <Name>.scss
                            <Name>.types.ts
                            <Name>.test.ts

Each component is three files by design: markup and behaviour in the `.svelte`,
every rule in the `.scss`, and the props contract in the `.types.ts` so a caller
can import the interface without importing the component.

## Using them

    import { Tree, TreeNode, StatusGlyph, type GlyphState } from "$console";

The tokens are CSS custom properties on `:root`, applied once by importing
`src/app.scss`. A component never imports the token file — it reads
`var(--tc-*)` at runtime, so a host can restyle the system without recompiling it.

## The rules these components hold to

**Controls are rectangles, structure is curved.** Buttons, chips, code slots and
the key bar are 6–8px rectangles. Curves are reserved for things that carry
meaning: the elbow that joins a twig to its trunk, the drawer's lip, the round
state glyphs. The one pill left is `Toggle`, where the shape is the convention.

**No rule ever runs between two items.** Separation is space and the rail.
The only horizontal edges in the system are the drawer lip and the screen frame.

**State is a shape before it is a colour.** Sweeping arc for working, hollow ring
for idle, filled disc for done, and a wedge — the one angular mark in the whole
system — for blocked. Round means the machine is dealing with it; angular means
you are the blocker.

**One filled element per screen.** `Button` with `variant="primary"` is it.
Two primaries on one screen is a design bug, not a component bug.

**Absent, not disabled.** A capability the host lacks renders as nothing.
`Composer` drops the attach control when `onattach` is null; `TabStrip` drops the
add control when `onadd` is null. A dead button promises something the machine
cannot do.

**Nothing owns its own value.** `Toggle`, `Composer`, `Chip` and `TabStrip` all
report intent and re-render from props. Local state here would fight whatever the
gateway says on the next frame.

## Testing

    yarn test              # once
    yarn test:watch
    yarn test:coverage

Tests are behavioural: what a caller can observe through the DOM and the
callbacks, not the class names. Where a test asserts a class, it is because the
class is the only carrier of a state distinction that colour alone would fail —
`is-blocked` being the example.

## Known gaps

`types/README.md` lists five fields the components need and the bindings do not
carry: turn role, turn timestamp, link state, question option details, and the
question fingerprint. Every one of them is a prop today, so nothing is blocked —
but each is a common-crate decision rather than a client one.
