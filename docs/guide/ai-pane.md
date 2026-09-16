# The AI pane

The right-hand pane hosts an AI CLI — Claude Code, opencode, anything that runs in a terminal — as
a program in its own terminal, inside CRIME's. It is a hosted pane: its keyboard, mouse and
clipboard belong to the program running in it, and CRIME's job is to carry your keys in and its
screen out faithfully. A review you submit and a Story you ask for both arrive there as a prompt.
The frame around it — panes, the palette, focus — is in [getting-around.md](getting-around.md);
what a review is and how to write one is [review.md](review.md); Stories are [stories.md](stories.md).

## Starting a session

With nothing running, the pane is a small box asking which CLI to start, prefilled with the one you
used in this project last time — or, with no history, the `ai.command` setting, which defaults to
`claude`. `Enter` starts it; typing replaces the suggestion. While no session runs, keys typed into
the pane belong to the box and never reach your shell.

| Gesture | Effect |
|---|---|
| palette `a` | focus the AI pane. It starts nothing — the box is already there |
| `:ai` | start the remembered CLI, or focus the session already running |
| `:ai <command>` | start that CLI. Refused, with a notice, if a different one is already running |
| `:ai! <command>` | stop the running session and start that CLI instead |

Switching CLIs mid-session is deliberate, so it takes the `!` — the same idiom as `:q!`. The CLI you
chose is remembered in the project's state, so reopening the project offers it again.

### Tall

Beside the editor, above a terminal that spans the whole width, is the default shape. An AI CLI is a
long conversation, and reading one two-thirds of a screen tall is the wrong shape, so the pane can
take the whole right-hand edge instead — the terminal gives up width rather than the AI pane giving
up rows.

| Gesture | Effect |
|---|---|
| palette `l` | swap between beside-the-editor and the whole right-hand edge |
| `:tall` | the same, from the command line |

Choosing the palette entry leaves focus where it was, which matters because you want the shape
while reading the pane, and a hosted pane's child owns the colon — `:tall` would mean leaving the
pane first. The shape is remembered per project, and a width you dragged the pane's edge to is the
width in either shape.

## What reaches it

A running session gets ordinary typing, and it gets it exactly as a terminal would send it: `Alt+Enter`
arrives with its modifier intact, `Shift+Tab` is `Shift+Tab`, and `Ctrl+Q` and `Ctrl+F` reach the
CLI rather than quitting CRIME or opening its search. The keys CRIME keeps for itself are an
exhaustive, written-down list — the Reserved keys — and everything not on it is the child's:

| Reserved key | Why CRIME keeps it |
|---|---|
| `Ctrl+Space` | opens the palette from every pane. The CLI loses one NUL byte |
| `Alt+h` `Alt+j` `Alt+k` `Alt+l` | move focus between panes |
| `Ctrl+C` while a Selection exists in this pane | copies it. With nothing selected it interrupts the child, as in any terminal |

`Esc Esc` opens the palette too, and is not on the list: both escapes still reach the CLI, and only
the pair CRIME counted beside them opens the palette. From the palette, `o` returns you to the
editor, `e` `r` `s` change view, `q` quits — none of it needs a modifier, which is what makes this
pane leavable on a terminal where Option is not Alt.

The environment the CLI sees is CRIME's, not your host terminal's: `TERM` is `xterm-256color`, and
the markers a terminal emulator exports about itself are removed, so a CLI that sniffs for
`TERM_PROGRAM` or `TMUX` gets an answer about CRIME rather than about your machine. Asked by
escape sequence, CRIME answers as `CRIME(x.y.z)`, declines advanced modifier reporting out loud, and
answers cursor-position and device queries — a program that hears silence picks the dumbest fallback
it has and never says so.

## Mouse

When the CLI asks for mouse events — full-screen AI CLIs do — clicks and the wheel are reported to
it in the encoding it asked for. A click lands on its *release*, and only if the pointer did not
move in between, so starting a drag to select text never activates whatever the drag began on. The
legacy encoding some programs ask for cannot name a cell past column or row 223; a click there is
declined rather than sent to the wrong place.

The wheel goes to the CLI only while it has asked to be told about the mouse. A full-screen program
runs on the terminal's alternate screen, where there is no history for CRIME to show, so the program
scrolls itself. When the CLI has not asked for mouse events, a click just focuses the pane and stays
CRIME's.

### Clicking a link

Hold the jump modifier — `Cmd` on macOS, `Ctrl` elsewhere; both are accepted — and click a URL the
CLI printed, and it opens in your browser. Nothing reaches the CLI. Only `http` and `https` URLs
count, and only one row of the screen at a time: a URL the terminal wrapped onto the next row is not
recognised, and clicking plain text with the modifier held opens nothing.

## Copying from the pane

Drag across the pane to select, then `Ctrl+C` or `Cmd+C` puts the text on the clipboard — over SSH,
on the machine you are sitting at. The Selection is the pane's own screen, not the terminal's below
it, and a drag across rows takes both ends and the rows between. A click drops it.

The pane selects only what is on its screen. An AI CLI runs on the alternate screen, where the
terminal keeps no scrollback, so a drag cannot reach past the top of the pane; to copy more, scroll
the CLI itself and select again. The obvious fix — keeping a transcript of the CLI's output to
select over — does not work: what a full-screen program prints is a stream of redraws and cursor
moves, not a transcript, and replaying it as text reconstructs garbage.

## Pasting into it

Use your terminal's own paste gesture (`Cmd+V` on macOS). The text arrives at the CLI as one send.
If the CLI asked to be told a paste from typing — bracketed paste, which every modern CLI does —
the text is wrapped in the paste markers, so a newline in the middle is not read as a submit. A
program that did not ask gets the text bare. A paste cannot smuggle the end of its own bracketing:
the closing marker is stripped from the text before it is wrapped. With no session running, a paste
reaches neither the box nor your shell.

## What CRIME sends it

**A submitted review.** `:submit` in Review view, after you confirm, writes the review to
`.crime/reviews/NNNN.json` and sends the CLI a prompt — an inline summary of the comments with their
file and line ranges, plus the artifact's path — and submits it, Enter included, so the CLI starts
working the moment you confirm. It asks first because sending clears whatever the CLI's prompt line
is showing, which may be a half-written message CRIME cannot read back. With no session running,
the remembered CLI is started first and the prompt waits until the CLI is ready to take input —
never typed at a program that has not printed its prompt yet. From a Bare workspace the artifact
goes to `~/.crime/reviews/` instead.

**A Story.** `:story` resolves a revision range, confirms it, and pastes an authoring prompt asking
the CLI to write a story file into `.crime/stories/`. The file watcher picks the file up; CRIME
never reads what the CLI prints. See [stories.md](stories.md).

**A merge.** When a Buffer with unsaved edits diverges from its file on disk, `D` then `m` hands the
CLI both versions and asks it to merge them. The Buffer stays flagged until the CLI's write comes
back through the watcher.

**A refactor.** The Risk list's row action asks the session for a refactor of one function, and the
refactor loop hands it a scope and a goal; see [risk.md](risk.md).

What does *not* happen: nothing comes back. CRIME does not read the CLI's screen to learn whether a
review was addressed or a story finished — the artifact on disk is the whole channel. A review
queued for a session that exits, fails to start or is replaced with `:ai!` before it is ready is
dropped rather than handed to the next session.

## When the CLI exits

The pane goes back to the box, prefilled with the command that just ran, ready to start another. A
CLI that cannot be started at all — a name that is not on your PATH — leaves the pane asking in
exactly the same way, prefilled with what failed, and `:ai` is not refused afterwards on the grounds
that something is "already running". Keys typed at a pane with no session reach nothing.

## No special cases

Nothing in CRIME tests which CLI is running in the pane — not by name, not by version, not by
sniffing its output. Every key, mouse report, paste and terminal query is carried faithfully or
refused out loud, and that is the whole interface. It is what makes a provider nobody has tried
work the same way as the ones that have, and why a newly bound key in your CLI needs no CRIME
release.

The consequence for you: if a CLI misbehaves inside CRIME, the fault is in the transport, and fixing
it there fixes it for every CLI. Things worth checking before reporting one:

- **A modifier that does nothing.** On macOS, Option is not Alt unless your terminal is told so
  (Ghostty: `macos-option-as-alt = true`), and a stock tmux strips modifier reports. Every gesture
  CRIME owns has a modifier-free route — the palette — but a CLI's own Alt bindings need the terminal
  to send Alt.
- **Clicks leaving stray characters in the prompt.** The CLI asked for one mouse encoding and is
  getting another. That is a CRIME transport bug, and the same one for every program.
- **A shortcut that seems disabled.** It is not gated on CRIME advertising anything — that was
  measured. Either the key is on the Reserved list above, or it never arrived.

Report what the CLI received, not what it is. The pane cannot tell one program from another, and
that is the point.
