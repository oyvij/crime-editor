# A Preview row is not a line

Everything the editor pane does is built on one arithmetic identity: screen row N shows source line
N + `editor_scroll`. Four subsystems rely on it independently — `code_lines` emits exactly one row
per line, `mouse.rs` hit-tests a screen row into a buffer line, `layout::viewport` clamps a scroll
offset against a line count, and `crime::matches` paints a match at a line and column. None of them
share a mapping; each one re-derives the same identity, which is only safe because the identity has
never had an exception.

A Preview is the exception. Markdown reflows: one long paragraph becomes six rows, a fence delimiter
becomes none, thirty lines of mermaid source become one diagram. So the render returns **rows and a
row-to-source-line map**, every row carrying the source line of the block it came from, and the four
consumers read the map instead of the identity.

The cheaper alternative was real and we turned it down. A **line-preserving render** — markup
consumed but one source line still drawn as exactly one row, no reflow — keeps the identity intact
and costs nothing but a column offset. It fails on the files people actually read: a README written
outside this repo has unwrapped paragraphs, and a Preview that runs off the right edge is a Preview
you stop opening. Prose that does not wrap is not a rendered document, it is coloured source with the
markers filed off.

**The second alternative was a crate, and it is the one worth naming.** `tui-markdown` renders
markdown straight to a `ratatui::text::Text`: four thousand lines covering every construct this
feature has tickets for — weighted headings, emphasis, inline code, nested and ordered and task
lists, block quotes with GFM alerts, a thousand lines of table layout, fenced code highlighted
through syntect, footnotes, definition lists, HTML, images, math — with the styling injectable
through a `StyleSheet` trait, so colour would still have been ours. It is maintained, widely used,
and written by a ratatui maintainer. Turning it down cost most of the remaining implementation work,
so the reason has to be good.

It has no offsets. Not a partial map, not an approximate one: it never asks the parser where an event
came from, because nothing that renders markdown *for display* needs to. CRIME is not displaying a
document, it is editing one — the cursor crosses between the two shapes, `/` matches in rows the
reader can see, a drag copies what is on screen, and the scroll clamp bounds a row count. All four
are the map, and a renderer that cannot say which line a row came from cannot answer any of them.

It also has no notion of width. It emits rows and leaves wrapping to `Paragraph::wrap` at draw time,
which makes the row count a lie — the failure `src/ui.rs` already documents for overlay sizing — and
its tables size to their content, so a table wider than the pane runs off the edge instead of
truncating. That is the same "runs off the right edge" the line-preserving render was rejected for
one paragraph above.

The savings were also smaller than four thousand lines suggests. Both answers need span-aware
wrapping, because a row carries styled pieces and `textwrap` cannot re-split them; both need the map.
What the crate genuinely saves is table borders and inline-style bookkeeping. So the render is ours,
and the crates are taken one solved problem at a time — `pulldown-cmark` for parsing, `textwrap` for
prose, `unicode-width` for columns, `syntect` through `highlight` for fences, `mermaid-text` for
diagrams. That is what "use the crate, don't write the function" asks for: the function here is
*width-aware layout that reports its own provenance*, and no crate offers it.

The map is what pays for the rest of the feature, and it is the reason several answers in F26 look
severe. A Preview cursor moves by **row**, because reading wants the next line of prose and not the
next paragraph. Crossing back to Source keeps the **line** and resets the column, because a column is
the one coordinate the map cannot carry: the `## ` a heading no longer shows means screen column 3 is
source column 5, and a cursor that lands three characters off is worse than one that lands
predictably at the start. `/` searches the rendered rows and a drag copies rendered text, for the same
reason in both cases — in a Preview, what is on screen is the document, and a match or a clipboard
that disagrees with it is answering a question nobody asked.

## Consequences

A row carries **styled pieces, not one string**. Emphasis, strong, strikethrough and inline code are
differences *inside* a row, so `Row { kind, line, text: String }` — the shape this decision was first
implemented with — cannot express any of them, and a heading's level has nowhere to sit either. The
shape was right for a tracer bullet and wrong for the feature; widening it is the first move of the
inline-styling ticket, and the pieces name what they *are*, never what colour they are, the way
`highlight::Token` does.

**Nothing renders as nothing.** The map's cost is paid per construct, which makes it tempting to
handle the constructs somebody remembered and let the rest fall through a `_ =>` arm. Four gaps were
found that way, by eye, in a Preview of this repo's own documents: headings all one weight, emphasis
flat, inline code flat, thematic breaks gone. So the classification of the parser's tags is
exhaustive — a construct it grows does not compile until somebody decides what it renders as — and a
sweep drives one sample of every construct against "produces rows, leaks no marker", with an explicit
omissions list naming the ticket that owes each one. `src/keys.rs` has the same sweep over key codes
for the same reason, and the omissions list is what makes leaving one out a decision rather than an
oversight.

**Amended: a Preview cursor is a row *and a rendered column*, and the offset follows it.** The
original decision here was that `editor_hscroll` may not follow a Preview cursor, because the cursor
had no column of its own and the buffer's column was an offset into a line that no longer exists on
screen: the `## ` a heading no longer shows means screen column 3 is source column 5, so a view that
slid to keep *that* cursor visible would slide to a column nobody is looking at. The reason is
sound and it lapses the moment the cursor stops borrowing the source column. A **rendered** column is
a column of exactly what is drawn — `Row::text()` is the same projection `/` matches in and a drag
copies out of — so following it slides to the character the reader is looking at, which is the thing
the original pin was protecting.

What reopened it is a report that a Preview answers only `j` and `k` and sits at column one on every
row, which makes reading a rendered document a strictly poorer experience than reading its source.
This file is named in `AGENTS.md` as a decision nothing may silently contradict, so it is amended
rather than worked around: a Preview answers the motions Source answers — `h l 0 $ w b e gg G` and
the arrows — over the **rendered rows**, and `editor_hscroll` follows the rendered column the same
way it follows Source's. The clamp is `settle`'s, against the row's own character count, so a
Diagram, a Rule and the blank row between blocks all hold the cursor at column one because that is
all the text they have.

Two halves of the original answer survive untouched, and they are the load-bearing ones. **Crossing
in either direction still starts at column one**: the row map carries no source column, so there is
nothing to carry across, and an offset taken while reading a fence must not survive the toggle as a
sideways jump nobody asked for. And the columns are counted in **characters**, not display width,
because everything a column meets here counts characters — `search::occurrences`, `editor::span_text`,
`ui::shift` and the drag that reads them all do. `preview::truncate_pieces` and `fit_cell` use
unicode-width, and that is a different question about how wide a cell *draws*, not about which
character a column names.

The offset was once held at zero outright, which cost the reader the tail of a code fence — the one
part of a Preview that is laid out one row per source line and never wrapped, because a code line
broken at the pane edge is a line the block's own syntax does not permit. `slid_width` stopped it at
the widest row of the whole render. With the offset following a cursor that has a column, that
widest-row clamp is no longer reachable for a Preview and is gone: the fence is read to its end by
holding `l`, which is the gesture Source already uses, and the clamp is `layout::viewport` against
the row the cursor is on.

The render is cached against `(Buffer::revision, pane width)` rather than revision alone. Width is
part of the answer once rows reflow, and AGENTS.md's "never parse per frame" is sharper here than it
is for syntax tokens: a mermaid A* routing pass per frame is visible, not merely wasteful.

The map is a `Vec<usize>` returned by a pure function and asserted in unit tests, not a field anyone
mutates. If a fifth consumer of the row-line identity ever appears, it reads the map or it is wrong —
and the way to find out is that this file exists.

Relative-link navigation, which several markdown readers offer, is not forbidden by this decision and
is not part of F26. It needs the map to answer "which link is under the cursor", so it is cheap to
add later; it also needs a rule about following a path out of an untrusted file, which is a security
question and deserves its own argument rather than a clause here.
