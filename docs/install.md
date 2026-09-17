# Installing CRIME

CRIME is installed by pointing a symlink on your PATH at the release binary inside your checkout.
Nothing is added to your shell configuration, and there is no second step to remember afterwards:
once the symlink exists, the ordinary release build *is* the install.

## The scripted way

`install.sh` at the repo root does the one-time step below and everything around it, interactively:

```sh
curl -fsSL https://raw.githubusercontent.com/oyvij/CRIME/main/install.sh | bash
```

It reads its prompts from `/dev/tty`, which is what lets it ask questions while piped from `curl`.
The order is: a feature menu (AI pane, language servers, formatters, reading aloud — all on by
default, toggle by number), then the required toolchain (git, a C compiler, `cargo` via rustup —
declining any of these aborts), then clone or `git pull --ff-only`, `cargo build --release` and the
symlink, then one `y/N` per missing optional program. A program whose install command needs a
package manager that is not there (`npm`, `go`, `pipx`, `uv`, `brew`) gets a second prompt for that
first. Nothing is installed without printing the command it is about to run.

If `crime` is already on PATH and resolves into a checkout's `target/release`, that checkout is the
one updated. Otherwise the script asks where the checkout should live: run from inside a clone
(`./install.sh` after `git clone`), it defaults to that clone and never clones again; run from
`curl`, it defaults to `~/.crime/src`. A clone the server refuses is offered again over SSH, for a
fork that is private. `CRIME_REPO` overrides the clone URL for a fork.

The language servers, formatters and the voice are not listed in the script. They are read out of
`DEFAULTS` in `src/startup.rs` — the same `[lsp.*]`, `[formatter.*]` and `[speech]` rows CRIME
offers from inside the editor — so a row added there with an `install.<os>` key is installable
here with no change to the script (ADR 0012). Only what the edge runs *without* configuration is
spelled out in `install.sh` itself: the build toolchain, git, the default AI CLI (`claude`), the
speech player and the URL opener. **When a feature adds a program CRIME shells out to, it goes in
one of those two places, and `./install.sh --list` shows whether it is picked up.**

`crime --deps` is the same table asked of a binary instead of a source tree: one line per row,
tab-separated as `kind`, `name`, `command`, `install`, the install command being this OS's
`install.<os>` or blank. The kinds are `lsp`, `formatter`, `speech`, and `player` for the speech
row's `player.<os>`. It needs no folder and no terminal, and exits before touching either — which
is what lets a machine with no checkout learn what to offer.

Windows is not covered: `DEFAULTS` carries `install.windows` rows for a hand install.

## The one-time step, by hand

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

## Thereafter, the build is the install

`cargo build --release` writes to the same file the symlink already names, so a build updates the
installed command. There is no copy step, no reinstall, and nothing that can be forgotten:

```sh
git pull && cargo build --release
```

A build that fails to compile writes no binary. The previous one stays exactly where it was, so a
broken checkout never leaves you without a working editor — you keep the last version that
compiled until the next one does.

## What this arrangement costs

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
