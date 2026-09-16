# Hosted panes are transparent

The AI pane and the shell pane host a child process, and CRIME used to interpret their keys the way
it interprets the editor's. Every keypress was collapsed into a fourteen-variant enum of the shapes
the editor was bound to, and whatever the enum had no name for reached its catch-all variant and was
dropped. So Option+Enter submitted a prompt instead of writing a newline — the Alt was gone before
the router saw the key — Shift+Tab did nothing, and clicks left literal characters in a CLI's prompt
because SGR reports went to a child that had asked for legacy tracking. Fixing those three would
have left a fourth, discovered the same way: by the user.

A **hosted pane** is now a terminal CRIME hosts, not a pane CRIME interprets. Its keyboard, mouse
and clipboard belong to the child, and CRIME claims an exhaustive, written-down list of **reserved
keys** — the four Alt focus keys, a bare Ctrl double-tap, and the copy key while a selection exists.
Everything else is transport. A key is withheld because it is on that list and for no other reason,
which is what makes adding one a visible decision rather than a silent narrowing, and one unit test
holds the whole input type to it: every key code in all sixty-four modifier combinations, against
both hosted panes, must either produce bytes or appear on the reserved list or on the list of shapes
no legacy xterm sequence exists for — where a real terminal would send nothing either.

This inverts what the key router claimed. Sending a key to a child is byte transport, not an
interpretation, so it never belonged in a router — but the router's stated reason for owning it,
that focus gating should be one rule rather than two, put it there, and the enum in the middle ate
the modifiers. The single focus rule stays; only the transport moved.

The trade-off is CRIME's own bindings inside these two panes, given up on purpose. `Ctrl+Q` does not
quit, `Ctrl+F` does not open the project search, and `Ctrl+Space` does not open the palette while a
child owns the keyboard: they are not on the list, so they reach the child. The alternative is a set
of keys CRIME takes from every CLI forever, and that set only ever grows — a key CRIME keeps is a
key some provider binds, and the user cannot tell a claimed key from a broken one.

Losing `Ctrl+Space` cost a way out, because the terminals that reject the Kitty flags are exactly
the terminals that never report a bare `Ctrl` press either; with Option not sending Alt there was
briefly no way out of a hosted pane at all. Nothing was free to reserve there — every key such a
terminal can report is one an xterm would have sent to the child — so the way out is a double-tapped
`Esc`, forwarded to the child **and** counted. That makes it a **palette tap** rather than a fourth
reserved key, taking nothing from the child at all. Giving `Ctrl+Space` back was rejected (it costs
the child NUL, which readline and emacs bind, and it is a modifier binding, which is what this was
meant to stop being the only way), as were a function key (a key some CLI binds) and requiring three
escapes (still interferes, harder to discover). Exactly one tap is armed per terminal — `Ctrl` where
modifier key events are reported, `Esc` where they are not — because arming `Esc` everywhere opens
the palette *over* a child that binds `Esc Esc` itself, and from that moment the keys are CRIME's.

## What answering the child's queries actually did

A terminal is two-way, and CRIME had never answered a child anything. The leading hypothesis behind
this work was that Shift+Tab mode cycling was *off* rather than merely unreachable: the CLI probed
at startup for advanced modifier reporting, heard silence, and disabled the bindings that need it.
That reasoning was sound and the conclusion was wrong, which is why the measurement is recorded here
and the hypothesis is not.

Claude Code v2.1.240 was driven through a real pty twice — once with CRIME's replies active, once
with every reply suppressed — and behaved identically both times: the "(shift+tab to cycle)" hint
appeared from the first frame, and `ESC [ Z` cycled `auto mode on` → `manual mode on` → `accept
edits on` in both runs. Mode cycling was never gated on a reply. It was gated on the key arriving,
which is the transport above. So no follow-up work about advertising modifier reporting is owed, and
that out-of-scope decision now has a measurement behind it instead of a guess.

CRIME answers the queries anyway, and not as a consolation prize: an unanswered question leaves a
program guessing, and a child's behaviour inside CRIME is meant to be a fixed target rather than a
function of the user's setup. Cursor position, terminal status, primary and secondary device
attributes and the version query are answered, and each modifier-reporting resource xterm defines is
refused out loud rather than by silence — a resource number xterm does not define gets nothing,
since asserting a value for a resource CRIME has never heard of is not refusing anything. The device
attributes name a deliberately low VT220 so every channel says the same "no": claim a recent xterm
and a program will enable `modifyOtherKeys` and then read CRIME's legacy key bytes under the wrong
rules. The kitty keyboard query is left unanswered for the same reason — that protocol defines no
negative reply, and its own detection procedure reads an answered device-attributes request arriving
without a keyboard answer as "unsupported". Answering queries is correct work for its own sake. It
just was not the cause of the reported symptom.

## The identity has a third channel

A child asks what terminal it is in twice: with an escape sequence, and by reading its environment.
Asked by name (XTVERSION), CRIME answers `CRIME(x.y.z)` — the honest answer, and the only one no
program can act on: a name no program knows is a name no program branches on, so the child takes
the generic path it already takes from an unset `TERM_PROGRAM`, while the question stops going
unanswered. It is still a name sitting beside a `TERM` that says xterm, so it does not make the
channels agree; it adds nothing they have to be checked against. Naming an xterm version would be
the one reply that *does* change behaviour, and it would change it wrongly: inviting the
`modifyOtherKeys` the device attributes refuse, which is the asymmetry below, widened.

Both answers are CRIME's rather than the host's — `queries::CHILD_ENV` sets `TERM` and `COLORTERM`
and removes every emulator marker the host exported, by name, because `TERM` alone left a CLI that
sniffs `TERM_PROGRAM` or `KITTY_WINDOW_ID` reading the user's machine. That list will go stale, and
that is tolerable here and nowhere else: emulators are few and slow-moving, and a missed marker costs
a cosmetic difference where a list of CLI providers would cost a broken feature.

The two channels are colocated, not proven consistent — nothing checks them against each other, and
they are not fully agreed today: `TERM=xterm-256color` invites a program to set `modifyOtherKeys`
without asking, while the device attributes above answer a VT220 that would refuse it. A program
that sets the mode blind, rather than querying, will read CRIME's legacy key bytes under the wrong
rules. No reported symptom traces to it, and the fix is not a lower `TERM` — that costs colour and
key names every CLI needs — so it is written down here rather than guessed at.

## Consequences

No provider-specific code, anywhere — the rule is in `AGENTS.md`, and this decision is what makes it
enforceable. A provider CRIME has never been run against works for the same reason the others do,
and a newly bound provider feature needs no CRIME release.

The reserved list is the whole interface. Read it before adding a binding: a new global key either
goes on it, with the child's loss argued out loud, or it does not exist in a hosted pane. The
coverage test will not tell you which — it only insists you choose.

The wheel is the child's only while the child can be told about it. A child that declared no mouse
encoding, or a cell the legacy encoding has no byte for, leaves the wheel scrolling CRIME's own
scrollback: transparency cannot mean reporting a click nobody made.

Best-effort is honest here, precision is not. `:submit` clears the AI prompt without knowing what is
in it, because reading a foreign CLI's prompt means screen-scraping per provider, which is the
treadmill this decision exists to end. It asks first instead, so nothing is destroyed unannounced.

A future reader will find that two panes bypass the key router and will want to route them through
it for symmetry. That is this decision, reversed.
