# 05 — `install.sh` installs from a Release and asks the binary what it needs

Status: resolved

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

- [x] A fresh machine with no toolchain gets a working `crime` from the binary path with no compile.
- [x] The asset is verified against `SHA256SUMS` before it is installed; a mismatch aborts with a
  message and installs nothing.
- [x] The dependency prompts are driven by `crime --deps` on both paths.
- [x] Re-running on a binary install replaces the binary; re-running on a checkout install pulls
  and builds as today.
- [x] `--list` works with no source present.
- [x] `docs/install.md` and the README describe both paths, binary first.
- [x] AGENTS.md's install rule names `--deps` as how the script learns the table.

## Comments

- Verified by running the script in a sandboxed `HOME` with no `crime` on PATH and
  `CRIME_REPO=file://…` pointing at a fake Release. Tested: a fresh binary install, a re-run
  replacing that binary, a tampered `SHA256SUMS` aborting with nothing written, and "source"
  switching to the build path. Not yet run against a real GitHub Release (issue 01) or on macOS.
- `crime --deps` runs once, before any prompt (`ask_deps`). A binary that lacks the flag or fails
  aborts with a message. Read inside `< <(rows)`, it would have looked like nothing is missing.
- A re-run replaces a binary where `command -v crime` finds it. A symlink that points into neither
  a checkout nor a binary is refused rather than overwritten.
- An SSH `CRIME_REPO` (`git@github.com:…`) is turned into its https form for the Release URL.
- The script and docs now name `oyvij/crime-editor`, which is the repository `RELEASE_URL` uses.
