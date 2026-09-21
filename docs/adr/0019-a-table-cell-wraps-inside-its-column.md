# A table cell wraps inside its column

A markdown table narrower than its natural width used to lose text. `shrink_to_fit` took a column
off the widest column until the row fit the pane, with a floor of one column per cell, and
`fit_cell` then cut any cell still wider than the width it had been allotted. The characters were
gone before anything was drawn: absent from `Row::text()`, so unreachable by scrolling, by `/`, and
by a drag that copies what is on screen. A table in a narrow pane — tree open, AI pane open — could
reach one character per cell. This is the failure glow was reported for in
[charmbracelet/glow#941](https://github.com/charmbracelet/glow/issues/941), which asks for exactly
what this decision does: truncate visibly or flow onto further lines, but never silently erase a
column.

`table_rows` recorded the reasoning that led there:

> A column too wide for `columns` shrinks — never wraps, since a wrapped cell would destroy the
> alignment a table exists to show.

That rules out the wrong operation. Two different things are called wrapping here, and only one of
them destroys alignment:

- Reflowing the **already-laid-out flat row string**, so a row's second half lands under its first
  with no idea where the columns were. Alignment is gone completely, and no renderer does this.
- Wrapping **inside a fixed column width**, so one source row occupies as many display rows as its
  tallest cell needs and its shorter cells are padded down that height. Alignment is preserved
  exactly — it is the only reason the columns can stay put at all.

The note rejected the first, correctly, and took the second down with it, leaving truncation as the
only move left. Every renderer worth comparing against wraps first and treats overflow as the
fallback: an HTML `<table>` under auto-layout wraps cell text at the column width and grows the row
taller, scrolling sideways only when an unbreakable token makes wrapping useless; **glamour**, the
closest comparison since it is a terminal renderer under the same constraint, has `WithTableWrap`
**true by default** and offers ellipsis truncation as the explicit opt-out; **Rich** wraps to fit
and makes `no_wrap` opt-in per column. Varde went straight to the last rung, by default, with no way
out of it.

So a cell wraps inside its column. One source row becomes N display rows, each column padded to its
width on every one of them, and a column's declared alignment applies to each row of a wrapped cell
rather than only its first — alignment is a property of the column, not of the first line.

## Consequences

**A word too long for its column is not broken; it overflows.** `wrapped` takes `textwrap`'s options
rather than a width now, and a cell passes `break_words(false)` where a paragraph does not. The row
then renders wider than the pane and is clipped, exactly as an unwrapped long code line already is —
and a clipped row is recoverable by scrolling sideways, where a cut one is not. That distinction is
the whole of this decision: overflow is visible and reversible, truncation is neither.

**The floor is a readable width, not one column.** A narrow column now costs height instead of
content, which changes what shrinking too far means: not a lost cell but a stack of fragments.
`MIN_COLUMN` is four, and a column narrower than its own natural width is never widened to reach it.
Below four, most words overflow the width they were given anyway, so shrinking further buys nothing
and costs legibility.

**Truncation is gone from `preview` entirely.** `fit_cell` became `pad_cell`, which only pads, and
`truncate_pieces` was deleted with its last caller. The shortfall it used to leave — a display-column
budget does not divide evenly by a double-width glyph — is still there for a wrapped row and is
padded away by the same arithmetic. `ui::truncate` still cuts, with an ellipsis, for a file name in
the tree: that is a name in a fixed strip with no second row to flow onto, and it says it has been
cut.

**A table that fits the pane is untouched.** This is the regression the change is most likely to
cause, so it is pinned: same widths, same padding, one display row per source row.

`columns` of zero — the terminal has not reported a size yet — still shrinks nothing and wraps
nothing, the convention `rows` itself uses.

Related: #35, the sideways scroll gesture that makes an overflowing row reachable. This decision does
not depend on it: without it, an overflowing row is clipped, which is where an unwrapped code line
already stood.
