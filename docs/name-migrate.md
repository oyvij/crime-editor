# Renaming CRIME → Varde (`varde-editor`)

CRIME becomes **Varde**. A *varde* is a stone cairn someone stacked to mark the way for whoever
comes next — which is what this editor does with a review, a story and a test gate. The binary, the
crate, the on-disk state directories, the release assets and the name the terminal answers to all
move with it.

This is a plan, not a record of work done. Nothing here is implemented. Every decision below is
**settled** — §1 records what was chosen and why, so a later reader does not reopen it.

Survey at the time of writing: **3,454 case-insensitive `crime` hits across 123 files** — 55
feature files, 19 ADRs, 10 guide pages, 1,147 `CrimeWorld` occurrences and 388 `crime::` paths.
`grep -rIi varde` returns **0 hits**, which is what keeps §5's verification meaningful.

## 1. Settled decisions

| # | Decision | Chosen | Why |
|---|---|---|---|
| D1 | The name | **Varde** — binary, crate and directories `varde`, repo `oyvij/varde-editor` | Rejected *Lumo* (Proton's AI assistant is live and marketed) and *Los* (reads as "loss", unpronounceable as a product). A Norwegian word is rare in English technical prose, so the rename stays verifiable by grep forever. |
| D2 | The backronym | **Dropped.** `description` and clap's `about` become `Varde — a terminal IDE that reviews, tests and ships your work` | `--help` is not the place for an etymology. The cairn line lives in the README. |
| D3 | Capitalisation | `Varde` in prose and identifiers · `varde` for binary, crate, directories, assets · `VARDE` **only** in the XTVERSION reply | CRIME was all-caps because it was an acronym. Varde is a word. |
| D4 | A project's `.crime/` | **Untouched. Varde never reads it, never moves it, never mentions it in code.** A folder simply gets a fresh `.varde/`. | Considered and rejected twice: an automatic rename writes into the user's repository on every start (breaking `lib.rs:1109` and, for a Bare workspace, ADR 0016), and a read-fallback is a branch in `src/` that would never be deleted. One line in the release notes says the old folder is the user's to remove. |
| D5 | `~/.crime` | **`install.sh` copies `config.toml` across and removes the old install.** No migration code in `src/` — no module, no new `Effect`, no scenarios. | The ladder's first step: this is a configuration problem, and the installer is where a configuration problem is cheapest. |
| D6 | Release-asset compatibility | **Hard cut.** No dual-asset transition release, no farewell CRIME release. | The install base is one person. §4 of the previous plan bought a permanent doubling of `asset_names_match_the_release_workflow` to protect nobody. Existing installs stop self-updating silently; re-running `install.sh` is the fix. |
| D7 | The XTVERSION reply | **`VARDE(x.y.z)`** | ADR 0004 requires naming itself honestly; uppercase matches the XTVERSION convention. Two unit tests pin it. |
| D8 | The version | **0.162.0** (CRIME ended at 0.161.1) | 0.x already declares nothing is stable, so a breaking change needs no major. 1.0.0 promises config, state layout and keybinding stability — do not spend it on a rename. |
| D9 | The local checkout | `~/crime` → `~/varde`, with `~/.claude/projects/-home-oyvindj-crime` moved alongside it | The agent memory directory is keyed off the checkout path and would otherwise be orphaned. So are this workspace's sidecars, which are keyed by absolute project path (`~/.crime/paths/%home%me%projects%crime-91`) — re-key that one by hand; no migration can guess which renames happened. |

## 2. What has to change, by category

### A. Crate and binary identity

`Cargo.toml` (`name`, `version`, `description`, and `repository` — which still says the stale
`oyvij/CRIME` while `startup.rs:880` says `oyvij/crime-editor`; both converge on
`oyvij/varde-editor`), clap's `#[command(name, about)]` in `src/main.rs`, the 388 `crime::` paths
across `src/`, `tests/` and `examples/`, and `target/release/crime`.

### B. On-disk state

`CRIME_DIR` in `src/lib.rs`, `crime_dir()`, the `crime_home` field and the `GLOBAL_LABEL` /
`PROJECT_LABEL` constants in `src/startup.rs`, the `.crime-update-` temporary prefix in
`src/main.rs:3016`, the seeded and template config text — including the piper rows'
`mkdir -p ~/.crime/voices` and `configures.voice` in `startup.rs:622-624` — and the user-facing
error strings naming `.crime/config.toml`, `.crime/lsp-<language>.log` or `~/.crime/config.toml`.

### C. Distribution

- `.github/workflows/release.yml`: the four `asset:` spellings (lines 77-84), the comment at line
  10, `cp target/release/crime` (91), `sha256sum crime-*` (120) and the release title
  `CRIME $VERSION` (128).
- `src/startup.rs:906`'s `format!("crime-{os}-{arch}")`, pinned against that workflow by the unit
  test `asset_names_match_the_release_workflow` (2579). Change both or every install reports no
  asset.
- `src/startup.rs:880`'s `RELEASE_URL` → `oyvij/varde-editor`.
- `install.sh` — see §4.
- `features/self_update.feature` encodes the manifest rule that identifies a checkout as Varde's
  own; it renames with everything else.

### D. Terminal identity

`src/queries.rs`'s XTVERSION reply and its two tests. `CHILD_ENV` needs no change: it names
emulators, not the product.

### E. Test scaffolding

`CrimeWorld` (1,147 occurrences) → `VardeWorld`; "CRIME starts in the project" and its siblings
across 55 `.feature` files; the fixture paths `/home/me/projects/crime` and `/home/me/src/crime`.

### F. Prose

`AGENTS.md`, `CLAUDE.md`, `CONTEXT.md`, `README.md`, the 19 ADRs — including the *filename*
`docs/adr/0010-crime-owns-the-test-gate.md` — the 10 files under `docs/guide/`, `docs/install.md`,
`docs/example-map.md`, `docs/stack.md` and `docs/agents/issue-tracker.md`.

### G. Assets

The **five** `docs/images/*.svg` (`edit`, `palette`, `preview`, `review`, `risk`) each render the
workspace title `crime`. Regenerate with `examples/shot.rs` after the checkout is renamed.

### H. Outside the repository

Rename the GitHub repository, update `git remote`, repoint `docs/agents/issue-tracker.md` and the
label docs, and update the agent memory note that names `oyvij/crime-editor`. Open issues are
historical and stay as they are.

## 3. Execution order

Land the `git-blame` branch first, then rename the GitHub repository, then branch off `main`.
Renaming the repo *before* the sweep means both spellings resolve while the work is in flight —
GitHub redirects the old name — so nothing is broken between commits.

A blind `sed s/crime/varde/g` is wrong in known places, so the sweep runs most-specific-first and
ends with a manual pass:

1. `crime-editor` → `varde-editor`; `oyvij/CRIME` → `oyvij/varde-editor`
2. the asset spellings `crime-{linux,macos}-*` → `varde-*`
3. `~/.crime` and `.crime` → `.varde` — **excluding `.crimean-war`**
4. `CrimeWorld` → `VardeWorld`
5. `crime::` → `varde::`; `crime_home` / `crime_dir` / `CRIME_DIR` → `varde_home` / `varde_dir` /
   `VARDE_DIR`
6. `CRIME(` → `VARDE(`
7. bare `CRIME` → `Varde`; bare `crime` → `varde`
8. by hand: the backronym lines, the README title, `Cargo.toml`'s description, the `/home/me/…`
   fixtures, the SVGs, and ADR 0010's filename (`git mv`)

**Known traps.**

- `src/story.rs:2368` asserts `.crimean-war/notes.md` is *not* one of the editor's own directories —
  a deliberate prefix near-miss that a blind replace turns into nonsense. It becomes
  **`.varden/notes.md`**, which near-misses `.varde` the same way.
- ADR 0010 needs a `git mv` to `0010-varde-owns-the-test-gate.md` plus its six inbound links:
  `AGENTS.md`, `docs/example-map.md` (×2), `docs/adr/0009`, `docs/adr/0012`, `docs/guide/risk.md`.

**Commits.** AGENTS.md requires the version bump in the same commit as the change it describes.

| | Contents | Bump |
|---|---|---|
| C1 | `docs/adr/0020-the-editor-is-called-varde.md` and the `CONTEXT.md` vocabulary entry | 0.161.2 |
| C2 | The whole mechanical rename — `src/`, `tests/`, `features/`, `examples/`, `install.sh`, `release.yml`, prose, SVGs. Suite green, no behaviour change. This plan file is deleted here. | 0.162.0 |

Two commits, not four: a half-renamed repo does not compile, so the sweep is atomic by nature. C1 is
separate only so that C2's diff is pure rename and reviewable as such.

**The ADR must record three things** beyond the name: that CRIME's release lineage ends at 0.161.1
and nothing bridges it; that migration is `install.sh`'s job and `src/` carries no migration code by
design; and that a project's `.crime/` is deliberately untouched. The third is the one that stops a
later agent from helpfully adding a fallback.

## 4. The installer's migration

One step, run for **both** install kinds — a source install never downloads an asset, so this cannot
live inside the binary branch:

1. `mkdir -p ~/.varde` and copy `~/.crime/config.toml` → `~/.varde/config.toml`, rewriting the
   string `~/.crime` → `~/.varde` inside it. Without the rewrite `configures.voice` and the piper
   rows point at a directory that no longer exists, and the voice fails silently.
2. If `~/.crime/src` exists, **move** it to `~/.varde/src` and `git remote set-url` to
   `varde-editor`. Never delete it — it is a git repo that may hold unpushed commits, and
   `install.sh` has no undo.
3. Install `varde` (download the asset, or build the checkout and symlink), then remove the old
   `crime` binary or symlink and `~/.crime/` — **only after** the copy above succeeded.

`locate()` also changes: both checkout-detection greps at `install.sh:239` and `install.sh:253` must
accept `^name = "\(crime\|varde\)"`, or an existing checkout stops being recognised as the editor's
own and the script offers a second clone beside it. `~/.crime/src` remains the *old* default; the
new one is `~/.varde/src`.

The `crime --deps` call, `$HOME/.local/bin/crime`, the `CRIME_REPO` / `CRIME_BIN` variables, the
`crime-$OS-$ARCH` asset spelling, the raw.githubusercontent URL in the header comment and every
`say`/`echo` naming CRIME move with the rest.

## 5. Verification

- `cargo test` — 55 feature files plus the unit tests. The key sweep is slow; expect minutes.
- `cargo clippy -- -D warnings` and `cargo fmt --check`
- `grep -rIi crime` returns **only** `.varden`'s near-miss context and the new ADR, which names the
  old name on purpose. Nothing else. This check is only trustworthy because `varde` had 0 prior hits.
- `asset_names_match_the_release_workflow` passes against the edited `release.yml`
- `./install.sh --list`, plus a dry run of the `~/.crime` → `~/.varde` copy and removal
- `cargo run -- .` by hand, plus the `script` / `examples/replay` check, since no scenario covers
  the edge
- The five SVGs regenerated and showing the title `varde`
