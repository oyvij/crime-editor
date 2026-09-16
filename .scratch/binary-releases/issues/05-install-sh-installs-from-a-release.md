# 05 — `install.sh` installs from a Release and asks the binary what it needs

Status: ready-for-agent

Blocked by: 01, 04

**What to build:** With no `crime` on PATH and no checkout under the script, `install.sh` offers a
binary install by default: fetch `SHA256SUMS` and `crime-<os>-<arch>` from the latest Release, verify, install to `~/.local/bin/crime` as a real file, then run `crime --deps` for
the dependency prompts. Run from inside a checkout, or answered "source" at the prompt, it clones
and builds as today. Re-run on a binary install, it replaces the binary with the latest Release.
`--list` reads `--deps` from whichever binary is installed. The `rows` and `player` functions that
read `src/startup.rs` are deleted.

The README's one-liner fetches the script from `main` and the script fetches the binary from the
Release, both of this repository. `CRIME_REPO` already overrides the repository for a fork and
covers both.

## Acceptance criteria

- [ ] A fresh machine with no toolchain gets a working `crime` from the binary path with no compile.
- [ ] The asset is verified against `SHA256SUMS` before it is installed; a mismatch aborts with a
  message and installs nothing.
- [ ] The dependency prompts are driven by `crime --deps` on both paths.
- [ ] Re-running on a binary install replaces the binary; re-running on a checkout install pulls
  and builds as today.
- [ ] `--list` works with no source present.
- [ ] `docs/install.md` and the README describe both paths, binary first.
- [ ] AGENTS.md's install rule names `--deps` as how the script learns the table.
