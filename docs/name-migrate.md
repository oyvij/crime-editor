# Renaming CRIME → Los (`los-editor`)

CRIME becomes **Los**, on a maritime-guidance metaphor: a *los* is a maritime pilot who boards a
ship and guides it through hazardous water. The binary, the crate, the on-disk state directories,
the release assets and the name the terminal answers to all move with it.

This is a plan, not a record of work done. Nothing here is implemented. Six decisions (§1) are
open and belong to the user; the rest is mechanical once they are answered.

Survey at the time of writing: **3,552 case-insensitive `crime` hits across ~110 files.**

## 1. Decisions to make first

These change what the rename *is*, not just how it is typed. Recommendations in bold.

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D1 | The backronym | CRIME is an acronym (`Command · Review · Integrated · Modal · Editor`) in `Cargo.toml`, `README.md` and clap's `about`. Los is a word. | **Drop the acronym.** Tagline: *"A los is a maritime pilot who boards your ship and guides it through hazardous water."* It fits the nouns already in the product — Review, Story, Risk, Reading — without renaming any of them. |
| D2 | On-disk state directories | `.crime/` (project) and `~/.crime/` (home) become `.los/` and `~/.los/`. Hard cut, in-product fallback, or installer migration? | **Installer migration.** `install.sh` moves `~/.crime` → `~/.los` when the new one is absent; a project's `.crime/` is the user's to move, and is mostly regenerable state. A fallback path in `src/` would need its own scenarios and would never be deleted. |
| D3 | Release-asset compatibility | Every installed binary self-updates by looking for `crime-<os>-<arch>` in `oyvij/crime-editor`'s latest release. Both halves change. | **Ship a transition release** that uploads *both* `los-*` and `crime-*` asset names for two or three versions, then drop the old spelling. See §4 — without it every existing install goes quiet. |
| D4 | The XTVERSION reply | `queries.rs` answers a child `\x1bP>\|CRIME(x.y.z)`. | **`LOS(x.y.z)`.** `docs/adr/0004-hosted-panes-are-transparent.md` requires naming itself honestly, and uppercase matches the XTVERSION convention. Two unit tests pin it. |
| D5 | The version bump | 0.159.0 today. State directories and asset names move, so this is breaking. | **1.0.0**, per AGENTS.md's "major for a breaking change". |
| D6 | The local checkout path | `/home/oyvindj/crime`, and baked into the test fixtures (`/home/me/projects/crime`) and the four README SVGs, whose workspace title reads `crime`. | Rename the checkout to `~/los` after the GitHub rename, update the fixtures, regenerate the SVGs. |

## 2. What has to change, by category

### A. Crate and binary identity

`Cargo.toml` (`name`, `description`, `repository`), clap's `#[command(name, about)]` in
`src/main.rs`, ~380 `crime::` paths across `src/` and `tests/`, and `target/release/crime`.

### B. On-disk state

`CRIME_DIR` in `src/lib.rs`, `crime_dir()`, the `crime_home` field and the `GLOBAL_LABEL` /
`PROJECT_LABEL` constants in `src/startup.rs`, `~/.crime/{voices,reviews,paths,src}`, the
`.crime-update-` temporary prefix in `src/main.rs`, the seeded and template config text, and the
eight or so user-facing error strings that name `.crime/config.toml` or `~/.crime/config.toml`.

### C. Distribution — the load-bearing part

- `.github/workflows/release.yml`: four asset spellings, `cp target/release/crime`,
  `sha256sum crime-*`, and the release title `CRIME $VERSION`.
- `src/startup.rs`'s `format!("crime-{os}-{arch}")`, pinned against that workflow by the unit test
  `asset_names_match_the_release_workflow`. Change both or every binary install reports no asset.
- `src/startup.rs`'s `RELEASE_URL` → `oyvij/los-editor`.
- `install.sh`: the `CRIME_REPO` and `CRIME_BIN` environment variables, `$HOME/.local/bin/crime`,
  the asset spelling, the default checkout `~/.crime/src`, the raw.githubusercontent URL, and the
  **two checkout-detection greps for `^name = "crime"`**.
- `features/self_update.feature` encodes that same manifest rule, so an existing checkout whose
  crate is named `crime` stops being recognised as Los's own. That is correct behaviour; say so in
  the release notes rather than working around it.

### D. Terminal identity

`src/queries.rs`'s XTVERSION reply and its two tests. `CHILD_ENV` needs no change: it names
emulators, not the product.

### E. Test scaffolding

`CrimeWorld` (1,138 occurrences) → `LosWorld`; "CRIME starts in the project" and its siblings
across 57 `.feature` files; the fixture paths `/home/me/projects/crime` and `/home/me/src/crime`.

### F. Prose

`AGENTS.md`, `CLAUDE.md`, `CONTEXT.md`, `README.md`, 19 ADRs — including the *filename*
`docs/adr/0010-crime-owns-the-test-gate.md` — the eight files under `docs/guide/`,
`docs/install.md`, `docs/example-map.md`, `docs/stack.md` and `docs/agents/issue-tracker.md`.

### G. Assets

The four `docs/images/*.svg` each render the workspace title `crime`. Regenerate them with
`examples/shot.rs` once D6 is settled.

### H. Outside the repository

Rename the GitHub repository, update `git remote`, repoint the issue and label docs, and update the
agent memory note that names `oyvij/crime-editor`.

## 3. Execution order

A blind `sed s/crime/los/g` is wrong in at least three known places, so the sweep runs
most-specific-first and ends with a manual pass over the leftovers:

1. `crime-editor` → `los-editor`; `oyvij/CRIME` → `oyvij/los-editor`
2. the asset spellings `crime-{linux,macos}-*` → `los-*`
3. `~/.crime` and `.crime` → `.los` — **excluding `.crimean-war`**
4. `CrimeWorld` → `LosWorld`
5. `crime::` → `los::`; `crime_home` / `crime_dir` / `CRIME_DIR` → `los_home` / `los_dir` / `LOS_DIR`
6. `CRIME(` → `LOS(`
7. bare `CRIME` → `Los`; bare `crime` → `los`
8. by hand: the backronym lines, the README title, `Cargo.toml`'s description, the `/home/me/…`
   fixtures, the SVGs, and ADR 0010's filename (`git mv`)

**Known traps.** `src/story.rs` asserts that `.crimean-war/notes.md` is *not* one of Los's own
directories — a deliberate near-miss that a blind replace turns into the nonsense `.losan-war`.
Rename it to a plausible `.los`-prefixed near-miss such as `.lost-and-found/`. ADR 0010's filename
needs a `git mv` plus every link to it, including the one in `AGENTS.md`.

**Commits.** AGENTS.md requires the version bump in the same commit as the change it describes:

| | Contents | Bump |
|---|---|---|
| C1 | An ADR recording the rename and the compatibility cut, plus `CONTEXT.md` vocabulary | patch |
| C2 | The mechanical rename of `src/`, `tests/`, `features/`, `examples/` — suite green, no behaviour change beyond the moved paths | major → 1.0.0 |
| C3 | Distribution: `release.yml`, `install.sh` including the `~/.crime` → `~/.los` move, `RELEASE_URL`, the dual-asset transition | minor |
| C4 | README, guide, AGENTS/CLAUDE, regenerated SVGs | patch |

## 4. The self-update cliff — the one real risk

An already-installed binary asks `api.github.com/repos/oyvij/crime-editor/releases/latest` for an
asset named `crime-<os>-<arch>`. After the rename the API URL still resolves — GitHub redirects a
renamed repository — but that asset name no longer exists, so every existing install stops updating
with no error the user will ever see.

Mitigation, in C3: keep uploading the `crime-*` asset names alongside `los-*` for a few releases —
one extra `cp` per matrix leg and a duplicate upload. An old binary then pulls a binary that is
already Los and self-heals on its next update. Drop the old spelling once nothing is left behind.

## 5. Verification

- `cargo test` — the full suite, 57 feature files plus the unit tests. The key sweep is slow;
  expect minutes.
- `cargo clippy -- -D warnings` and `cargo fmt --check`
- `grep -rIi crime` returns **only** the documented leftovers: the near-miss fixture and the ADRs'
  historical prose.
- `./install.sh --list`, and a dry run of the `~/.crime` → `~/.los` move
- `cargo run -- .` by hand, plus the `script` / `examples/replay` check, since no scenario covers
  the edge
- `asset_names_match_the_release_workflow` still passes after C3
