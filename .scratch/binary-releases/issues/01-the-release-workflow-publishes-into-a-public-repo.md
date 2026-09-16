# 01 — The release workflow publishes a Release of this repository

Status: ready-for-human

**What to build:** `.github/workflows/release.yml`, drafted in this pass and committed beside this
ticket. Nothing is created by hand: the workflow uses its own token, publishes to this repository,
and needs no secret or variable. What remains is human: run it once by dispatch and look at what
it made.

Read the spec's *Implementation Decisions* for the shape. In short: a push to `main` that changes
the minor or major Version builds four native binaries, tags `v<Version>`, and creates a Release
with the assets, `SHA256SUMS` and generated notes.

## Human steps

- [ ] Trigger the workflow by hand once (`workflow_dispatch`) and confirm: four assets or three
  with the arm64 Linux leg skipped, `SHA256SUMS`, a tag, notes.
- [ ] Check the Linux binary runs on the beelink and the macOS one on the Mac.
- [ ] If `macos-15-intel` or `ubuntu-22.04-arm` is not offered to this repository, change the
  runner label and note the reason in the workflow's comment.

## Acceptance criteria

- [ ] A push to `main` whose Cargo.toml bump is a patch produces no release and no tag.
- [ ] A push whose bump is minor or major produces a tag and a Release.
- [ ] A Version that already has a tag makes the publish job fail before it uploads anything.
- [ ] Every asset is named `crime-<os>-<arch>` with exactly the four spellings the core's unit test
  pins (issue 02).
- [ ] The Release carries `SHA256SUMS` covering every asset.
- [ ] The arm64 Linux leg failing for want of a runner does not stop the other three.
