# CRIME

An IDE TUI: it opens on a folder and presents it as a workspace — a file tree, buffers, a diff
review, a shell and an AI session, side by side in one terminal.

This file is the glossary and nothing else. `AGENTS.md` holds the working contract;
`docs/example-map.md` holds the spec; `docs/adr/` holds the decisions.

## Language

### Selecting and copying

**Selection**:
The text the workspace currently holds as "what you picked". There is exactly one, and it is what
copying copies — a mouse drag and a keyboard extend produce the same thing.
_Avoid_: highlight, marked text, visual selection

**Charwise**:
A selection measured in characters, which may start and end mid-line.
_Avoid_: character mode, inline selection

**Linewise**:
A selection measured in whole lines, ends included. What a review comment covers.
_Avoid_: line mode, block selection

**Anchor**:
The end of a selection that stays put while the other end moves.
_Avoid_: start, origin, mark

**Extending**:
Growing or shrinking a selection by moving the cursor while the anchor holds.
_Avoid_: expanding, dragging out

**Row selection**:
The highlighted row in a list pane — the file tree, or whichever list is in the Corner. It names
something to go to rather than text you picked, so it is never copied as characters and never
becomes the selection.
_Avoid_: selected file, tree highlight

**Corner**:
The one pane-sized slot beneath the file tree, at the tree's width, taking its columns from the
shell. It names its occupant — the Risk list, the Buffers pane or the Cursor history — or nothing at
all, so "both on screen at once" is not a state it can hold and asking for one while another shows
is a replacement. Every occupant is the same rectangle: which pane is in the Corner changes what a click
means, never where the Corner is.
_Avoid_: the risk pane's slot, bottom-left pane, second sidebar

**Copying**:
Putting the selection on the system clipboard, falling back to the terminal over SSH.
_Avoid_: cut, clip

**Yanking**:
Putting text in the register. Yanking also copies; the register is for putting text back inside
the workspace, the clipboard for carrying it out.
_Avoid_: copy (in the vim sense)

**Register**:
The single slot yanking fills and putting reads.
_Avoid_: clipboard, buffer, kill ring

### Moving and finding

**Motion**:
A keypress that moves the cursor without changing the text. Every motion is reachable without a
modifier; modifier bindings are aliases for one.
_Avoid_: navigation, movement command

**In-file search**:
Looking inside the buffer you are editing and moving the cursor to a match.
_Avoid_: local search, find

**Match**:
A place in the current buffer where the in-file search query occurs — somewhere you go.
_Avoid_: hit, result

**Project search**:
Looking across every file in the workspace and listing hits grouped by file.
_Avoid_: global search, grep

**Hit**:
An occurrence the project search found, in a file you may not have open — a place to open.
_Avoid_: match, result

**Result row**:
One row of the project search's box: a file heading, or a hit beneath one. A hit's row is its index
*plus the headings above it*, which is why the renderer and the scroll clamp read the same rows
rather than counting hits.
_Avoid_: line, entry

**Visit**:
One place the cursor has been: a file, a line, a column, and the text that line held at the time.
Always a **source** line, even when it was taken from a Preview, whose cursor is a rendered row — a
row stored there names a line nobody was on and excerpts whatever line of that number happens to
hold. Recorded only by a Jump, and recorded as the place being *left*, so going back returns you
where you were. The text is carried rather than looked up, so a row still says something about a
file that has since been edited — and can be marked Stale rather than claimed to be current.
_Avoid_: mark, position, jump point, breadcrumb, history entry

**Jump**:
A long-distance move of the cursor, made deliberately: opening a file, switching Buffer, an in-file
search landing, the ends of a file. A Jump records a Visit; a Motion does not, and neither does a
click, and neither does browsing the tree — a preview the next preview replaces is a file you passed,
not a file you went to. It is what "back" and "forward" step between.
_Avoid_: navigation, goto, move

**Landing**:
Putting the cursor on a place something else chose: a Visit, a search Hit, a Risk row's line, a
definition, the line a crossing to Preview came from. The place is always a source line, so the
landing is where the crossing into a Preview's rendered rows is made — one for all of them, because
a landing that forgot it left the caret on the top of the render while an invisible second cursor
sat on the line nobody could see.
_Avoid_: goto, seek, reveal

**Cursor history**:
The Visits, oldest first, and a cursor of its own into them — the row the pane highlights and the
position back and forward move. A log of where you have been rather than a tree: a Jump made while
travelling appends rather than truncating what was ahead. This session's, never written down, and
capped, so it stays a list you can read. Its cursor sits *past the newest* Visit whenever you are
somewhere newer than everything recorded, which is no row at all.
_Avoid_: jumplist, back stack, trail, breadcrumbs

**Helper row**:
The dim row inside a box naming the keys that box answers — the results box has one, drawn from the
list that lives beside the router. Inside the box, not in its border: it names the way through what
the box is showing.
_Avoid_: footer, hint, legend

### Showing what is open

**Buffer**:
A file the workspace is holding open, saved or not. Opening never closes the previous one.
_Avoid_: tab, document, editor

**Preview**:
A markdown Buffer shown as the document it describes rather than as the characters it holds:
headings, prose, lists, tables and diagrams, laid out to the pane. Read-only — a Preview is a
rendering, and the file it renders is changed as Source, which is where `i` and `:format` both cross
to before a character moves. Not a View: Edit, Review and Story are where
you are in the workspace, and a Preview is one Buffer's way of being drawn inside Edit.
_Avoid_: formatted view, rendered view, markdown mode, reading mode

**Rendered column**:
Which character of a Preview row the cursor is on, counted in characters of what is *drawn* — never
a column of the source line behind it, which the row map cannot carry. It is the coordinate a
Preview's motions, its caret, its footer position and the sideways offset all speak in, and it
starts over at one whenever the cursor crosses between Preview and Source.
_Avoid_: screen column, display column, x

**Source**:
A Buffer shown as the characters the file holds, markup included. The only way a file is edited: the
switch to Source is the switch to being able to change it.
_Avoid_: raw (that is unsanitised bytes elsewhere in this repo), plain, unformatted

**Buffer mark**:
The glyph on a file tree row saying whether that file is open, current, or unsaved.
_Avoid_: badge, indicator, icon

**Change bar**:
The bar in the gutter beside a line the last commit does not hold — inserted or edited, saved or
not — so every place a file was changed is visible while editing it. It answers the buffer, not the
disk, and a file the commit has no copy of carries none. The quietest of the three marks the gutter's
one column can hold: a diagnostic and the voice's place both take it first.
_Avoid_: gutter indicator, diff decoration, modified marker, git gutter

**Cheatsheet**:
The read-only reminder of the keys, tucked into the editor's top-right and hidden while inserting.
_Avoid_: help, legend, hints, key list

**Minimap**:
A far-off mirror of the whole file down the editor's right-hand edge, two lines to a row and four
source columns to a cell — the shape of the file rather than its text. Press or drag in it to
travel. Not a pane: it takes columns out of the editor's own rectangle and is hit-tested inside it,
the way the gutter is.
_Avoid_: overview, preview (that is a markdown Buffer here), thumbnail, bird's-eye view

**Slider**:
The line down the Minimap's first column, beside the rows the editor's own window covers — where
you are, in the mirror. A line and not a field: a mirror this faint is mostly blank, so anything
painted behind it is the loudest thing on the pane. **Lit** while the pointer is on the mirror,
which is also the whole of the travelling gesture, and quiet otherwise.
_Avoid_: viewport box, highlight, region

**Thumb**:
The line in the editor's last column saying how far through the file the window is — drawn only
where there is no Minimap to read it off, and absent altogether when the whole file is on screen.
One indicator at a time: beside a mirror, the Slider already says it.
_Avoid_: scrollbar (that is the thumb and its track, and there is no track), handle, grip

### Hosting a child process

**Hosted pane**:
A pane whose keyboard, mouse and clipboard belong to the child process running in it. The AI pane
and the shell pane.
_Avoid_: terminal pane, passthrough pane, pty pane

**Reserved key**:
A key CRIME claims from a hosted pane. An exhaustive list, not a policy: a key is reserved because
it is on the list, and everything not on it reaches the child.
_Avoid_: global key, binding, shortcut

**Palette tap**:
A key whose double tap opens the palette. Exactly one is armed per terminal, whichever gesture that
terminal can report. Where the armed one is a key a child could otherwise have received, it is
passed on as well as counted, so it is not a Reserved key.
_Avoid_: escape hatch, fallback binding, chord

### Staying up to date

**Version**:
What a checkout claims about itself. It is a claim, not an observation: it moves when somebody
moves it, and a checkout whose code has changed without it is telling you something untrue.
_Avoid_: release, tag, build number, revision

**Running version**:
What the binary you are talking to was compiled from — the one thing a running CRIME knows for
certain about itself, because it was baked in when it was built.
_Avoid_: current version, installed version, binary version

**Update**:
A Version strictly newer than the Running version: the checkout has moved on and the binary has
not. A Version that is equal or older is not an Update, so there is nothing to offer.
_Avoid_: upgrade, new release, available version, newer build

### Settling the project's answers

**Indent width**:
How many spaces one level of indentation is, as the project says: `editor.tab_width` in the layered
config, four when no layer names one. It is what Tab lays down, and what opening a block *falls back
to* — a file's own shallowest indentation beats it, because the lines already there are better
evidence about that file than a configured number is.
_Avoid_: tab size, tab stop (that is where a Candidate left a blank), shift width, indentation

**Seeding**:
Writing a file the first time CRIME opens a folder, and only when nothing is there — the project's
`config.toml`, whose every key arrives commented out. A key nobody can find is a key nobody sets,
which is the whole reason it is written at all; a live value in it would be this binary's answer
frozen into a file that outlives it — and so would a commented one gone stale, so a test uncomments
them and holds each against the shipped defaults. "Nothing is there" is the config layer the edge
already read being absent, so the decision is the core's and the write is an ordinary one.
_Avoid_: scaffolding, generating, initialising, installing, creating (that is the folder)

**Bare workspace**:
A workspace CRIME writes nothing into — opened by naming no folder at all, which is the whole of how
it is asked for. The folder is still the workspace: the file tree is it, and `:w` writes there.
Everything that would have gone in the project's `.crime/` goes in a Sidecar instead, so a Bare
workspace forgets everything between runs and seeds nothing, measures no Risk unasked, and cannot be
told to remember — one that remembers is a project
(`docs/adr/0016-a-bare-workspace-leaves-nothing-behind.md`).
_Avoid_: bare-bone instance, temp instance, scratch workspace, editor mode

**Sidecar**:
Where a Bare workspace's own state lives — under `~/.crime/paths/`, named for the folder and the
process, deleted when CRIME exits and swept on start when a crash escaped that. It holds what belongs
to CRIME, never what belongs to the user: a Guest repo, a story set, a session's `state.json`. A
submitted review is the one thing that does not go here, because an output destroyed at exit is a
different kind of nothing than a trace not left.
_Avoid_: scratch (taken twice — `.scratch/` is the issue tracker, `~/.crime/tmp/` is a Reading's
audio), shadow, cache, temp folder

### Walking a change

**Range**:
The two revisions a Story set is authored for, and the spelling that names them: a base, a head, and
the text a reviewer typed or CRIME resolved. Always the change the head *introduced* — a merge-base
against the base, never tip against tip, because a base that has moved on since the head forked
would otherwise read as the head deleting everything that landed meanwhile. `worktree` is a legal
head and names the uncommitted change; there is only one working tree, so there is only one of those.
_Avoid_: diff, revision range (say Range), commit range, base..head

**Guest repo**:
A repository CRIME cloned into a Sidecar to author a Story set for a branch of it — somebody else's
code, on somebody else's remote, present for one session. Its files are never in the file tree and
are never saved to: a Guest repo is read, and it is gone at exit. Cloned and fetched by the user's own
`git`, never by `git2` (`docs/adr/0015-a-clone-is-the-users-own-git.md`).
_Avoid_: temp clone, external repo, checkout, scratch repo

**Repository under review**:
Whichever repository a Story set's git questions are asked of — the Guest repo when one has been
cloned, the workspace otherwise (`State::repo_root`). Not the same thing as the workspace: the tree,
a save and what `.gitignore` covers are the folder CRIME was opened on, while a range, a checkout and
a Step's staleness belong to the repository the Story describes.
_Avoid_: workspace root (that is the folder), target repo, current repo

**Story set**:
Every Story authored for one range, plus a title naming what the range accomplishes at large —
a feature added, a bug fixed, an improvement made — so a reviewer with zero context knows what
the whole set is for before reading a single Story. One file, one range, one title (`docs/adr/0005-a-story-dies-with-its-range.md`).
_Avoid_: story artifact, story batch, change (too generic — this is specifically the authored set)

**Story**:
One narrative through a change: a name, a premise and an ordered chain of steps that follows how the
code runs rather than how the files are arranged. Authored, durable for the life of the review, and
readable by someone who did not write it. A Story set may hold several. A Story's name states the
concrete subject — what changed — never a standalone metaphor; the premise is where the *why* goes.
_Avoid_: tour, trail, walkthrough (that is the act), narrative, guide

**Step**:
The atom of a Story — a name, one claim about one Site, with why it exists and what flows in and
out of it. A Step is where the cursor goes and what a comment can be made against. Its name is
short enough to sit in the step-menu; its claim is the full sentence the band narrates.
_Avoid_: stop, node, frame, slide

**Step-menu**:
The list of a Story's Steps, named, shown to the left of the gutter while walking one, with the
current Step marked. Read-only — it shows where you are; `n`/`p` still do the moving. Never shown
while walking the Remainder, which has no names to show.
_Avoid_: step list, step sidebar, outline

**Site**:
Where in the code a Step points: a file, a side of the change, a range of lines, and the text those
lines held when the Story was written. Not an Anchor — that word is already the fixed end of a
selection, and one word for two things is how a glossary stops being one.
_Avoid_: anchor, location, position, target, region

**Walkthrough**:
One person's position in a Story: what has been walked, what was skipped, where they are. Started,
stopped and restarted freely; disposable, and never shared. A Story is authored, a Walkthrough is
lived.
_Avoid_: session, progress, run, playthrough

**Site mark**:
How the code on screen says which lines the current Step points at: the Site's lines carry a bar and
their colour, everything else is flattened to one grey. It is drawn from where the Walkthrough
stands, never asked for and never dismissed — a reviewer sent to a file with no idea which lines the
claim is about is reading a screen of code, not a Step. Not a Selection: it is not copied, not
extended, and it exists whether or not anything is selected.
_Avoid_: highlight, selection, focus, cursor range

**Spine**:
The ordered list of a Story's steps, and of the Stories a Story set holds, under its title. What
the reviewer reads to see the shape of the whole before walking any of it.
_Avoid_: outline, table of contents, index, timeline

**Prediction**:
A question at a Step about *why* - why the code is written this way and not the obvious other way,
why the Story goes where it goes next. Three reasons to choose between, one keypress, never graded,
never blocking, and a wrong pick is told why it is wrong and may pick again. It exists to make the
reviewer commit to a reason, which is what turns reading into comprehension. Never a bet on which
line runs next: that is trivia, and it measures nothing.
_Avoid_: quiz, question, test, checkpoint, score

**Nudge**:
The extra sentence a Step holds in reserve for a reviewer who wants more, written when the Story was
written rather than fetched on demand.
_Avoid_: hint, tooltip, expansion, detail

**Cited value**:
A concrete value a Step uses to make the flow real, carrying the place it was copied from so the
reviewer can go and check it. A value with nowhere to point is invented, and is shown as invented —
the distinction is the whole reason a reviewer can trust the rest.
_Avoid_: example, sample, mock, simulated value

**Stale step**:
A Step whose Site no longer holds the text it was written against. It says so, says what the Site
used to hold, and stops claiming to describe what is on screen; it is never quietly re-pointed at
whatever moved into its place.
_Avoid_: broken, outdated, drifted, invalid

**Coverage**:
Which of a change's hunks the Stories claim. Never a ratio: a percentage needs a denominator, every
denominator is a judgement about which files deserve a reviewer's eye, and a low percentage teaches
the reviewer to ignore the line. Coverage is a count and a list. The word is this and only this:
the tested-lines ratio a CRAP figure needs is *test coverage*, always spelled out, because it is the
ratio this entry exists to refuse.
_Avoid_: completeness, progress, percentage, score

**Remainder**:
The hunks no Step claims, once the Stories are subtracted from what git reports. It is walkable but
it is not a Story - no premise, no claim, nothing authored - and it says nothing about whether
leaving those hunks out was wrong. Deletions are counted separately, so a range that deleted nothing
is distinguishable from a range whose deletions nobody walked.
_Avoid_: gap, leftovers, uncovered, missed

### Paying down risk

**Risk**:
What the workspace says about how hard its own code will be to change safely — measured per Function
and counted across the Scope, computed rather than authored. Not a claim about correctness: code can
be risky and right, and a Risk figure never says a Function is wrong.
_Avoid_: quality, debt, smell, health, score (bare — say which metric)

**CX**:
The Risk metric that needs nothing installed and nothing configured: complexity alone, with no test
coverage in it. What a workspace shows unless it can do better.
_Avoid_: complexity score, CRAP (that name claims more than CX measured)

**CRAP**:
What CX becomes once test coverage is in play — complexity weighted by how much of the Function is
tested. The name is earned by having read coverage, never assumed: a figure computed without it is a
CX figure wearing a better name, and the label is how you tell which one you are looking at.
_Avoid_: crap score, risk score, quality score

**Function**:
The unit Risk is measured against and the unit a refactor moves: a function or method whose
enclosing space is not itself a function. A closure counts toward the Function holding it and is
never listed alone — complexity that can be relocated into a nested space is complexity nobody has
to pay down. Containers are read and never listed, because a container's figure is the figures
inside it counted twice.
_Avoid_: method, symbol, unit, space (the analyser's word for any region, containers included)

**Risk count**:
How many Functions in the Scope sit above the threshold. A count and a list, for the reason Coverage
is one, and the figure the Refactor loop exists to move.
_Avoid_: total, percentage, average, grade

**Scope**:
What a Risk figure or a Refactor loop covers: the whole workspace, or the files under Review. Never
a mix — one figure describing two different sets of files is a figure nobody can act on.
_Avoid_: target, selection (that is text you picked), range (that is a Story set's)

**Refactor loop**:
CRIME's own iteration over an AI session: it hands the session a Scope and a goal, waits to be told
a pass is finished, then measures the workspace and decides whether that pass stands. The deciding
is CRIME's. A session that reports success is reporting what it believes, and belief is not a
measurement.
_Avoid_: agent loop, auto-refactor, AI run, automation

**Iteration**:
One pass of the Refactor loop — one prompt, one set of edits, one measurement, one verdict. An
Iteration whose edits fail the Gate leaves nothing behind in the workspace, so the workspace after a
loop is a workspace where every Iteration passed.
_Avoid_: round, attempt, turn, step (that is a Story's atom)

**Gate**:
What an Iteration's edits must satisfy to stand: the tests still pass, the Risk count or total moved
down, and no other metric moved up. Failing any one of them returns the workspace to what it held
when the Iteration began. The Gate is why "reduce this number" cannot be satisfied by scattering
complexity somewhere the number does not look — which the prompt now says outright as well, so a
session hears it before spending an Iteration on a pass the Gate would put back.
_Avoid_: check, validation, acceptance criteria, guardrail

**Stale figure**:
A Risk figure whose workspace has moved since it was computed. It says so rather than being drawn as
current, and it is never quietly recomputed while you type — a figure that churns per keystroke is
noise, and one that lies about being current is worse.
_Avoid_: outdated, dirty, invalid, pending

**Unparsed**:
A file in a language the analyser handles that it could not read. Counted and shown, never skipped
in silence: a Risk figure that quietly omits what it failed to read is claiming a Scope it never
covered.
_Avoid_: skipped, failed, ignored (that is what .gitignore does), unsupported (that is a language
nobody promised)

### Knowing what the code means

**Language server**:
A child process CRIME hosts to answer questions about code it has no other way to answer — what a
name is, where it is defined, what is wrong with it. Named in configuration and never in a branch, for
the reason a CLI in a Hosted pane is never named (`docs/adr/0011-a-language-server-is-a-second-hosted-child.md`).
It has no pane, so it is not a Hosted pane; it is the other kind of hosted child.
_Avoid_: LSP (that is the protocol, not the process), backend, provider, engine, analyser (that is
what computes Risk)

**Document version**:
Which state of a Buffer a message is about. It is the Buffer's revision — bumped by every content
change and nothing else — sent with the text and quoted back in what the server says about it. A
message naming any other version describes text the user has already edited past and is dropped —
and so is a reply to something CRIME asked, measured against the version it was asked at, which is
the same rule read the other way round.
_Avoid_: sequence, generation, timestamp, revision number (say revision, or Document version)

**Diagnostic**:
Something a Language server says is wrong with a range of a document: a Severity, a message, and the
lines it covers. Pushed rather than asked for, held per file so it survives switching buffers, and
replaced wholesale when the server pushes again — never appended to. It describes the present or it
is discarded; a Diagnostic that outlived its conversation is not a record, it is a lie about the
screen.
_Avoid_: error, warning (those are Severities, not the thing), problem, marker, lint, squiggle

**Severity**:
How much a Diagnostic matters: error, warning, information or hint. Four values, distinguished on
screen, because a hint drawn like an error is a gutter nobody reads.
_Avoid_: level, priority, kind (that is a highlighting token's), type

**Hover**:
What a Language server says the symbol under the cursor *is* — its type, and its documentation if the
server has any. Asked for, shown where it does not cover the symbol it describes, and dismissed. A
symbol the server knows nothing about is told so; an empty box is not an answer. Almost every server
answers in markdown, so a Hover holds the same Rows a Preview does rather than the characters the
server sent — a reply the server labelled plain text is the one that is not read as markdown.
_Avoid_: tooltip, popup (that is the Candidate list's shape), info, docs, quick info

**Definition**:
Where a name is introduced — a file and a line, somewhere to go. A server may answer with several,
and several are places to open, which is what a Hit already is: they are shown in the list the
project search fills, rather than the first being taken quietly.
_Avoid_: declaration, source, target, reference (find-references is a different question, not in scope)

**Candidate**:
One of the things a Language server offers as what you might be typing. Chosen from a list with the
keys the rest of the workspace uses, and inserted as the text it holds — never reformatted, and
expanded only where the server marked it a snippet and CRIME said it could receive one. Dismissing
leaves the Buffer holding exactly the characters that were typed, which is the promise the whole
feature stands on.
_Avoid_: completion (that is the act), suggestion, item, proposal

**Tab stop**:
A place an expanded Candidate left for a value to go, and the sequence of them Tab walks. The
protocol's own word, and the one thing here that outlives the keystroke that made it: while stops
are left, Tab means "the next one" and means nothing otherwise. Escape leaves the stops and keeps
the text, because abandoning a Candidate is not undoing it — and leaving the file or the pane ends
the sequence rather than carrying places into a file they do not name.
_Avoid_: placeholder (that is the `${1:…}` in the reply, not the place in the Buffer), field, blank
(the cheatsheet's word for the gesture, not the thing), marker, anchor

**Trigger character**:
A character a Language server named as one it wants to be told about, so that it can lay out the text
around it the moment it is typed. The server's own list, read off its `initialize` reply and never
spelled in CRIME: rust-analyzer names `.`, `=`, `<`, `>`, `{`, `(`, `|` and `+`, jdtls names `;`, `}`
and a newline, clangd names a newline alone, and gopls and the TypeScript server name none. What
comes back is edits, applied as one thing to undo — and dropped when the reader has typed on since,
which is the Document version rule read the other way round.
_Avoid_: format key, hotkey, on-type character, brace (that is one of them, not the idea), autoformat

**Formatter**:
An external command a project names to lay one language's files out — `prettier`, `black`, `gofmt`.
Named in a `[formatter.<language>]` row and never in a branch, for the reason a Language server is
never named. It is asked only where no server will answer, it is handed the Buffer on stdin and
answers on stdout, and it never touches the file: `:w` is the reader's. One process per `:format`,
so nothing about it is remembered — which is why a Formatter installed a moment ago works a moment
later.
_Avoid_: linter (that reports, this rewrites), prettifier, beautifier, pretty-printer, formatting
provider (that is the server's capability, not this)

**Not measured**:
What a file reads as when nothing has answered for it yet — no server for its language, or a server
that has not spoken. Distinct from zero, which is an answer. Telling a reviewer that a file with no
server is clean is the failure this word exists to refuse; it is the same distinction Unparsed draws
for Risk.
_Avoid_: none, zero, empty, n/a, pending

### Reading aloud

**Reading**:
The act of speaking a document's prose, and the thing that can be in flight, paused or stopped. A
Reading covers the Selection and nothing else — there is no reading from the cursor and no reading
of a whole file, because the passage worth hearing is the one you picked. Starting one while another
is in flight replaces it rather than queueing behind it, the same way a Risk generation supersedes
the answer still coming.
_Avoid_: playback, narration, TTS, speech, audio, play

**Utterance**:
One sentence-sized run of speech within a Reading — what previous and next move by, and what the
marker on screen names. Sentences are the unit because a Reading is built as one continuous stream
with real silence between them: a separate player per sentence leaves a gap the machine chose rather
than one the listener needs, which is audible as staccato and is the reason this word does not mean
"a clip".
_Avoid_: chunk, clip, segment, phrase, sentence (the text is a sentence; this is the speech of one)

**Transport**:
The strip of controls on the editor's top border — play and pause, previous, next, stop, and the
speed. Drawn only for a markdown buffer, right-aligned the way the Risk pane's action icons are, and
clickable. It is an affordance and a reminder, never the only way in: everything it offers has a key
binding, because a control you can only reach with a mouse is one the cheatsheet cannot promise.
_Avoid_: play button, toolbar, controls, player bar, media bar
