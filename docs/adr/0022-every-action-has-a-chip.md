# Every action has a Chip

`AGENTS.md` already requires every action to be reachable from the keyboard without a modifier, and
the cheatsheet to be the contract for what is bindable. That covers the hand on the keyboard and
leaves the hand on the mouse to luck: the file tree's focused row carries icons for its actions and
the Reading has a Transport, but a new feature could still ship keyed-only, and nothing said it
should not. While the debugger was being designed, the user asked for continue, pause and the steps
to be clickable too — and for that to be the rule across Varde, not a feature of the debugger.

**Every action is both keyed and clickable. The click target is a Chip: on a Transport along a
pane's top border for what drives the pane, or on the focused row for what acts on that row.** It is
never a right-click menu. Right-click stays a no-op, as `features/mouse.feature` specifies.

## What a Chip is

A glyph and the keys that do the same thing — every one of them, so the debugger's step over
reads ` ⤼ F8 ␣n `, and the Chip teaches its own key and its modifier-free alias every time it is
looked at.
No word: the keys carry more than a label would and cost less width.

**Dimmed and lit come from state, never from a timer.** A Chip is dimmed while its action cannot
run, and lit while it is the last action taken, whether that was by click or by key. A lit Chip
that faded after a moment needed a Tick, and
`docs/adr/0009-a-spinner-is-bounded-by-its-job.md` allows a Tick only while work is in flight. "Lit
until the next action" answers "what did I just do?" better anyway, including for somebody who
looked away.

**Glyphs are ones every font draws one cell wide.** Geometric shapes, not emoji and not Nerd Font
icons. Emoji are colourful and bigger, but terminals disagree about whether they are one cell or
two, and where they disagree every click to the right of one lands a column off — the failure the
one-layout rule in `AGENTS.md` exists to prevent. Nerd Font icons are the best-looking option, but
Varde cannot ask the terminal which font it has, and without one installed each icon is an empty
box.

**Colour is the theme's named colours**, never fixed RGB, for the reason the faint layer is drawn in
the terminal's own foreground: a Chip has to belong to whatever theme it is drawn in.

**Short of room, every Chip sheds its keys at once**, down to glyph alone. None is cut or wrapped,
and none keeps its keys while a neighbour loses them, so the row has one shape at a given width and
the eye learns it once. The keys stay in the cheatsheet.

## Why not a right-click menu

It is how most GUIs make an action clickable, and it was considered for the Variables' row actions.
It hides every action until somebody thinks to ask for it, it would add a second way of offering
actions alongside the row icons the tree already has, and it would take away a promise — right-click
does nothing — that the mouse suite asserts. Icons on the focused row keep the actions in sight
where the eye already is, and they are the pattern Varde already had.

## Consequences

**A new action without a Chip is incomplete**, the same as a key missing from the cheatsheet. The
debugger (issue #45) is the first feature built this way, and the Reading's Transport is restyled to
match. Existing keyed-only actions elsewhere in Varde are catching up, not grandfathered: each gets
its Chip when the feature it belongs to is next worked on.

**Width is the cost, and it is paid knowingly.** A Transport of eight Chips with their keys is wider
than a row of bare glyphs, so the key-shedding rule will apply often on narrow screens. Chips with
glyph and keys were chosen over bare glyphs for exactly the discoverability that costs the width.

**Transport and Chip are words in `CONTEXT.md`.** A Transport is no longer the Reading's alone: it
is any pane's row of Chips driving something in flight.