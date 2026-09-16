# Issue tracker: Local Markdown

Issues and specs for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01` — never a single combined tickets file
- Triage state is recorded as a `Status:` line near the top of each issue file (see `triage-labels.md` for the role strings)
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## What this tracker is not

`AGENTS.md` says the cucumber suite is the single source of truth for what is *done*, and that no
hand-written progress checklist may live anywhere, because it drifts. That rule still holds. This
tracker records work that is *intended* — a spec to write, a question to settle, a ticket to pick
up. It never records feature status. Do not mirror scenario or feature completion into `.scratch/`:
run `cargo test` instead. `docs/example-map.md` tracks the spec; the suite tracks the work; this
tracker tracks the queue.

Issue files are committed — they are shared artifacts, not per-user scratch state.

## Closing a ticket

The five triage roles in `triage-labels.md` have no "done": on a hosted tracker, finishing a ticket
means **closing the issue**, and `/triage` only ever closes for `wontfix`. A file-based tracker has
no close, so this is it — and without it, finished tickets sit at `ready-for-agent` forever. Ten of
them did, which is how nine shipped features came to look untouched.

When the work is merged and the suite is green:

- Rewrite the `Status:` line to `resolved`. Same vocabulary the wayfinding section below already
  uses, so the tracker has one word for "out of the queue" rather than two.
- Append what the next reader needs under `## Comments` — the decision taken, the binding chosen,
  the thing that turned out to be wrong. `/implement` does not do this; it implements and commits.
  The rationale is the part no diff carries, and it is why these files are committed.
- Tick the `- [ ]` acceptance criteria you actually implemented and verified — this is the only place
  ticket-level progress is visible, and a ticket closed with boxes still unticked is as unreadable
  as one left at `ready-for-agent`. Tick a box only once its behaviour is covered by a passing test;
  never tick ahead of the suite.

`resolved` is a queue state, never a claim about the code — `cargo test` is still the only thing
that says a feature works. The boxes are not a second copy of that claim: they record *which* of a
ticket's criteria this pass covered, for a ticket only partly done or picked up by someone else.
This is different from the hand-written *feature-status* checklist `AGENTS.md` bans elsewhere: that
rule is about not shadowing the suite's pass/fail with a parallel tracker that can disagree with it;
a ticket's own criteria, ticked only as the suite confirms them, cannot drift the same way.

## Do not use git worktrees for tickets

Issue 10 was implemented in a worktree under `.claude/worktrees/`, and renaming this repo's folder
broke the registration — git stores worktree paths absolutely — so its branch went unmerged and
invisible while `main` kept the old code. The finished work had to be recovered by reading a dead
process's arguments. Nothing in the skills asks for a worktree: `/implement` says "commit your work
to the current branch". Work tickets on a branch in this checkout.

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/` (creating the directory if needed).

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or the issue number directly.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a file with one **child** file per ticket.

- **Map**: `.scratch/<effort>/map.md` — the Notes / Decisions-so-far / Fog body.
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`, with the question in the body. A `Type:` line records the ticket type (`research`/`prototype`/`grilling`/`task`); a `Status:` line records `claimed`/`resolved`.
- **Blocking**: a `Blocked by: NN, NN` line near the top. A ticket is unblocked when every file it lists is `resolved`.
- **Frontier**: scan `.scratch/<effort>/issues/` for files that are open, unblocked, and unclaimed; first by number wins.
- **Claim**: set `Status: claimed` and save before any work.
- **Resolve**: append the answer under an `## Answer` heading, set `Status: resolved`, then append a context pointer (gist + link) to the map's Decisions-so-far in `map.md`.
