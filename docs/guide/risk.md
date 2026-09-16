# Risk

Risk is what the workspace says about how hard its own code will be to change safely. CRIME
measures it per function, counts how many functions sit above a threshold, and puts that count on
the tree pane's top border where you will see it without asking. The Risk list turns the count
into a worklist, worst first, and the Refactor loop points your AI session at that list — with
CRIME, not the AI, deciding whether each pass was good.

Risk is not a claim about correctness. Code can be risky and right, and a figure never says a
function is wrong.

## The words

- **Function** — the unit Risk is measured against: a function or method whose enclosing space
  is not itself a function. A closure counts toward the Function holding it and is never listed
  alone; containers — impls, classes, namespaces, files — are read and never counted.
- **Figure** — a Function's measured complexity.
- **Risk count** — how many Functions in the Scope are above the threshold. A count and a list,
  never an average or a percentage.
- **Scope** — what a figure covers: the whole workspace, or the files under review. Never a mix.
- **CX** — the metric shown when no test coverage has been read: complexity alone. It is
  labelled `CX` rather than `CRAP` because `CRAP` claims complexity weighted by coverage and this
  figure is not that.
- **Stale figure** — a figure whose workspace has moved since it was computed. It says so, and it
  is still shown.
- **Unparsed** — a file in a language the analyser handles that it could not read. Counted and
  shown, never dropped in silence.
- **Refactor loop**, **Iteration**, **Gate** — below.

## The figure

The metric is cyclomatic complexity per Function, read by the `rust-code-analysis` library. That
library implements the metrics for Python, Rust, C/C++, Java, JavaScript and TypeScript/TSX; a
workspace in no language it handles is `nothing-analysed` and shows no count, because a fabricated
zero is a clean bill of health nobody was given. Alongside the primary figure CRIME records each
Function's cognitive complexity, maintainability index and line count, because the Refactor
loop's Gate watches all of them.

### When it is computed

- **On opening a project** (`crime <folder>`), without being asked. The border shows a spinner
  while the job runs and the count when it lands. The result is cached against the commit it was
  measured at, so reopening on the same commit runs nothing and reopening after the commit moved
  runs it again.
- **When you ask**, with the Risk pane's recompute action. A recompute runs whether or not the
  figure is stale, and a recompute while one is in flight supersedes it rather than queueing —
  two analyses of two different states cannot both be true.
- **After every Iteration** of the Refactor loop, so the Gate measures rather than believes.
- **On entering Review view**, for the files under review only — see below.

Never on a keystroke and never on a save. Saving marks the figure **stale**: the border says so,
and the stale count stays up until you recompute or the commit moves. A figure that churned as you
typed would be noise; one that lied about being current would be worse.

A Bare workspace (`crime` with no folder) measures nothing at startup, because the result would be
written into a Sidecar deleted at exit. Its border reads `nothing-analysed` until you ask; the
recompute action still works.

### The threshold

A Function counts toward the Risk count when its figure is above `threshold`, under `[risk]` in
your config. The default is 15. A codebase with different norms sets its own bar; see
[Configuration](configuration.md) for where the file lives and how project and global settings
layer.

```toml
[risk]
threshold = 15
```

## The Risk list

The list is a pane beneath the file tree, at the tree's width, taking its width from the shell
pane. Toggle it from the palette with `k`: opening it moves focus into it, closing it returns
focus to the tree and gives the shell its width back. Whether it is open is remembered per
project.

With the pane open, `Alt+j` from the tree reaches it and `Alt+j` again reaches the shell below;
`Alt+h` from the shell comes back to it. With the pane hidden `Alt+j` from the tree reaches the
shell directly.

### Rows

The list is flat and sorted by figure descending, across the whole Scope. By default it shows
only Functions above the threshold — it is a worklist, not an inventory — and `a` toggles showing
every Function, for reading a figure that is not yet a problem. Each row names a Function, the
line it starts on and its figure. The selected row's file is shown in the pane's border, because
the pane is narrower than a path. The pane also shows how many files were Unparsed, so you know
when the figure describes less than the whole workspace.

While a job runs and nothing has been measured yet the pane says `computing`. With a stale figure
it shows the stale list, marked stale, until the fresh one lands — an old answer beats no answer,
labelled.

| Key | In the Risk list |
|---|---|
| `j` `k` / `Down` `Up` | move the selection |
| `Enter` | open the file with the cursor on the Function's first line, and follow it into the editor |
| click a row | the same, but focus stays in the pane |
| `a` | show every Function / only those above the threshold |
| `r` | recompute the figure |
| `l` | start the Refactor loop — or stop it while one runs |
| `j` past the last row | step onto the pane's own actions; `Left` `Right` choose, `Enter` runs, `k` steps back up |
| `Alt+h` `Alt+j` `Alt+k` `Alt+l` | move focus between panes |

The wheel scrolls the list without moving the selection; any key pulls the selection back into
view. The selection names a place to go, not text you picked, so it is never copied and a drag
over the pane selects nothing.

### Row action: ask for one refactor

Each row carries an action — on its icon, or via the same gesture the file tree's row actions use —
that asks the AI session for a suggested refactor of that one Function. It is one prompt carrying
the name, the file and the figure, asking for splits that stand on their own and naming a helper
called from exactly one place as a failed pass. Then it is yours to review: no Iteration, no test
run, no Gate, no revert, nothing committed. If no session is running one is started and the prompt
is held until it is ready.

### Pane actions

The pane's own two actions live on its border, where the figure is, and are reached by clicking
their icons or by stepping down off the end of the list:

- **recompute** — measure the Scope again;
- **start the Refactor loop** — which becomes **stop** while a loop runs.

An empty list starts with the keyboard on these actions, because a Scope with no rows is exactly
where the recompute is the only thing left to reach.

## The Refactor loop

The loop hands your AI session a Scope and a goal — bring the Functions above the threshold down —
waits to be told a pass is finished, then **runs the project's tests itself**, **recomputes the
figures itself**, and applies the Gate. It never asks the session whether the pass was good. A
session reporting success is reporting what it believes, and belief is not a measurement; the
reasoning is `docs/adr/0010-crime-owns-the-test-gate.md`.

Start it with the pane's action (`l`, or its icon). It is refused, visibly, when:

- **no test command can be determined** (`no-test-command`) — a Gate that passes having run
  nothing is the one failure worse than no Gate;
- **nothing has been measured yet** (`no-figure`) — the Gate takes its baseline from the last
  figure, and a loop judged against zero could only ever read as no improvement. Recompute first.
  A *measured* zero is a fine baseline;
- **a loop is already running** (`loop-already-running`) — two loops editing the same files is
  two agents fighting.

### An Iteration

1. CRIME deletes the sentinel `.crime/refactor-done` if one is left over, so a stale one cannot
   complete the pass before it starts.
2. It snapshots the files under `.crime/snapshots/`, per Iteration.
3. It sends the session one prompt naming the Scope, where the figures are written
   (`.crime/risk.json`), the worst few Functions, the target threshold, what a good split is, and
   an instruction to follow whatever convention file the repository holds. The prompt never
   contains the test command and never names an AI provider. If no session is running one is
   started and the prompt held until it speaks.
4. It waits for the session to write `.crime/refactor-done`. The file's contents are ignored; its
   existence is the signal. **There is no timeout.** A session thinking for forty seconds looks
   exactly like one that finished, and timing out would mean measuring a half-written edit. The
   pane says what it is waiting for — `session`, `tests`, `measuring` — so an indefinite wait is
   distinguishable from a hang.
5. When the sentinel appears CRIME runs the test command off the shell pane, so the shell stays
   yours, then recomputes the figures for the Scope.
6. The **Gate**: the tests still pass, the primary figure moved down, and no other recorded
   metric moved up. "Moved down" counts either the Risk count or the total — chipping a Function
   down without yet crossing the threshold is progress. A pass that meets all three **stands, in
   the working tree, uncommitted**, and the next Iteration begins. A pass that fails any one is
   **put back from its snapshot and the loop stops**, saying which condition stopped it:
   `tests-failed`, `no-improvement` or `other-metric-worsened`.

The third condition is the defence against gaming. Shredding one long Function into fifteen
trivial ones lowers the count while cognitive complexity holds or rises, and is reverted. The
prompt says so up front to spare an Iteration; the Gate is what enforces it.

A revert goes to the snapshot, never to the last commit, and covers only the files that Iteration
touched: a file you had edited and the loop did not is never restored over, and your own
uncommitted edits in a file it did touch come back as yours. A reverted Iteration is explained to
the session as well as to you — the failing condition and the tests' output — so the session does
not build its next answer on an edit it believes landed.

### The cap and the stop

The loop runs at most `max_iterations` passes (default 10) and says `cap-reached` when it stops
for that reason.

```toml
[risk]
max_iterations = 10
```

Stop is the start action flipped, in the same place — the pane's icon, or `l`. Not `Esc`, which
does nothing to a running loop: it is too overloaded a key for something midway through editing
your files. Stopping puts back the Iteration in flight and leaves the workspace where the last
accepted Iteration left it — nothing half-applied.

### Reading a run

While a loop runs the pane shows the Iteration and the cap, the figure as it moves, and the last
test result; the tree border shows the live count. Once it is over the verdict stays on the pane
until the next measurement — a recompute is a fresh measurement, so it clears a finished run's
verdict; a running loop's own verdict is left alone.

**Nothing is ever committed.** The loop's entire output is one dirty working tree, and
[Review view](review.md) is where you judge it.

### The test command

The Gate runs the project's tests. Where the command comes from, first match winning:

1. `test_command` under `[risk]` in your config;
2. otherwise the shape of the project:

| File present | Command |
|---|---|
| `Cargo.toml` | `cargo test` |
| `package.json` | `npm test` |
| `pyproject.toml` | `pytest` |
| `go.mod` | `go test ./...` |
| `pom.xml` | `mvn test` |
| `build.gradle` / `build.gradle.kts` | `gradle test` |
| `Makefile` | `make test` |

A Rust project that also has a `Makefile` gets `cargo test`. If neither the config nor the table
names a command, the loop is refused.

```toml
[risk]
test_command = "cargo nextest run"
```

## Risk in Review view

Entering [Review view](review.md) measures the files under review — that Scope and never a mix —
against the revision the diff is measured from, and the border shows the **delta** rather than
the workspace's count. A change that raised the figure is marked worse, because that is the
signal a reviewer most wants and least reliably gets from reading a diff. Leaving the view puts
the workspace's count back. A stale figure is recomputed for the reviewed files rather than drawn
as a stale delta.

With the Risk list open it lists the Functions in the changed files, each with its figure and its
delta, and no Function from outside the change. A reviewed file in a language the analyser does
not handle contributes nothing and is never reported as an improvement.

The pane's action here starts the loop over exactly those files. It is the same machinery with a
different file set: the same three-condition Gate, the same per-Iteration snapshot, the same cap,
the same stop, nothing committed, and no file outside the Scope is touched. One loop at a time
across both Scopes.

## What lives under `.crime/`

| Path | What it is |
|---|---|
| `.crime/risk.json` | The last figure: the commit it was measured at, the metric (`CX`), and every Function with its file, name, first line and all four recorded metrics — `cyclomatic`, `cognitive`, `maintainability`, `lines`. The loop's prompt points the session here. |
| `.crime/snapshots/` | Per-Iteration copies of the files the loop let the session touch, restored on a failed Gate. |
| `.crime/refactor-done` | The sentinel the session writes to say a pass is finished. Deleted before each Iteration. |

CRIME never writes a git ignore rule for these or anything else. Whether to ignore them is your
project's decision, in your `.gitignore`.

## See also

- [Review](review.md) — where the loop's output is judged, and the delta on the border.
- [AI pane](ai-pane.md) — the session the loop drives.
- [Configuration](configuration.md) — `[risk] threshold`, `max_iterations`, `test_command`.
