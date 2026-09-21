#!/usr/bin/env bash
# Install or update Varde, its global config, and the package managers its rows use.
#
#   curl -fsSL https://raw.githubusercontent.com/oyvij/varde-editor/main/install.sh | bash
#
# Interactive: every prompt reads /dev/tty, so it works piped from curl. A fresh
# machine gets the binary from the latest Release by default, verified against
# its SHA256SUMS; run from inside a checkout, or answered "source", it clones and
# builds instead. Re-run, it updates whichever kind it finds behind `varde`.
#
# Language servers, formatters and the voice are not installed here: each is
# taken from Tools inside Varde (ADR 0018). What this script offers is the package
# managers their install commands start with, asked of the installed binary with
# `varde --deps`, so a row that needs a new manager is asked about with no change
# to this file. Only what the edge runs *without* configuration is spelled out
# below: the build toolchain, git, the default AI CLI, the speech player and the
# URL opener.
#
# Windows is not covered: this is a bash script, and `PROGRAMS` carries its own
# `install.windows` rows for a hand install.

set -euo pipefail

# Clone URL and Release source both: a fork overrides one variable.
REPO="${VARDE_REPO:-https://github.com/oyvij/varde-editor.git}"
BIN="${VARDE_BIN:-$HOME/.local/bin/varde}"

case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux) OS=linux ;;
  *) echo "install.sh runs on macOS and Linux only" >&2; exit 1 ;;
esac

# Spelled as the release workflow's asset names, `varde-<os>-<arch>`.
case "$(uname -m)" in
  x86_64 | amd64) ARCH=x86_64 ;;
  arm64 | aarch64) ARCH=aarch64 ;;
  *) ARCH=$(uname -m) ;;
esac

say() { printf '\033[1m%s\033[0m\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

ask() { # ask "question" -> 0 on yes, default no
  local answer
  read -r -p "$1 [y/N] " answer </dev/tty || answer=n
  [[ "$answer" =~ ^[Yy] ]]
}

run() { # run "shell command" — printed first so what happens is what was read
  echo "  \$ $1"
  bash -c "$1"
}

# ---- the required toolchain --------------------------------------------------

require() { # require <command> "<what>" "<install command>" — abort on no
  have "$1" && return
  say "$2 is required and not installed."
  ask "Install it now with: $3 ?" || { echo "Cannot continue without $1." >&2; exit 1; }
  run "$3"
  # rustup and brew land outside the current PATH; pick them up for this run.
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  [ -x /opt/homebrew/bin/brew ] && eval "$(/opt/homebrew/bin/brew shellenv)"
  export PATH="$HOME/.local/bin:$PATH"
  have "$1" || { echo "$1 is still not on PATH; open a new shell and rerun." >&2; exit 1; }
}

toolchain() {
  require git "git" "$( [ $OS = macos ] && echo 'xcode-select --install' || echo 'sudo apt install -y git')"
  require curl "curl" "sudo apt install -y curl"
  require cc "a C compiler (the linker cargo needs)" \
    "$( [ $OS = macos ] && echo 'xcode-select --install' || echo 'sudo apt install -y build-essential pkg-config')"
  require cargo "the Rust toolchain" "$(installer_install rustup)"
}

# ---- the package managers the rows' install commands start with ------------

# The first word of an `install.<os>` command, or the one after `sudo` — the
# same rule Tools applies when it reads a row `needs-installer`.
installer_for() {
  local words
  read -r -a words <<<"$1"
  if [ "${words[0]:-}" = sudo ]; then echo "${words[1]:-}"; else echo "${words[0]:-}"; fi
}

installer_install() { # installer_install <tool> -> the command that installs it here, or ""
  case "$1:$OS" in
    brew:macos) echo '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"' ;;
    npm:macos) echo 'brew install node' ;;
    npm:linux) echo 'sudo apt install -y nodejs npm' ;;
    go:macos) echo 'brew install go' ;;
    go:linux) echo 'sudo apt install -y golang-go' ;;
    pipx:macos) echo 'brew install pipx' ;;
    pipx:linux) echo 'sudo apt install -y pipx' ;;
    uv:*) echo 'curl -LsSf https://astral.sh/uv/install.sh | sh' ;;
    rustup:*|cargo:*) echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" ;;
    *) echo "" ;;
  esac
}

ensure_installer() { # ensure_installer <tool> "<why>" -> 0 if it is present afterwards
  have "$1" && return 0
  local how
  how=$(installer_install "$1")
  [ -z "$how" ] && { echo "  $2, and this script cannot install $1 on $OS"; return 1; }
  if [ $OS = macos ] && [ "$1" != brew ] && [[ "$how" == brew\ * ]]; then
    ensure_installer brew "brew is used to install $1" || return 1
  fi
  ask "  $2. Install $1 with: $how ?" || { echo "  skipped $1"; return 1; }
  run "$how" || { echo "  $how failed; $1 stays missing"; return 1; }
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  [ -x /opt/homebrew/bin/brew ] && eval "$(/opt/homebrew/bin/brew shellenv)"
  export PATH="$HOME/.local/bin:$PATH"
  have "$1"
}

# `kind<TAB>name<TAB>command<TAB>install` for every row the config names. Asked
# once, up front: a failure inside `< <(...)` would not stop the script and would
# read as nothing needed.
ask_deps() {
  DEPS=$("$EXE" --deps) || { echo "$EXE --deps failed; cannot tell what Varde needs." >&2; exit 1; }
}

# "a", "a and b", "a, b, and c"
listing() {
  case $# in
    1) echo "$1" ;;
    2) echo "$1 and $2" ;;
    *) local all="" x; for x in "${@:1:$#-1}"; do all+="$x, "; done; echo "${all}and ${!#}" ;;
  esac
}

# One y/N per package manager the rows need and this machine lacks, naming the
# rows that need it: servers by language, formatters by command.
installers() {
  local tools="" tool kind name cmd inst
  while IFS=$'\t' read -r kind name cmd inst; do
    [ -n "$inst" ] || continue
    tool=$(installer_for "$inst")
    [[ " $tools " == *" $tool "* ]] || tools+=" $tool"
  done <<<"$DEPS"
  for tool in $tools; do
    if have "$tool"; then echo "  $tool: installed"; continue; fi
    local named=() formatters=() rows=0 item
    while IFS=$'\t' read -r kind name cmd inst; do
      [ -n "$inst" ] && [ "$(installer_for "$inst")" = "$tool" ] || continue
      case "$kind" in
        formatter) rows=$((rows + 1)); item=$cmd; [[ " ${formatters[*]:-} " == *" $item "* ]] || formatters+=("$item") ;;
        *) item=$name; [[ " ${named[*]:-} " == *" $item "* ]] || named+=("$item") ;;
      esac
    done <<<"$DEPS"
    case $rows in
      0) ;;
      1) named+=("the ${formatters[0]} formatter") ;;
      *) named+=("the $(listing "${formatters[@]}") formatters") ;;
    esac
    ensure_installer "$tool" "$tool is used to install $(listing "${named[@]}")" || continue
  done
  if [[ " $tools " == *" npm "* ]]; then npm_prefix; fi
}

# Debian's npm, which is the one `sudo apt install npm` gives, installs globally
# into /usr/local, so every `npm install -g` row fails with EACCES. npm's own
# answer is a prefix the user owns, and ~/.local/bin is already on PATH for varde.
npm_prefix() {
  have npm || return 0
  local prefix dir
  prefix=$(npm config get prefix)
  dir="$prefix/lib/node_modules"
  while [ ! -e "$dir" ]; do dir=$(dirname "$dir"); done
  [ -w "$dir" ] && return 0
  ask "  npm installs globally into $prefix, which needs root, so its rows in Tools would fail. Install into ~/.local instead with: npm config set prefix ~/.local ?" || return 0
  run "npm config set prefix ~/.local"
}

# ---- the global config ----------------------------------------------------------

# The template, as `varde --default-config` prints it — the same text Varde
# seeds on its own start: every setting commented out, every program row live.
# Never written over an existing file, and never left half-written.
seed_config() {
  local config="$HOME/.varde/config.toml"
  if [ -f "$config" ]; then
    echo "  $config: kept"
    return 0
  fi
  mkdir -p "$HOME/.varde"
  if "$EXE" --default-config > "$config.tmp"; then
    mv "$config.tmp" "$config"
    echo "  $config: created"
  else
    rm -f "$config.tmp"
    echo "  $EXE cannot print a default config; $config was not created"
  fi
}

# ---- the AI pane --------------------------------------------------------------

ai() {
  # `claude` is what `ai.command` defaults to (src/lib.rs); `:ai <command>` picks another.
  if have claude; then echo "  claude: installed"; return; fi
  ask "  claude (the default AI CLI) is missing. Install with: curl -fsSL https://claude.ai/install.sh | bash ?" || return 0
  run "curl -fsSL https://claude.ai/install.sh | bash"
}

# ---- the speech player (Linux only; `afplay` ships with macOS) ----------------

# Tools offers no install for it, so this is where it comes from.
player() {
  [ $OS = linux ] || return 0
  local p
  p=$(awk -F'\t' '$1 == "player" { print $3 }' <<<"$DEPS")
  if [ -z "$p" ] || have "$p"; then return 0; fi
  if ask "  $p (plays the speech) is missing. Install with: sudo apt install -y alsa-utils ?"; then
    run "sudo apt install -y alsa-utils"
  fi
}

# ---- the URL opener (Linux only; `open` ships with macOS) ---------------------

opener() {
  [ $OS = linux ] || return 0
  have xdg-open && return 0
  if ask "  xdg-open (opens URLs clicked in a pane) is missing. Install with: sudo apt install -y xdg-utils ?"; then
    run "sudo apt install -y xdg-utils"
  fi
}

# ---- CRIME, which is what Varde was called until 0.161.1 ---------------------

# An old install is not updated into a new one: the release assets were renamed
# and CRIME's last release, 0.161.1, has no Varde in it. What carries over is
# the global config, and a checkout is moved rather than left to rot beside a
# second one. Nothing is removed here — `retire_crime` does that, below the
# install, so a failed install never takes the old one with it. This runs for
# both install kinds: a source install never downloads an asset, so it cannot
# live in the binary path.
OLD="$HOME/.crime"

migrate_crime() {
  [ -d "$OLD" ] || return 0
  say "CRIME is the old name for Varde; carrying its config over"
  if [ -f "$OLD/config.toml" ] && [ ! -f "$HOME/.varde/config.toml" ]; then
    mkdir -p "$HOME/.varde"
    # Every mention, not just the ones spelled with a leading ~: a voice
    # configured as ~/.crime/voices/… points at nothing once it moves and fails
    # with no message, and the comments name .crime/config.toml as where a
    # project's own keys go, which is instructions to a directory Varde does not
    # read.
    sed 's|\.crime|.varde|g' "$OLD/config.toml" > "$HOME/.varde/config.toml"
    echo "  copied $OLD/config.toml -> $HOME/.varde/config.toml"
  fi
  if [ -d "$OLD/src" ] && [ ! -d "$HOME/.varde/src" ]; then
    mkdir -p "$HOME/.varde"
    mv "$OLD/src" "$HOME/.varde/src"
    echo "  moved $OLD/src -> $HOME/.varde/src"
    run "git -C '$HOME/.varde/src' remote set-url origin '$REPO'"
  fi
}

retire_crime() {
  local old
  old=$(command -v crime 2>/dev/null || true)
  if [ -n "$old" ] && [ "$old" != "$BIN" ] && ask "Remove the old CRIME at $old ?"; then
    rm -f "$old"
    echo "  removed $old"
  fi
  [ -d "$OLD" ] || return 0
  if [ -d "$OLD/src" ]; then
    # A checkout can hold commits that are nowhere else, and this script has no undo.
    echo "  $OLD kept: it still holds a checkout at $OLD/src"
  elif ask "Remove $OLD ? Its config is now at $HOME/.varde/config.toml"; then
    rm -rf "$OLD"
    echo "  removed $OLD"
  fi
}

# ---- which install, and where ---------------------------------------------------

locate() { # sets MODE (binary or source), and CHECKOUT for source; UPDATE=1 when varde is already installed
  UPDATE=0
  if have varde; then
    UPDATE=1
    local path link
    path=$(command -v varde)
    link=$(readlink "$path" || true)
    if [[ "$link" == */target/release/varde ]] && grep -qsE '^name = "(crime|varde)"' "${link%/target/release/varde}/Cargo.toml"; then
      CHECKOUT="${link%/target/release/varde}"; MODE=source; return
    fi
    if [ -L "$path" ]; then
      echo "$path is a symlink to $link, which is neither a Varde checkout nor a binary this script installed; remove it and rerun." >&2
      exit 1
    fi
    # Replaced where it is found, so an install outside ~/.local/bin is not duplicated there.
    BIN=$path; MODE=binary; return
  fi
  # Run from inside a checkout — `./install.sh` after a clone by hand — that
  # checkout is built, so a private fork never needs a second clone.
  local here default="$HOME/.varde/src" answer
  here=$(cd "$(dirname "${BASH_SOURCE[0]:-.}")" 2>/dev/null && pwd)
  if grep -qsE '^name = "(crime|varde)"' "$here/Cargo.toml"; then
    default=$here
  else
    read -r -p "Install the prebuilt binary, or build from source? [binary/source] " answer </dev/tty || answer=""
    [[ "$answer" =~ ^[Ss] ]] || { MODE=binary; return; }
  fi
  MODE=source
  local where
  read -r -p "Where should Varde's checkout live? [$default] " where </dev/tty || where=""
  CHECKOUT="${where:-$default}"
  CHECKOUT="${CHECKOUT/#\~/$HOME}"
}

# ---- the binary path ------------------------------------------------------------

sha256() { # the hash alone; sha256sum on Linux, shasum on macOS
  if have sha256sum; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

download() {
  local web="${REPO/#git@github.com:/https://github.com/}"
  local asset="varde-$OS-$ARCH" base="${web%.git}/releases/latest/download"
  TMP=$(mktemp -d)
  trap 'rm -rf "$TMP"' EXIT
  say "Downloading $asset from the latest Release"
  curl -fsSL "$base/SHA256SUMS" -o "$TMP/SHA256SUMS" \
    || { echo "Could not fetch $base/SHA256SUMS. Nothing was installed." >&2; exit 1; }
  local want
  want=$(awk -v a="$asset" '$2 == a || $2 == "*" a { print $1 }' "$TMP/SHA256SUMS")
  [ -n "$want" ] || { echo "The latest Release has no $asset. Rerun and answer \"source\" to build it. Nothing was installed." >&2; exit 1; }
  curl -fsSL "$base/$asset" -o "$TMP/$asset" \
    || { echo "Could not fetch $base/$asset. Nothing was installed." >&2; exit 1; }
  [ "$(sha256 "$TMP/$asset")" = "$want" ] \
    || { echo "$asset does not match its SHA256SUMS line; the download is corrupt or tampered with. Nothing was installed." >&2; exit 1; }
  echo "  checksum verified"
  mkdir -p "$(dirname "$BIN")"
  # A temporary file beside the target and a rename, so a varde that is running
  # keeps its file and the next start gets the new one whole.
  chmod +x "$TMP/$asset"
  cp "$TMP/$asset" "$BIN.new" && mv -f "$BIN.new" "$BIN" \
    || { echo "Could not write $BIN. Nothing was installed." >&2; exit 1; }
  echo "  installed $BIN"
  EXE=$BIN
}

# ---- the source path ------------------------------------------------------------

fetch_and_build() {
  if [ -d "$CHECKOUT/.git" ]; then
    if [ "$UPDATE" = 1 ]; then say "Updating $CHECKOUT"; else say "Reusing the checkout at $CHECKOUT"; fi
    run "git -C '$CHECKOUT' pull --ff-only"
  else
    say "Cloning into $CHECKOUT"
    if ! run "git clone '$REPO' '$CHECKOUT'"; then
      # A private repo refuses an anonymous HTTPS clone; SSH uses the key already on this machine.
      local ssh="${REPO/#https:\/\/github.com\//git@github.com:}"
      [ "$ssh" != "$REPO" ] && ask "Clone refused — the repo may be private. Retry over SSH with: git clone '$ssh' ?" || exit 1
      run "git clone '$ssh' '$CHECKOUT'"
    fi
  fi
  say "Building (the release build is the install)"
  run "cargo build --release --manifest-path '$CHECKOUT/Cargo.toml'"
  mkdir -p "$(dirname "$BIN")"
  ln -sfn "$CHECKOUT/target/release/varde" "$BIN"
  echo "  $BIN -> $CHECKOUT/target/release/varde"
  EXE="$CHECKOUT/target/release/varde"
}

# ---- --list: what would be checked, and its state, without touching anything --

list() {
  EXE=$(command -v varde 2>/dev/null || true)
  [ -n "$EXE" ] || { [ -x "$BIN" ] && EXE=$BIN; }
  [ -n "$EXE" ] || { echo "varde is not installed; --list asks the installed binary what it needs" >&2; exit 1; }
  ask_deps
  printf '%-10s %-12s %-28s %s\n' kind name command state
  while IFS=$'\t' read -r kind name cmd inst; do
    printf '%-10s %-12s %-28s %s\n' "$kind" "$name" "$cmd" "$(have "$cmd" && echo installed || echo "missing${inst:+ ($inst)}")"
  done <<<"$DEPS"
  printf '%-10s %-12s %-28s %s\n' ai default claude "$(have claude && echo installed || echo missing)"
}

main() {
  [ "${1:-}" = --list ] && { list; exit 0; }
  [ -r /dev/tty ] || { echo "install.sh is interactive and needs a terminal" >&2; exit 1; }
  say "Varde installer ($OS, $ARCH)"
  migrate_crime
  locate
  case "$UPDATE:$MODE" in
    1:source) echo "varde is installed from $CHECKOUT; updating it." ;;
    1:binary) echo "varde is installed as a binary at $BIN; replacing it with the latest Release." ;;
    0:binary) echo "Fresh install of the prebuilt binary." ;;
    0:source) echo "Fresh install from source." ;;
  esac
  if [ "$MODE" = binary ]; then
    say "Varde"
    require curl "curl" "sudo apt install -y curl"
    download
  else
    say "Required toolchain"
    toolchain
    fetch_and_build
  fi
  ask_deps
  say "Config"
  seed_config
  say "Package managers"
  installers
  say "Programs Varde runs"
  ai
  player
  opener
  retire_crime
  say "Done"
  case ":$PATH:" in
    *":$(dirname "$BIN"):"*) echo "Run: varde ." ;;
    *) echo "$(dirname "$BIN") is not on your PATH. Add it to your shell profile, then: varde ." ;;
  esac
}

main "$@"
