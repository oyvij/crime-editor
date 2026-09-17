# Stories

A diff shows you what changed. A Story tells you how the change *runs*: the AI in your right-hand
pane reads the change and writes a narration of it — a handful of Stories, each a chain of Steps
that follows the code in execution order rather than file order, each Step pointing at a Site in
the diff and making one claim about it. You walk a Story with the code on screen, and you can
comment on any Step exactly as you would in [Review view](review.md).

CRIME does not write Stories itself and never parses what the AI prints. It asks the AI to write a
file, waits for the file to appear, checks it, and walks it.

## The words

- **Story set** — everything authored for one Range, under a title that says what the whole
  change is for. One file, one Range, one title.
- **Story** — one narrative through the change: a name, a premise, an ordered chain of Steps.
- **Step** — one claim about one Site, with a why and what flows in and out. Where the cursor goes
  and what a comment is made against.
- **Site** — a file, a side of the change (old or new), a range of lines, and what those lines
  held when the Story was written.
- **Spine** — the list of Stories in a set and, under each, its Steps, read to see the shape of
  the whole before walking any of it.
- **Remainder** — the hunks of the change that no Step claims.
- **Prediction** — a three-way *why* question at a Step, never graded.
- **Range** — the two revisions a Story set is about: what the head introduced over the base.

## Asking for a Story set

| Command | What it does |
|---|---|
| `:story` | Story this change. Resolves a Range for you and confirms it. |
| `:story <range>` | Story an explicit Range, e.g. `:story main..HEAD`. |
| `:story! <range>` | Re-author a Range that already has a Story set. |
| `:story?` | Pick a branch of this repository and story what it introduced. |
| `:story? <url>` | Clone a repository you do not have — a guest repo — and pick one of its branches. |

Palette `s` switches to Story view and shows whatever set is loaded; with nothing authored it says
so.

### What `:story` chooses

A bare `:story` looks at the working tree:

- **Dirty tree** — the Range is your uncommitted change against `HEAD`, spelled
  `HEAD..worktree`.
- **Clean tree** — the Range is what your branch introduced over the default branch, which is
  resolved offline in a fixed order: `origin/HEAD`, then the upstream branch, then
  `init.defaultBranch`, then a branch called `main`, then `master`. The first one that exists wins.

If none of those resolve, authoring is refused with a notice rather than guessed — a wrong Range
costs you five to eleven minutes of the AI's time on the wrong commits. An explicit Range git
cannot resolve is refused the same way, never shown as an empty list.

A Range is always *what the head introduced*: the merge-base against the base, never tip against
tip, so a base branch that moved on after yours forked does not read as your branch deleting
everything that landed meanwhile.

### Confirming

Whatever the Range, CRIME shows it and asks before authoring, because sending the prompt clears
whatever the AI's command line is showing.

| Key | In the confirmation |
|---|---|
| `Enter` or `y` | author |
| `Esc` or `n` | decline — nothing is sent, nothing is written |

Confirming pastes the authoring prompt into the AI pane and submits it. If no AI session is
running, the configured one is started first and the prompt is held until it is ready for input
(see [AI pane](ai-pane.md)). Beside the story file CRIME also writes a companion
`.crime/stories/<base>-<head>.context.md` and names it in the prompt, so the AI has the change in
front of it.

### A Range already authored

`:story` on a Range that already has a set on disk **loads it** — instantly, offline, without
touching the AI. Only `:story!` re-authors, and it confirms first, because re-authoring costs
minutes, clears the AI's prompt and discards your position in the old set.

## While it writes

Story view says `authoring` and waits. There is no timeout: authoring takes five to eleven minutes
on real changes and a guessed limit would be wrong on a slow run and slow on a fast one. What ends
the wait is the file arriving, the AI exiting (the view says `authoring-abandoned`), or you
pressing `Esc`.

When the file lands CRIME checks four things that need no judgement: every Site's file exists,
its line range fits inside the file, a Site that claims a change really overlaps one, and a cited
value really appears on the line it cites. A set that fails is **handed back** to the AI with a fix
request naming only the failing Steps, and CRIME keeps waiting. Two rounds; if the second artifact
still fails, the set is refused with what failed. A file that does not parse at all — a missing
title, a Step without a name, a Prediction without exactly three choices — is refused whole, never
salvaged in part: a spine quietly missing one Story is indistinguishable from a Story the AI never
wrote.

## Where a Story set lives, and how long

Sets are written to `.crime/stories/<base>-<head>.json`, named for the two revisions as twelve-hex
prefixes — `aaaaaaaaaaaa-bbbbbbbbbbbb.json` for a committed Range, `aaaaaaaaaaaa-worktree.json`
for an uncommitted one. The ten most recent are kept, with their companion files; older sets are
pruned when a new one is written.

**A Story dies with its Range.** CRIME never updates a Story to follow the code. When a Step's Site
no longer holds the text it was written against, the Step says so, shows what the Site used to
hold, and stops claiming to describe what is on screen — it is never quietly re-pointed at
whatever moved into its place. Walk the same code next month and somebody authors it again; that
is the cheap side of the trade, because a Story generated per Range is never *wrong*. The
reasoning is `docs/adr/0005-a-story-dies-with-its-range.md`.

A Story set is not hand-written, and nothing in CRIME will help you make one last. If you want
durable, curated tours of a codebase, that is a different feature.

## The spine

The spine lives in the left-hand pane, where the file tree is in Edit view. Under the set's title
each Story is listed with its step count and, when some of its Steps no longer match the code,
a stale count. Below the Stories sits the Remainder.

| Key | In the spine |
|---|---|
| `Up` `Down` | move between Stories, and one slot further to the Remainder |
| `Enter` | walk the selected Story, or the Remainder |
| `t` | toggle the pane between the spine and the changed-files list |
| `j` `k` | scroll |

`t` joins the changed-files list to the spine rather than replacing it, so you can look at the
plain diff of a file from Story view. There is no filter box on the spine.

### The Remainder

The Remainder is what the Stories did not reach: a count of the hunks no Step claims and a list of
the files they are in. It is never a percentage — every denominator is a judgement about which
files deserve your eye, and a low one teaches you to ignore the line. Twelve unclaimed hunks reads
as twelve things to look at.

A Site claims a hunk by overlapping it, not by containing it; a Site of kind `context` — code the
Story merely passes through — claims nothing; two Stories claiming the same hunk count it once.
The Remainder is recomputed as the change grows under it, so a set authored for your uncommitted
work stays honest while the AI keeps writing. Deletions get a line of their own, present only when
the Range deletes something, so "this change deleted nothing" and "twelve lines went and nobody
walked them" cannot look the same.

Walking the Remainder steps through the unclaimed hunks as bare locations: no claim, no
Prediction, no step menu.

## Walking a Story

Enter a Story from the spine and focus moves to the editor. The cursor lands on the first Step's
Site and the view is framed so the whole Site opens downward into the pane, with a little context
above it. The Site's lines carry a mark and their colour; everything else on screen is dimmed to
one grey. The mark says whether the Site is changed code or context. A band beneath the code
carries the Step's claim and, when it has them, its cited values. To the left of the gutter a step
menu lists every Step of the Story with the current one marked — it shows where you are; the keys
below do the moving.

| Key | While walking |
|---|---|
| `n` `p` | next and previous Step |
| `j` `k` | scroll the code, without stepping |
| arrows | move the cursor through the code, without stepping or editing |
| `l` `h` `0` | slide the code sideways, and home |
| `d` | show and hide the range's diff over the Site: added lines in green, removed lines as red rows where they were |
| `D` | open and close the Step's detail: claim, why, flow, and the nudge when there is one |
| `g` | jump to a cited value's source |
| `Ctrl+P` `Ctrl+N` | jump back and forward through cursor history |
| `c` | comment on this Step |
| `e` | open the Step's file in Edit view |
| `t` | show the changed-files list in the left-hand pane |
| `:submit` | send the review, as in Review view |
| `Esc` | close an open overlay, or leave the Story for the spine |

The code is read-only while walking; the mode label says so. Stepping past the last Step stays on
it rather than leaving the Story.

### Cited values

A Step may name concrete values to make the flow real. Each carries where it was copied from, and
`g` jumps there. A value with nowhere to point is displayed as **invented** — the distinction is
the whole reason you can trust the rest. CRIME checks that a cited value's characters are on the
line it cites and nothing more; whether it is really a literal is not something CRIME can tell.

### Stale Steps

A Step whose Site no longer holds the text it was written against is **stale**. The band warns
before you read the claim, and `D` shows what the Site used to hold beside what it holds now.
Three things make a Step stale: its file is gone, its line range no longer fits, or the text
changed. Whitespace-only reformatting does not count. Editing the file in Edit view makes the Step
stale as soon as the text differs.

A stale Step keeps everything but the claim to describe the screen: it still narrates, still
opens, still takes a comment — often exactly where a comment belongs. A Step on the *old* side of
the change cannot show its text on screen and says so; it is immune to the working tree moving,
except where an uncommitted Range is committed and the base moves under it.

### Where you left off

Your position in a Story survives leaving it and restarting CRIME. It is discarded when the set is
re-authored — carrying a position across a rewrite would land you on a different claim while
telling you it is where you left off — and the view says so.

## Predictions

At a Step the author chose, arriving puts a question over the code: *why* is the code written this
way and not the obvious other way, or why does the Story go where it goes next. Exactly three
reasons to choose between.

| Key | In a Prediction |
|---|---|
| `1` `2` `3` | pick a reason |
| `n` | dismiss it and step on |
| `p` | step back |
| `Esc` | close it |

A wrong pick shows why that reason is wrong and leaves the choices up, so you can pick again as
often as you like. Only a correct pick replaces the choices with its explanation, and it cannot
be undone by picking again. Nothing blocks: `n` steps on with the question unanswered.

A Prediction already put — answered or skipped — is not asked again when you return to the Step.
Re-authoring puts it afresh. CRIME records only *that* a Prediction was put, never which choice
you picked: a history of wrong answers is a score by another name, and this is a reviewer reading
a colleague's change.

## Commenting from a Story

`c` on a Step opens the same comment box Review view uses, on the Step's Site: pick `i`, `n`, `s`
or `c` for `ISSUE`, `NOTE`, `SUGGESTION` or `COMMENT`, type the body, `Ctrl+S` to file. The
comment records the Story and Step it was made against as well as the file, line range, type and
reviewed revision, so a submitted review says which *claim* you rejected. A filed comment stays
drawn under its line for as long as that file is on screen, wherever the walk has moved.

Comments made while walking and comments made in Review view are one review. `:submit` from
either view sends them all; see [Review](review.md#submitting).

## Storying a branch

`:story?` lists the branches of the repository — local and remote-tracking, deduplicated by short
name, most recently committed first — in a picker.

| Key | In the branch picker |
|---|---|
| type | narrow the list; `Backspace` widens it again |
| `Up` `Down` | move the selection |
| `Enter` | check the branch out and story what it introduced |
| `Esc` | close |

Picking a branch **checks it out**, then resolves the Range as what that branch introduced over
the default branch and asks you to confirm as usual. Checking out is necessary: a Step's
staleness is judged by what its lines hold on disk, so a set for a branch that is not checked out
would report every Step stale. CRIME does not check the original branch back out afterwards — a
second checkout can fail if files were touched during the review — and Story view names which
branch it is on and which one it left.

The picker is refused on a dirty working tree with an instruction to commit first, because CRIME
never checks out over unsaved work. CRIME's own `.crime/` files do not count as your work. It is
also refused in a folder that is not a repository.

## A Bare workspace

`crime` with no folder opens the current directory as a **Bare workspace**: the folder is the
workspace and `:w` still writes into it, but nothing of CRIME's is written there. No `.crime/` is
created, no config is seeded, and everything CRIME needs for itself goes into a Sidecar under
`~/.crime/` that is deleted when CRIME exits. A Bare workspace remembers nothing between runs —
that is the point of it.

Two things follow for reviews and Stories. A review submitted from a Bare workspace is written to
`~/.crime/reviews/`, outside every workspace, so the output of your reading is not deleted with
the Sidecar. And a Bare workspace is where you review a repository that is not on your machine.

## Guest repos

In a Bare workspace, `:story? <url>` clones the repository at that URL — a **guest repo** — into
the Sidecar and lists its branches in the same picker. The clone runs as **your own `git`**, in
the shell pane, so your SSH config, per-host keys and agent all work without being set up twice,
progress is visible, and a passphrase or host-key prompt can be answered. Story view says
`cloning` until it finishes; a clone that fails is refused with its exit status rather than left
waiting. `git` has to be installed; without it the command is refused.

Picking a guest branch checks it out *in the clone* and resolves the Range against the guest
repo's own default branch. Walking opens the guest repo's files inside the clone, and their Sites
are judged against the clone. Those files are read-only: a file that is deleted when CRIME quits
is a file editing cannot help, and every key that would change one is refused out loud. The guest
repo never appears in the file tree — the tree is the folder you started in.

A second `:story?` on the same URL in one session **fetches** into the copy already there rather
than cloning again, which is what puts a branch pushed since the clone in the picker; a fetch that
fails keeps the copy you have. A bare `:story?` after a guest repo is this folder's own branches
again. At exit the Sidecar and the clone go with it — nothing is left behind.

`:story? <url>` in a project workspace (`crime <folder>`) is refused: a guest repo needs a Bare
workspace.

## See also

- [Review](review.md) — the diff, the comment box, `:submit`.
- [AI pane](ai-pane.md) — the session that authors, and what happens when it is not running.
- [Getting around](getting-around.md) — the palette, panes and focus.
