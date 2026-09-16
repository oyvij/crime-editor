# One selection, shared by the mouse and the keyboard

CRIME had grown four separate notions of "selected": mouse-dragged text feeding the clipboard,
a linewise visual selection feeding vim's register, a linewise diff selection feeding review
comments, and the file tree's current row. Yanking never reached the system clipboard and dragging
never reached `p`, so "copy what I picked" meant different things depending on which hand you used.

We collapsed the first three into **one selection**: it is charwise or linewise, it is set by a
drag or by extending with the keyboard, and it is what copying copies. Yanking still fills the
register — putting text back inside the workspace is a different job from carrying it out — but it
now also copies. The tree's row selection stays separate on purpose: a filename is not text you
copy character by character.

## Consequences

The two representations do not merge. A buffer's selection is anchored to line and column so it
survives scrolling; a pty pane's is anchored to the screen grid, because a pty has no buffer to
anchor to. They are one concept with two shapes, and code that assumes a single type will be wrong.

A buffer span includes both of its ends, as vim's charwise span does. That is what lets the cursor
reach a line's last character while normal mode still clamps it to the line's width — a half-open
span could never cover it.
