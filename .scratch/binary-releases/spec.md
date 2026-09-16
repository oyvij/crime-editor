# Binary releases: CRIME as a package that updates itself

Status: ready-for-agent

To be specified as **F39** in `docs/example-map.md` (entry-points row included). Scenarios extend
`features/self_update.feature`, whose Background already drives the seam this feature lives on. The
one decision hard enough to reverse is argued in
`docs/adr/0017-a-binary-install-updates-itself-from-a-release.md`, which amends the "no network"
sentence of the self-update feature and sits beside ADR 0011's "CRIME never downloads a server".

## Problem Statement

CRIME can only be installed by building it: clone, `cargo build --release`, symlink. That needs a
Rust toolchain, a C compiler and a few minutes of compiling before anyone has seen the editor. For
someone who only wants to use it, that is the wrong first minute.

Once installed that way, `:update` rebuilds from the checkout, which is exactly right for a
developer and useless for anyone who did not clone: a copied binary has no checkout above it, so
CRIME offers nothing and the Update marker never appears. The author does not want a second class
of install where updating means leaving the editor.

## Solution

CRIME becomes a **package**: CI builds a binary per platform whenever a minor or major Version
lands on `main` and publishes them as a GitHub Release of this repository, beside the source.
`install.sh` fetches the binary for the machine by default and builds from source only when asked.

A CRIME that was installed as a binary — no checkout above it — asks this repository once at
startup for the latest release. If that release's Version is strictly newer than the Running
version, the existing Update marker appears, fed from a release rather than a manifest. `:update`
and palette `u`, which today run a build in the terminal, instead **download the binary for this
platform, verify its checksum, put it in place of the running one and relaunch CRIME** — with the
same refusal quitting already makes when a buffer is unsaved. A checkout install keeps today's
behaviour exactly.

The dependency table `install.sh` reads today out of the source moves into the binary's own mouth:
`crime --deps` prints the programs it can be configured to run and how to install each, so the
script can serve a machine with no source on it, and the binary you installed is the one that says
what it needs.

## Vocabulary

To be written into `CONTEXT.md` under "Staying up to date":

- **Install kind** — how this CRIME got onto the machine: a **checkout install** (the binary sits
  inside its own checkout, which is `target/release/crime` under a manifest naming crime) or a
  **binary install** (anything else). Decided once at startup from where the binary is.
- **Release** — a published Version with one binary per platform and a checksum list, a GitHub
  Release of this repository. A Release is what a binary install compares itself against; a
  checkout install never looks for one.
- **Asset** — the one file in a Release built for this platform, named `crime-<os>-<arch>`.
- **Relaunch** — CRIME replacing itself with the binary now on disk, keeping its arguments. Not a
  restart of the session: the shell and the AI pane end with the old process, as they do on quit.

## User Stories

1. As a new user, I want to install CRIME with one command and no toolchain, so that trying it costs a minute rather than an afternoon.
2. As a new user, I want that command to need nothing but curl, so that a machine with no Rust, no compiler and no git can still run it.
3. As a new user, I want the install to pick the binary for my OS and CPU, so that I never read a table of targets.
4. As a new user, I want the same install prompts for language servers, formatters and the voice that a source install gets, so that a feature I turned on works on first launch.
5. As a user, I want CRIME to tell me when a newer release exists, so that I do not discover a fix by reading the changelog.
6. As a user, I want that check to cost one request at startup and nothing periodic, so that CRIME never wakes the network while I work.
7. As a user, I want a failed check to show nothing, so that being offline does not nag me.
8. As a user, I want `:update` to download and install the new release from inside CRIME, so that updating never means leaving the editor.
9. As a user, I want the palette's `u` to do the same thing `:update` does, so that there is one gesture whichever way I reach it.
10. As a user, I want the downloaded binary verified against its published checksum before it replaces mine, so that a truncated or tampered download is refused rather than run.
11. As a user, I want a refused download to say why, so that a network error, a missing asset and a bad checksum are three different messages.
12. As a user, I want CRIME to relaunch on the new binary once it is in place, so that I am on the new version without finding my terminal again.
13. As a user with an unsaved buffer, I want the relaunch refused with the same notice quitting gives, so that an update can never discard my work.
14. As that user, I want the next `:update` after saving to relaunch without downloading again, so that the refusal cost me a save and nothing more.
15. As a user, I want to see which release I am about to install before it is fetched, so that the marker names the Version and not just "an update".
16. As a user, I want the binary replaced atomically, so that a crash mid-download leaves the old CRIME working.
17. As a user, I want the replaced binary to keep its executable bit and its path, so that the symlink on my PATH still points at a working program.
18. As a developer, I want a checkout install to behave exactly as today — manifest comparison, `cargo build --release` in the terminal, no network — so that this feature costs the source workflow nothing.
19. As a developer, I want the install kind decided from where the binary is and nothing else, so that copying a built binary out of a checkout makes it a binary install with no configuration.
20. As the maintainer, I want releases cut only when the minor or major Version changes, so that a release means a user-visible change and the marker is worth reading.
21. As the maintainer, I want CI to refuse to release a Version that already has a tag, so that a force-push cannot publish two different binaries under one number.
22. As the maintainer, I want the release published by the workflow's own token, so that there is no personal access token to mint, store or rotate.
23. As the maintainer, I want `install.sh` fetched from `main` and the binaries from the Release, so that both come from this repository and neither is copied anywhere.
24. As the maintainer, I want the release notes generated from the commits since the last release, so that a release costs no writing.
25. As the maintainer, I want the repository tagged with the same Version, so that a binary can be traced to the commit it was built from.
26. As the maintainer, I want the Linux binary built on the oldest supported runner, so that it runs on a machine whose libc is a few years old.
27. As the maintainer, I want a platform whose runner is unavailable to be skipped rather than block the release, so that a missing arm64 Linux build does not stop the macOS one.
28. As the maintainer, I want the checksum list published beside the binaries, so that the in-editor update and a hand install verify the same file.
29. As the maintainer, I want the asset naming shared by CI and CRIME in one place each and held equal by a test, so that a renamed asset cannot make every binary install report "no asset for this platform".
30. As a script author, I want `crime --deps` to print one line per program with its kind, language, command and install command for this OS, so that the script parses lines and never TOML.
31. As a script author, I want `--deps` to work with no folder and no terminal, so that it can run before any workspace exists.
32. As a user of `install.sh`, I want it to offer the binary path by default and the source path when I ask, so that the fast route is the default and the developer route still exists.
33. As a user of `install.sh`, I want re-running it on a binary install to replace the binary with the latest release, so that the script and `:update` are two spellings of one thing.
34. As a user, I want the check to run unauthenticated, so that no token is ever needed on a machine that only uses CRIME.
35. As a reader of AGENTS.md, I want the rule that a program CRIME shells out to is installable by the script to stay true, so that `--deps` becomes the way the script learns it rather than a second table.

## Implementation Decisions

**Install kind is decided at startup, on the seam the self-update scenarios already use.** Startup
already receives where the binary is and the checkout's manifest, and answers with a known checkout
and whether an Update exists. A start with no known checkout is a binary install and returns one
new effect asking the edge for the latest Release of this repository. A start with a known
checkout returns no such effect: the checkout path stays offline exactly as ADR 0003 argues. Startup also learns the machine's
CPU architecture beside the OS it already knows, because the asset name needs both.

**The edge fetches, the core decides.** The effect names the URL of this repository's latest
release. The edge performs the request and hands the response body back as an event, unparsed. The
core parses it — it already carries a JSON dependency — and decides three things: the Version it
names, whether that Version is strictly newer than the Running version (the same `semver` ordering
the manifest comparison uses), and which asset is this platform's, by the name
`crime-<os>-<arch>`. A body that will not parse, a Version that is not newer, or a Release with no
asset for this platform each leave the marker down and raise no notice. A failed request is the
same event with no body and is logged by the edge, never shown.

**The marker is the marker.** `update_available` is set from a Release exactly as it is from a
manifest today; the renderer changes nothing. The State additionally remembers the Release it saw
— Version, asset URL, checksum URL — because `:update` needs them.

**`:update` in a binary install replaces the binary.** The existing Rebuild event branches on the
install kind: a known checkout returns today's terminal command; a remembered Release returns a
new effect naming the asset URL and the checksum URL; neither returns today's `no-checkout` notice,
whose text is widened to say there is nothing to update from. The edge downloads the asset to a
temporary file beside the running binary, downloads the checksum list, verifies the asset's SHA-256
against the line naming it, sets the executable bit, and renames the temporary file over the
running binary's resolved path — the file the symlink names, never the symlink. Unix keeps the old
inode open for the running process, so this is safe while CRIME is up. Failure at any step comes
back as an event carrying which step, and the core raises one notice per kind: download failed,
no asset, checksum mismatch, could not replace. Success comes back as an event with no payload.

**Replaced means relaunch, held to the refusal quitting makes.** On the success event the core
runs the same path the existing Restart event runs — which *is* Quit, so an unsaved buffer raises
`unsaved-changes` and nothing exits — but the terminal effect is a new Relaunch rather than Exit.
The edge executes Relaunch by replacing its own process with the binary at its own path and the
same arguments. The core also remembers that the binary on disk is already replaced, so a second
`:update` after a refused relaunch relaunches without fetching. That memory dies with the process,
which is correct: the next process *is* the new binary.

**Two edge tools, chosen to add nothing to the binary.** The request and the download shell out to
`curl`, which every macOS and Linux machine has and which `install.sh` already requires. The checksum is
computed with the `sha2` crate — small, pure Rust, one call — rather than by shelling out to a tool
that is `shasum` on one platform and `sha256sum` on the other, which is the branch this repo
refuses. Named in `docs/stack.md`.

**Asset names are one constant on each side.** The core builds `crime-<os>-<arch>` from the two
strings startup already has (`macos`/`linux`, `aarch64`/`x86_64`). The workflow's matrix spells
the same four names. A unit test in the core enumerates the four and a comment in the workflow
points at it; there is no way to hold YAML and Rust equal by a compiler, so the test names the
workflow file and the workflow names the test.

**`crime --deps` is a library function printed by the edge.** The dependency table is the
`[lsp.*]`, `[formatter.*]` and `[speech]` rows of the shipped defaults, already parsed at every
start. A function returns them as records — kind, name, command, install command for the given
OS — and the binary's argument parser gains one flag that prints them one per line, tab-separated,
and exits before any terminal is touched. `install.sh` stops reading the source and asks the
binary instead, on both install paths: the binary it downloaded, or the one it just built.

**The release workflow publishes a Release of this repository.** On a push to `main` that
touches the manifest, a first job reads the Version before and after and decides whether the
minor or major changed; a manual dispatch forces a release of whatever `main` holds. The build job
is a matrix of four native runners — arm64 and Intel macOS, x86_64 and arm64 Linux — each running
an ordinary locked release build and uploading its binary under the asset name. The arm64 Linux
leg is allowed to fail, because its runner is not on every plan. A publish job downloads what was
built, writes `SHA256SUMS`, refuses if the tag already exists, tags `v<Version>`, and creates the
Release with the workflow's own token, attaching the assets and checksums with notes generated
from the commits since the previous tag. No secret, no variable, nothing created by hand.

**`install.sh` grows the binary path and keeps the source one.** With no `crime` on PATH and no
checkout under the script, it offers a binary install by default: fetch `SHA256SUMS` and the asset
for this platform from the latest Release, verify, install to
`~/.local/bin/crime` as a real file rather than a symlink, then run `crime --deps` for the
dependency prompts. Run from inside a checkout, or asked for source, it builds as today. On an
existing binary install, re-running replaces the binary with the latest release. The `--list` mode
reads `--deps` from whichever binary is installed.

## Testing Decisions

A good test here asserts on what startup or `update` returns and on nothing the edge does: the
effect requested, the state after the event, the notice raised, the command not run. The
self-update scenarios are the prior art and the same steps are reused: `Given there is no checkout
manifest`, `When CRIME starts in the project`, `Then an Update is available`, `And no command has
been executed`, `Then the user is told …`.

Scenarios to add to `features/self_update.feature`, each a single event:

- A binary install asks for the latest Release at startup; a checkout install does not.
- A Release strictly newer than the Running version makes an Update available; an equal or older
  one does not; a body that does not parse does not; a Release with no asset for this platform
  does not — and none of them raises a notice.
- `:update` on a binary install with a Release fetches that Release's asset and checksum and runs
  nothing in the terminal; with no Release it notifies and fetches nothing; on a checkout install it
  still runs the build command.
- The palette's `u` does the same as `:update` on a binary install.
- A failed fetch of each kind raises its own notice and CRIME keeps running on the old binary.
- A replaced binary relaunches; a replaced binary with an unsaved buffer is told `unsaved-changes`
  and does not relaunch; `:update` after that refusal relaunches without fetching.
- `--deps` output is pinned by a unit test beside the function, against the shipped defaults, on
  both OS names.
- The four asset names are pinned by a unit test that names the workflow file.

The workflow itself has no test; it is verified by a release. The edge's download, checksum and
rename are edge code, verified by running them, as every effect is.

## Out of Scope

- Cutting releases from a tag pushed by hand. The Version diff on `main` is the trigger; manual
  dispatch exists for a re-run.
- Code signing or notarization on macOS. A binary fetched by curl carries no quarantine flag.
- A Homebrew tap, a `.deb`, Windows. A Release with named assets leaves room for the first two.
- A periodic check while CRIME runs. One request at startup, as the ADR argues.
- Rolling back to a previous Release from inside CRIME. Re-run `install.sh` with a version if that
  is ever wanted.
- Coverage or CI badges. That is the `ci` workflow, which does not exist yet and is a separate spec.
- Delta or resumable downloads. A CRIME binary is small enough to fetch whole.

## Further Notes

The feature's ordering matters: the workflow first, because nothing else has anything to fetch
without it; then the startup and update seams, which are red scenarios until then; then `--deps`
and `install.sh`, which are what make the repository a package rather than a download page.

The first release will have no previous tag, so its notes are the whole log truncated; that is
acceptable once.
