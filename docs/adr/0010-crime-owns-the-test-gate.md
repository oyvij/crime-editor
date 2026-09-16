# CRIME owns the test gate

The Refactor loop points an AI session at the workspace and asks it to lower a number. That is a
program editing your files in a sequence you are not watching keystroke by keystroke, so the whole
design question is: **who decides that a pass was good?**

The tempting answer is the session itself. One prompt — reduce the Risk count, run the tests, keep
iterating until you cannot improve it — and CRIME merely hosts the pane. It is less code, it needs no
test runner, and it is what every agent CLI is built to do.

It cannot work here, for a reason this repo has already written down. `docs/adr/0004-hosted-panes-are-transparent.md`
forbids reading what a hosted pane prints — not as a style rule, but because a branch that inspects a
child's output is a branch that has a provider in it. So CRIME cannot see the session claim success,
and would not be entitled to believe it if it could. A session reporting that the tests pass is
reporting what it believes. Belief is not a measurement, and the thing being risked is your code.

**The decision: CRIME hands over a Scope and a goal, and CRIME decides.** Each Iteration snapshots the
files it is about to let the session touch, hands the session the Scope, waits to be told the pass is
finished, then runs the project's tests itself, recomputes the Risk figures itself, and applies the
Gate. Edits that fail the Gate are returned to the snapshot. Edits that pass it stay in the working
tree — **uncommitted, always**.

**Never committing is the constraint the rest hangs off.** CRIME's reason to exist includes reviewing a
change before it lands; a loop that commits its own work leaves nothing to review and quietly promotes
an agent's fifteen passes into history nobody read. So the loop's entire output is one dirty working
tree, and the Review view is where it is judged. This also means the loop cannot be trusted *because*
it is gated — it is gated so that what reaches your review is at least still green.

## What was rejected

**A quiescence timer for "the pass is done."** Silence is not completion: an agent thinking for forty
seconds looks exactly like an agent that finished. Completion is made a *filesystem* fact instead — the
session writes a sentinel, and CRIME's existing watcher sees it. A file is something the edge may
observe without reading a pane, and any CLI that can edit files can write one.

**`git stash`, or `git checkout HEAD -- .`, for the revert.** Both destroy uncommitted work that was
yours before the loop started. The snapshot is per Iteration and covers only the files that Iteration
touched, so a file you had edited and the loop did not is never restored over.

**A timeout on the wait.** Timing out mid-refactor means measuring, and possibly reverting, a
half-written edit. The loop waits indefinitely; the stop action and the Iteration cap are the exits.

**Gating on one metric.** Cyclomatic complexity is trivially gamed: shred one long function into
fifteen two-line functions and every one of them falls under the threshold while the code gets harder
to read. Cognitive complexity does not fall for that, because it charges for nesting and comprehension
rather than counting branches. So the Gate requires the primary figure to improve **and** no other
recorded metric to worsen. This is Goodhart's law arriving on schedule, and the answer to it is
structural: an objective that cannot be gamed, not a prompt that asks nicely. The prompt now *states*
that intent as well — a split whose pieces do not stand on their own is named as a failed pass, and
scattering into helpers called from one place as the shape that fails — but stating and enforcing are
different jobs: the wording spares an Iteration spent on a shredded pass the Gate would revert
anyway, and the row action's one-shot ask has no Gate behind it at all, so there the wording is all
there is. Nothing about it relaxes the third condition.

## Consequences

**The prompt is one generic constant and carries no project-specific fact.** Notably it never carries
the test command, because CRIME runs the tests — which removes the most project-specific string of all
from a prompt that has to work on any workspace. What it does carry is the Scope, where the figures
are written, and an instruction to obey whatever convention file the repo already has.

**The sentinel is deleted before each Iteration begins.** A leftover from the previous pass would
complete the next one instantly, scoring edits the session had not started.

**A failed Iteration is explained to the session, not only to the user.** The loop stops, but the pane
stays open; a session that believes its reverted edit landed will build whatever you ask next on a
false premise. Telling it is the same "never silent" rule applied to the agent as a consumer of errors.

**The loop is parameterized by Scope from the start.** Whole workspace and files-under-Review are the
same machinery with a different file set, and the Review case is arguably the more valuable one: it
stops risk arriving rather than paying it down later.

**Nothing here names a provider.** The loop drives whatever `:ai` drives, through the same
`SpawnAi`/`SendKeys` path a typed prompt takes. If a particular CLI cannot write a sentinel file, the
loop stalls visibly and the manual gate finishes the pass — the failure is in the transport, not in a
branch about which CLI it was.
