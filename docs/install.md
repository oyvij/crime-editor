# Installing CRIME

CRIME installs two ways. The **binary** is the default: a prebuilt `crime` for this platform,
downloaded from the latest GitHub Release, verified and put at `~/.local/bin/crime`. It needs no
toolchain and updates itself from inside the editor (`:update`, ADR 0017). **From source** is a
checkout, a release build and a symlink on your PATH pointing at it — for anyone working on CRIME,
where the ordinary release build *is* the install. Nothing is added to your shell configuration
either way.

## The scripted way

`install.sh` at the repo root does either, and sets up what CRIME needs around it, interactively:

```sh
curl -fsSL https://raw.githubusercontent.com/oyvij/crime-editor/main/install.sh | bash
```

It reads its prompts from `/dev/tty`, which is what lets it ask questions while piped from `curl`.
With no `crime` on PATH it asks **binary or source**, binary by default. Then it installs CRIME,
writes the global config, asks about package managers, and offers the default AI CLI, the speech
player and the URL opener. Nothing is installed without printing the command it is about to run.

**The binary.** The script fetches `SHA256SUMS` and `crime-<os>-<arch>` from the latest Release,
checks the asset's SHA-256 against its line, and only then writes it — as a real file, not a
symlink — to `~/.local/bin/crime`. A missing asset, a failed download or a checksum mismatch aborts
with a message and installs nothing. Only `curl` is required.

**From source.** The script requires git, a C compiler and `cargo` via rustup (declining any of
these aborts), then clones or `git pull --ff-only`, runs `cargo build --release` and links the
symlink. Run from inside a clone (`./install.sh` after `git clone`), it never asks binary or source:
it builds that clone and never clones again. Answered "source" from `curl`, it asks where the
checkout should live, defaulting to `~/.crime/src`. A clone the server refuses is offered again over
SSH, for a fork that is private.

**Re-run**, it updates whichever kind it finds behind `crime`: a symlink into a checkout's
`target/release` is pulled and rebuilt; anything else is a binary install and is replaced in place
with the latest Release. `CRIME_REPO` overrides the repository for a fork — the clone URL and the
Release the binary comes from are both derived from it.

**The config.** Every run leaves a `~/.crime/config.toml`: when none is there, the script writes
what `crime --default-config` prints — the template CRIME itself seeds the file with on a start that
finds none, every setting commented out and every program row live (ADR 0018). An existing file is
never replaced, and a binary too old to print the template is reported rather than leaving a partial
file.

**Language servers, formatters and the voice are not installed by the script.** Each one is taken
from Tools inside CRIME, one key per row, which writes the row, runs its install in the shell pane
and fills in what the install configures (ADR 0018). What the script does for them is make sure
their install commands can run: it asks the `crime` it just installed, on either path, with
`crime --deps` — the rows `~/.crime/config.toml` names, or the template's when that file does not
exist yet — and takes the package manager each `install.<os>` starts with (the first word, or the
one after `sudo`, the same rule Tools uses to read a row `needs-installer`). Each one this machine
lacks is one `y/N`, naming the rows that need it:

```
npm is used to install javascript, python, typescript, vue, and the prettier formatters. Install npm with: sudo apt install -y nodejs npm ? [y/N]
```

A row that needs a new manager is asked about with no change to the script. How to install each
manager is the one table the script still owns (`installer_install`), since a package manager cannot
install itself; a manager missing from it is named as one the script cannot install on this OS.
On macOS a manager that comes from `brew` asks about `brew` first.

Only what the edge runs *without* configuration is spelled out in `install.sh` itself: the build
toolchain, git, the default AI CLI (`claude`), the speech player's package (`alsa-utils`, on Linux,
since Tools offers no install for it) and the URL opener. **When a feature adds a program
CRIME shells out to, it is either a row with an `install.<os>` key or a line in the script, and
`./install.sh --list` shows whether it is picked up.** `--list` asks whichever `crime` is installed,
so it needs no source.

`crime --deps` prints one line per row, tab-separated as `kind`, `name`, `command`, `install`, the
install command being this OS's `install.<os>` or blank. The kinds are `lsp`, `formatter`, `speech`,
and `player` for the speech row's `player.<os>`; a speech command or player no file names is not
listed. A config file CRIME would refuse to start on stops `--deps` with the same file and line. It
needs no folder and no terminal, and exits before touching either — which is what lets a machine
with no checkout learn what to offer.

Windows is not covered: `PROGRAMS` carries `install.windows` rows for a hand install.

## From source, by hand

From your checkout, build once and link the result into a directory that is already on your PATH:

```sh
cargo build --release
ln -sfn "$(pwd)/target/release/crime" ~/.local/bin/crime
```

`$(pwd)` is what makes this work from wherever you cloned into — the symlink records your own
absolute path, so there is nothing to configure and nothing to edit for a different checkout
location. `~/.local/bin` is on PATH on the target machine; if yours is somewhere else, substitute
it. Confirm with `command -v crime`, which should print the symlink's path.

That is the whole installation. From any folder in any terminal:

```sh
crime .            # open the current folder as the workspace
crime ~/some/repo  # open a folder somewhere else
```

## On a source install, the build is the install

`cargo build --release` writes to the same file the symlink already names, so a build updates the
installed command. There is no copy step, no reinstall, and nothing that can be forgotten:

```sh
git pull && cargo build --release
```

A build that fails to compile writes no binary. The previous one stays exactly where it was, so a
broken checkout never leaves you without a working editor — you keep the last version that
compiled until the next one does.

## What the symlink costs

Two consequences follow from the symlink, and both are deliberate:

- **The running editor is whatever the checkout last compiled successfully.** A commit that builds
  but misbehaves is an editor that misbehaves. There is no second copy of the binary held back as a
  known-good version.
- **Cleaning the build directory uninstalls CRIME.** `cargo clean` deletes `target/`, which is the
  file the symlink points at, and `crime` stops working until you build again.

Both are recovered by a build, which is why neither is worth a second copy of the binary to avoid.

## Reclaiming build space

`cargo clean` is the wrong tool here, and the section above says why: it deletes the file the symlink
names. The two halves of `target/` have opposite risks and should never be swept together.

- **`target/release` is the installation.** Deleting it uninstalls CRIME until the next build. The
  update path — Ctrl+Space, which runs `cargo build --release` in the checkout — overwrites the
  binary in place and never needs a clean first. Nothing accumulates here: one binary, overwritten.
- **`target/debug` is disposable.** It holds the test-suite artifacts and nothing on your PATH points
  into it. `rm -rf target/debug` costs one recompile of the suite and cannot break the installed
  editor.

The growth to watch for is in `target/debug/deps`, and it is a macOS-specific trap. Mach-O keeps
DWARF in the loose `.rcgu.o` object files that a compilation emits rather than in the binary, so the
files have to survive the link — and Cargo never collects them. Their names carry a hash of the code
they came from, so every recompile writes a *new* set instead of replacing the last one, and the
edit-test loop this repo runs on is an unbounded producer. Left alone for a year it reached 862,000
object files and 157 GB, against 38 MB of actual repository.

`debug = 0` in `[profile.dev]` is what stops it: with no debug info there are no object files to
retain. The cost is line numbers in panic backtraces — function names survive, which is enough to
find a failing scenario. If a debugging session ever needs more, ask for it on the command line
rather than in the manifest, and expect a full rebuild either way:

```sh
RUSTFLAGS="-Cdebuginfo=1 -Csplit-debuginfo=packed" cargo test
```

`packed` is the important half of that: it runs `dsymutil` to gather the debug info into a single
`.dSYM` bundle that gets replaced on each build, so a session spent under a debugger does not leave
another pile behind.

Object files were the bulk of it but not all: the artifact *names* carry a hash of the compilation
inputs, and the package version is one of them. The version bump this repo requires on every commit
therefore mints a fresh set of test binaries each time and abandons the last — 191 commits had left
489 of them, at 143 MB for the cucumber binary alone. Nothing collects those either.

That one is not worth a tool. A full rebuild of the whole suite from an empty `target/debug` measures
59 seconds, so the maintenance is to delete the directory whenever it bothers you and let the next
`cargo test` repopulate it:

```sh
rm -rf target/debug     # never `cargo clean`, which takes the installed binary with it
```
