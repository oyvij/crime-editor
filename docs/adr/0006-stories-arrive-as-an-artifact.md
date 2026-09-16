# Stories arrive as an artifact, never as parsed output

CRIME asks a hosted AI CLI to write a Story by pasting a prompt into its pane and then waiting for a
file to appear at `.crime/stories/<base12>-<head12>.json`. The existing file watcher picks it up.
CRIME does not read one character of what the CLI printed.

ADR 0004 established that a hosted pane is a terminal CRIME hosts, not a pane CRIME interprets, and
that no branch anywhere may test which CLI is running. It settled what CRIME must not do to a child's
*output*. It did not say how CRIME gets structured data *out* of one, and this feature is the first
that needs to — so the obvious reading of 0004 ("just don't") would have left the question to whoever
implemented it first.

The alternative was to read the pane. It is superficially cheaper — the text is right there, already
in a vt100 grid CRIME owns — and it is the reason to write this down, because it will look cheaper
again to the next person. It fails on three counts. A CLI's output format is its own and changes
without notice, so parsing it is a branch per provider by another name, which 0004 forbids outright.
The grid is a *rendering*: it wraps, it is repainted, it scrolls away, and a story set runs to 30–70KB
against a pane nineteen rows tall, so most of it is gone before the last of it arrives. And there is
no cross-vendor protocol for "here is structured data" to fall back on — we checked, the same way F8
checked for a way to push text into a running session and found only a pty write.

The artifact channel is the same move F8 already makes for review submission, reversed: a file for the
data, the pane for the hand-off. It was measured rather than assumed. Three fresh CLI sessions, given
only the 5.7KB authoring prompt and no other context, produced **83 of 83 byte-exact Sites and 20 of
20 verified citations, with zero problems**, and held execution order in all three. The reason is
worth recording because it is what makes the whole approach viable: all three *scripted* the text
extraction rather than transcribing by eye. A CLI has tools, so its self-check is a program instead of
a plea — and the prompt must say so, because that is the property being relied on.

## Consequences

CRIME verifies the artifact's shape and nothing else. A malformed file is refused whole, with a
notice, never salvaged in part: a spine that silently omits one story of three is indistinguishable
from a story the AI never wrote. A cited value is rendered as a jumpable pointer and an uncited one is
displayed as invented, but CRIME never proves the value is really a literal — that would need a parser
per language, and it is out of scope for good.

The wait is unbounded, and deliberately so. Authoring takes five to eleven minutes, a guessed timeout
is wrong on a slow run and slow on a fast one, and the failure that actually needs catching is not
slowness but a dead CLI — which arrives as `AiExited` from the edge, not from a stopwatch. This is the
same reasoning that made `Event::AiSpoke` replace a fixed delay before typing at a freshly spawned
pty.

Nothing here reads the pane, so nothing here breaks when a provider changes its output, and a provider
nobody has tried works if it can write a file to a path it was told.
