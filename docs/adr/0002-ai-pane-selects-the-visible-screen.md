# The AI pane selects only what is on screen

An AI CLI runs full-screen on the alternate screen, where vt100 keeps no scrollback — the same
reason the wheel is forwarded to it rather than scrolling history. So a drag in the AI pane can
only cover the grid that is currently visible: to copy more, scroll the child and select again.

The obvious fix is to shadow the pty's byte stream into our own scrollback and select over that.
It does not work. What arrives from a full-screen program is a stream of redraws and cursor moves,
not a transcript; replaying it as text reconstructs garbage. This is the same reason a multiplexer's
copy-mode cannot help you inside a full-screen app.

Recorded because a future reader will try to drag past the top of the AI pane, find they cannot,
and reach for the scrollback buffer that looks like the answer.
