# 02 — A binary install asks for the latest Release, and an Update comes from it

Status: ready-for-agent

**What to build:** Startup decides the install kind. With a known checkout, nothing changes. With
none, startup returns an effect asking the edge for the latest Release of this repository;
the edge performs it with `curl` and hands the body back as an event; the core parses the body,
compares the Release's Version to the Running version with the same `semver` ordering the manifest
comparison uses, picks this platform's asset by name, and raises the existing Update marker.

Read `docs/adr/0017-a-binary-install-updates-itself-from-a-release.md` first: it is why the
checkout path stays offline and why the binary path makes exactly one request.

The shapes, settled during the seam sketch:

```
Effect::CheckRelease { url: String }
Event::ReleaseAnswered(Option<String>)      // None: the request failed; logged at the edge

State { release: Option<Release> }
Release { version: String, asset: String, checksums: String }   // two URLs
```

Startup gains `arch` beside `os`, from the edge's `std::env::consts::ARCH`. The asset name is one
function of `(os, arch)` and a unit test pins the four spellings the workflow's matrix uses, naming
`.github/workflows/release.yml` so the two are read together.

The URL the effect names is a constant in the core — this repository's latest-release API — and
the request is unauthenticated: sixty a hour is the anonymous limit, and one per launch is nowhere
near it.

## Acceptance criteria

- [ ] A start with no checkout manifest returns `CheckRelease`; a start with a known checkout does not.
- [ ] A body naming a Version strictly newer than the Running version makes an Update available and
  remembers the Release with this platform's asset and the checksum list.
- [ ] A body naming an equal or older Version leaves no Update.
- [ ] A body that does not parse, and an answer with no body, leave no Update and raise no notice.
- [ ] A Release with no asset named `crime-<os>-<arch>` leaves no Update and raises no notice.
- [ ] The four asset names are pinned by a unit test.
- [ ] `CONTEXT.md` gains Install kind, Release, Asset, Relaunch.
- [ ] `docs/example-map.md` gains F39 with an entry-points row.
