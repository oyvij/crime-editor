@AGENTS.md

## Agent skills

### Issue tracker

Issues live as GitHub issues on `oyvij/crime-editor`, driven with the `gh` CLI; the old `.scratch/` tracker survives only in git history. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical roles, unrenamed, applied as GitHub labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root, both created lazily. See `docs/agents/domain.md`.

## Running long commands

Run the test suite in the foreground with a long timeout (up to 10 minutes), or in the background
and wait for its completion notification. Never write a `pgrep`/`sleep` loop to wait for it:
`pgrep -f "cargo test"` matches the loop's own command line, so the loop never ends — an
`/implement` run sat on one for minutes after the suite had already passed.
