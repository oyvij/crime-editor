# A GUI front end beside the TUI — notes to grill

Not a decision. A handoff from a conversation on 2026-09-19, written so a later grilling session
starts with everything that conversation found. Nothing here is settled; the ADRs come out of the
grilling, not out of this file. Facts were read off `main` at 96f0442 — re-check them before
leaning on one.

## The owner's constraints

These were stated, not proposed. The grilling should test them, not reopen them silently.

- **The TUI stays the product.** Varde remains a terminal TUI, unchanged. The GUI is an
  *alternative* app for better visibility, not a replacement and not a merge.
- **No separate windows.** Everything stays inside Varde's one window. The Markdown preview stays
  in the editor pane where it is today; a companion window next to the terminal was rejected.
- **The terminal's theme must carry over.** An app does not run inside the terminal, so it does
  not inherit its palette — this was raised as the objection, and the GUI has to answer it.
- **The goal is comprehension:** more readable fonts, real graphics, and animation where it helps
  someone understand what changed.

## What the code already gives a second front end

- **The core is pure.** `varde::update(&State, Event) -> (State, Vec<Effect>)` in `src/lib.rs`
  makes every decision; effects are data. No terminal, pty, git or filesystem in `src/` outside
  the edge.
- **The edge is small and the terminal stack lives only there.** `main.rs`, `ui.rs`, `pty.rs` and
  `rpc.rs` are about 10.5k of about 55k lines.
- **`ui.rs` depends on ratatui only, never on crossterm.** It draws into a `Frame` with ratatui
  widgets and reads view models from the library (`risk::list`, `history::list`, `buffer_list`,
  `preview::Row`, …). crossterm appears in about 9 places, all in `main.rs`: raw mode, alternate
  screen, event reading, `SetCursorStyle`, the OSC 52 clipboard. The `Screen` type alias in
  `main.rs` is the one line that names the backend.

## Where the core assumes a character grid

This is what "pure" does not mean: the core is free of I/O, not free of cells.

- **Geometry is cells.** `layout::Area` is `u16` columns and rows, reproducing ratatui's solver;
  `GUTTER`, border label strips and minimap rows are cell counts.
- **The mouse is cells.** `mouse::on_mouse` hit-tests cell coordinates; drag spans are cell
  columns; `mouse::report` encodes xterm bytes.
- **Keys are a terminal type.** `keys.rs` takes `terminput::KeyEvent`, and its bindings are shaped
  by what a terminal can report (no bare modifier press, the Ctrl+Space fallback).
- **Scroll and wrap are rows and columns.** `Event::Resized { width, height }` is cells; scroll
  offsets clamp in rows; LSP text wraps by column with `textwrap` and `unicode-width`.
- **`preview::rows(text, columns)` does two jobs at once:** it understands the document (headings,
  lists, code, tables, mermaid, the source line each came from) *and* lays it out to `columns`.
  Mermaid comes out as text art through `mermaid-text`.
- **Hosted panes are terminals by nature.** The shell and AI panes need `vt100` and
  `queries::reply` in any front end, and draw as a cell grid in any front end.
- **Effect execution lives in the binary.** `main.rs` carries out the effects and is not a
  library, so a second front end cannot reuse it as it stands.

## Colour today

Counted in `ui.rs`: about 110 uses of *named* ANSI colours (`Cyan`, `DarkGray`, `Yellow`, …) —
the terminal's theme decides what those look like, which is why Varde matches the terminal today —
and about 70 fixed ones (`Rgb`, `Indexed`). Syntax highlighting has its own `editor.theme`.
A GUI has to answer "what is cyan?" itself, and must anyway: the shell and AI panes inside it
print ANSI colours.

## The shape the conversation converged on

**Two front ends on one core.** `varde .` in a terminal is today's TUI. A GUI app (say
`varde --gui .`) is the same panes in the same places in its own window, where:

- the editor, shell and AI panes stay a cell grid (code is monospace; hosted panes must be), and
- the reading and comprehension panes — preview, story, Reading, risk — draw in pixels: real
  fonts, real diagrams, charts.

**Animation belongs to the GUI renderer only.** The core keeps whole rows and says *what is*; the
renderer interpolates between the previous frame and this one. What the core would need is stable
identity for things that move (tree rows keyed by path, say) so old and new can be matched.
Animation needs a redraw source bounded the way ADR 0009 bounds the tick, or the idle-CPU promise
breaks.

**The one core change foreseen:** split `preview::rows` into a shared *document* step and a
TUI-only *lay out to columns* step. The TUI's output stays identical and the existing preview
tests prove it. story, Reading and risk probably need the same split.

**The palette:** the TUI asks the terminal for its colours (OSC 10/11 for foreground and
background, OSC 4 for the sixteen ANSI colours) and saves the answer in Varde's config directory;
the GUI reads it. A `[palette]` section in config is the override, and the fallback for a terminal
or tmux that does not answer. Fonts cannot be queried, so the GUI needs a `font` and `size` key.
This fits the edge rule — the edge asks, the answer comes back as data — the reverse direction of
the replies `queries::reply` already gives children.

## Options weighed and set aside

- **A Neovide-style GUI (the whole window a cell grid).** Neovide is a Rust GPU front end for
  Neovim that draws Neovim's grid with better fonts, smooth scroll and an animated cursor; it still
  looks like a TUI. Cheapest form for Varde: keep `ui.rs` unchanged behind a ratatui GUI backend
  (`ratatui-wgpu`, `egui_ratatui`, `bevy_ratatui` — none checked for maintenance or ratatui 0.30
  support). About 1–2 weeks, no core change. Set aside because it gives sharper text and nothing
  else: no real graphics, no proportional prose.
- **A fully pixel-laid-out GUI.** Would force `layout`, `mouse`, scroll clamping and wrapping — an
  estimated 5–10k lines of the core — off cells, and breaks the one-layout rule. Set aside for the
  hybrid.
- **A companion window next to the TUI,** showing only the rich preview. Rejected by the owner: no
  windows outside Varde.
- **Images inside the terminal** (Kitty graphics protocol, as in Kitty, WezTerm, Ghostty). Real
  diagrams, even real fonts rendered into an image, inside today's pane. Limits: those terminals
  only (needs today's text as the fallback), unreliable through tmux, slow over ssh, text in an
  image is not selectable or searchable, animation is a new image per frame. Still a candidate as
  a cheap add-on for diagrams in the TUI.
- **A web UI (Tauri).** Best typography and animation, native mermaid — at the price of a second
  language and serialising every view model. Set aside.
- **Parsing each terminal's config file for its theme.** A parser per emulator, broken by includes
  and theme files. Set aside for querying the terminal.

## Open questions for the grilling

1. **Toolkit.** egui (redraws from state every frame, built-in animation helpers, easy to mix text,
   images and custom drawing) or iced (Elm architecture — `update`, `view`, messages — the closest
   fit to Varde's own shape, but younger animation support)? Is there a maintained terminal-grid
   widget for either, or does the GUI draw `vt100`'s grid itself?
2. **Which panes go pixel,** and is the line between grid and pixel panes the right one?
3. **Drawing twice.** Every future feature in a pixel pane is drawn for the terminal and for the
   window. Is that cost acceptable, and what keeps the two from drifting — shared view models,
   scenarios asserting on them, something else?
4. **One binary or two.** A `--gui` flag on the same binary (smaller change), or the effect
   executor moved into a shared crate used by two binaries (cleaner, a structural change AGENTS.md
   asks to be justified)?
5. **Palette.** Query-and-save versus config only; where the saved palette lives; what happens
   when the terminal theme changes while the GUI is open.
6. **Which animations actually aid comprehension,** rather than decorate? Candidates raised: smooth
   scroll that keeps your place, a changed line fading in, tree rows sliding into place.
7. **Pixel-pane mouse and scroll.** When the preview is pixel-laid-out, how do its scroll offset
   and hit-testing live in the core without breaking "the renderer and the hit-test read the same
   field"?
8. **Rules that must survive.** Every motion reachable without a modifier, the cheatsheet as
   contract, draw only when something changed — which bind the GUI, which relax?
9. **Is it worth it at all,** and does a spike come first?

## Proposed first step

A 1–2 week spike in egui: only the Markdown preview in the editor pane, fed from the existing
`varde::preview` output, with real fonts, one real mermaid diagram, the terminal's palette, and one
animation (smooth scroll, or a highlight fading in). It tests all three goals on the pane where
they matter most, and touches nothing in the core.

## Rough cost

- A first GUI proving the idea: 1–2 weeks.
- Keeping two front ends (moving effect execution out of `main.rs`): about a week, once.
- A polished hybrid GUI: months, mostly in the renderer and the pixel panes; `update` untouched.
- Packaging: a macOS `.app`, a desktop entry, `install.sh` learning the windowing and GPU
  dependencies.
