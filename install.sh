#!/usr/bin/env bash
# Install or update CRIME and everything it shells out to.
#
#   curl -fsSL https://raw.githubusercontent.com/oyvij/crime-editor/main/install.sh | bash
#
# Interactive: every prompt reads /dev/tty, so it works piped from curl. A fresh
# machine gets the binary from the latest Release by default, verified against
# its SHA256SUMS; run from inside a checkout, or answered "source", it clones and
# builds instead. Re-run, it updates whichever kind it finds behind `crime`.
#
# What CRIME can be configured to run — language servers, formatters, the voice —
# is asked of the installed binary with `crime --deps`, so a new row in `DEFAULTS`
# with an `install.<os>` key is installable here with no change to this file. Only
# what the edge runs *without* configuration is spelled out below: the build
# toolchain, git, the default AI CLI and the URL opener.
#
# Windows is not covered: this is a bash script, and `DEFAULTS` carries its own
# `install.windows` rows for a hand install.

set -euo pipefail

# Clone URL and Release source both: a fork overrides one variable.
REPO="${CRIME_REPO:-https://github.com/oyvij/crime-editor.git}"
BIN="${CRIME_BIN:-$HOME/.local/bin/crime}"

case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux) OS=linux ;;
  *) echo "install.sh runs on macOS and Linux only" >&2; exit 1 ;;
esac

# Spelled as the release workflow's asset names, `crime-<os>-<arch>`.
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
  require cargo "the Rust toolchain" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
}

# ---- the installers the install commands themselves need ---------------------

# The first word of an `install.<os>` command names the package manager it
# assumes. This is where each one comes from when it is not there yet.
installer_for() {
  case "$1" in
    sudo) echo apt ;;
    npm | go | pipx | uv | brew | rustup) echo "$1" ;;
    *) echo "" ;;
  esac
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
    *) echo "" ;;
  esac
}

ensure_installer() { # ensure_installer "<install command>" -> 0 if its package manager is present
  local tool
  tool=$(installer_for "${1%% *}")
  [ -z "$tool" ] && return 0
  have "$tool" && return 0
  local how
  how=$(installer_install "$tool")
  [ -z "$how" ] && { echo "  needs $tool, which this script cannot install on $OS"; return 1; }
  if [ $OS = macos ] && ! have brew; then
    case "$tool" in npm | go | pipx) ensure_installer brew || return 1 ;; esac
  fi
  ask "  $tool is needed to run that. Install it with: $how ?" || return 1
  run "$how"
  [ -x /opt/homebrew/bin/brew ] && eval "$(/opt/homebrew/bin/brew shellenv)"
  export PATH="$HOME/.local/bin:$PATH"
  have "$tool"
}

# ---- rows out of `crime --deps` ------------------------------------------------

# `kind<TAB>name<TAB>command<TAB>install` for every [lsp.*], [formatter.*] and
# [speech] row, plus the speech row's `player`. Asked once, up front: a failure
# inside `< <(rows)` would not stop the script and would read as nothing missing.
ask_deps() {
  DEPS=$("$EXE" --deps) || { echo "$EXE --deps failed; cannot tell what CRIME needs." >&2; exit 1; }
}
rows() {
  printf '%s\n' "$DEPS"
}

install_rows() { # install_rows <kind> — one y/N per missing command, deduplicated
  local seen=" "
  while IFS=$'\t' read -r kind name cmd inst; do
    [ "$kind" = "$1" ] || continue
    [[ "$seen" == *" $cmd "* ]] && continue
    seen="$seen$cmd "
    if have "$cmd"; then
      echo "  $cmd ($name): installed"
      continue
    fi
    if [ -z "$inst" ]; then
      echo "  $cmd ($name): missing, and crime --deps names no install command for $OS — install it by hand"
      continue
    fi
    ask "  $cmd ($name) is missing. Install with: $inst ?" || continue
    ensure_installer "$inst" || { echo "  skipped $cmd"; continue; }
    run "$inst" || echo "  $inst failed; $cmd stays missing"
  done < <(rows)
}

# ---- the voice ----------------------------------------------------------------

voice_path() { # where the speech install command puts its .onnx: `--output-dir` plus the URL's basename
  local inst dir url
  inst=$(rows | awk -F'\t' '$1 == "speech" { print $4 }')
  dir=$(grep -oE -- '--output-dir [^ ]+' <<<"$inst" | cut -d' ' -f2)
  url=$(grep -oE 'https?://[^ ]+\.onnx( |$)' <<<"$inst" | head -1)
  [ -n "$dir" ] && [ -n "$url" ] && echo "${dir/#\~/$HOME}/$(basename "${url% }")"
}

reading() {
  install_rows speech
  local p
  p=$(rows | awk -F'\t' '$1 == "player" { print $3 }')
  if [ -n "$p" ] && ! have "$p"; then
    if [ $OS = linux ]; then
      ask "  $p (plays the speech) is missing. Install with: sudo apt install -y alsa-utils ?" && run "sudo apt install -y alsa-utils"
    else
      echo "  $p is missing; it ships with macOS, so something is unusual here"
    fi
  fi
  local model config="$HOME/.crime/config.toml"
  model=$(voice_path)
  [ -n "$model" ] && [ -f "$model" ] || return 0
  grep -qs '^voice = ' "$config" && return 0
  mkdir -p "$HOME/.crime"
  if grep -qs '^\[speech\]' "$config"; then
    awk -v v="voice = \"$model\"" '{ print } /^\[speech\]$/ { print v }' "$config" > "$config.tmp" && mv "$config.tmp" "$config"
  else
    printf '\n[speech]\nvoice = "%s"\n' "$model" >> "$config"
  fi
  echo "  set speech.voice = \"$model\" in $config"
}

# ---- the AI pane --------------------------------------------------------------

ai() {
  # `claude` is what `ai.command` defaults to (src/lib.rs); `:ai <command>` picks another.
  if have claude; then echo "  claude: installed"; return; fi
  ask "  claude (the default AI CLI) is missing. Install with: curl -fsSL https://claude.ai/install.sh | bash ?" || return 0
  run "curl -fsSL https://claude.ai/install.sh | bash"
}

# ---- the URL opener (Linux only; `open` ships with macOS) ---------------------

opener() {
  [ $OS = linux ] || return 0
  have xdg-open && return 0
  ask "  xdg-open (opens URLs clicked in a pane) is missing. Install with: sudo apt install -y xdg-utils ?" && run "sudo apt install -y xdg-utils"
}

# ---- which install, and where ---------------------------------------------------

locate() { # sets MODE (binary or source), and CHECKOUT for source; UPDATE=1 when crime is already installed
  UPDATE=0
  if have crime; then
    UPDATE=1
    local path link
    path=$(command -v crime)
    link=$(readlink "$path" || true)
    if [[ "$link" == */target/release/crime ]] && grep -qs '^name = "crime"' "${link%/target/release/crime}/Cargo.toml"; then
      CHECKOUT="${link%/target/release/crime}"; MODE=source; return
    fi
    if [ -L "$path" ]; then
      echo "$path is a symlink to $link, which is neither a CRIME checkout nor a binary this script installed; remove it and rerun." >&2
      exit 1
    fi
    # Replaced where it is found, so an install outside ~/.local/bin is not duplicated there.
    BIN=$path; MODE=binary; return
  fi
  # Run from inside a checkout — `./install.sh` after a clone by hand — that
  # checkout is built, so a private fork never needs a second clone.
  local here default="$HOME/.crime/src" answer
  here=$(cd "$(dirname "${BASH_SOURCE[0]:-.}")" 2>/dev/null && pwd)
  if grep -qs '^name = "crime"' "$here/Cargo.toml"; then
    default=$here
  else
    read -r -p "Install the prebuilt binary, or build from source? [binary/source] " answer </dev/tty || answer=""
    [[ "$answer" =~ ^[Ss] ]] || { MODE=binary; return; }
  fi
  MODE=source
  local where
  read -r -p "Where should CRIME's checkout live? [$default] " where </dev/tty || where=""
  CHECKOUT="${where:-$default}"
  CHECKOUT="${CHECKOUT/#\~/$HOME}"
}

# ---- the binary path ------------------------------------------------------------

sha256() { # the hash alone; sha256sum on Linux, shasum on macOS
  if have sha256sum; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

download() {
  local web="${REPO/#git@github.com:/https://github.com/}"
  local asset="crime-$OS-$ARCH" base="${web%.git}/releases/latest/download"
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
  # A temporary file beside the target and a rename, so a crime that is running
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
  ln -sfn "$CHECKOUT/target/release/crime" "$BIN"
  echo "  $BIN -> $CHECKOUT/target/release/crime"
  EXE="$CHECKOUT/target/release/crime"
}

# ---- the menu -----------------------------------------------------------------

FEATURES=("AI pane (claude)" "Language servers" "Formatters" "Reading aloud (piper voice)")
ON=(1 1 1 1)

menu() {
  local i pick
  while :; do
    say "Features to install or check (the editor itself is always installed):"
    for i in "${!FEATURES[@]}"; do
      printf '  %d. [%s] %s\n' $((i + 1)) "$( [ "${ON[$i]}" = 1 ] && echo x || echo ' ')" "${FEATURES[$i]}"
    done
    read -r -p "Type a number to toggle, Enter to continue: " pick </dev/tty || pick=""
    [ -z "$pick" ] && return
    [[ "$pick" =~ ^[0-9]+$ ]] && (( pick >= 1 && pick <= ${#FEATURES[@]} )) || continue
    i=$((pick - 1))
    ON[i]=$(( 1 - ON[i] ))
  done
}

# ---- --list: what would be checked, and its state, without touching anything --

list() {
  EXE=$(command -v crime 2>/dev/null || true)
  [ -n "$EXE" ] || { [ -x "$BIN" ] && EXE=$BIN; }
  [ -n "$EXE" ] || { echo "crime is not installed; --list asks the installed binary what it needs" >&2; exit 1; }
  ask_deps
  printf '%-10s %-12s %-28s %s\n' kind name command state
  rows | while IFS=$'\t' read -r kind name cmd inst; do
    printf '%-10s %-12s %-28s %s\n' "$kind" "$name" "$cmd" "$(have "$cmd" && echo installed || echo "missing${inst:+ ($inst)}")"
  done
  printf '%-10s %-12s %-28s %s\n' ai default claude "$(have claude && echo installed || echo missing)"
}

main() {
  [ "${1:-}" = --list ] && { list; exit 0; }
  [ -r /dev/tty ] || { echo "install.sh is interactive and needs a terminal" >&2; exit 1; }
  say "CRIME installer ($OS, $ARCH)"
  locate
  case "$UPDATE:$MODE" in
    1:source) echo "crime is installed from $CHECKOUT; updating it." ;;
    1:binary) echo "crime is installed as a binary at $BIN; replacing it with the latest Release." ;;
    0:binary) echo "Fresh install of the prebuilt binary." ;;
    0:source) echo "Fresh install from source." ;;
  esac
  menu
  if [ "$MODE" = binary ]; then
    say "CRIME"
    require curl "curl" "sudo apt install -y curl"
    download
  else
    say "Required toolchain"
    toolchain
    fetch_and_build
  fi
  ask_deps
  [ "${ON[0]}" = 1 ] && { say "AI pane"; ai; }
  [ "${ON[1]}" = 1 ] && { say "Language servers"; install_rows lsp; }
  [ "${ON[2]}" = 1 ] && { say "Formatters"; install_rows formatter; }
  [ "${ON[3]}" = 1 ] && { say "Reading aloud"; reading; }
  opener
  say "Done"
  case ":$PATH:" in
    *":$(dirname "$BIN"):"*) echo "Run: crime ." ;;
    *) echo "$(dirname "$BIN") is not on your PATH. Add it to your shell profile, then: crime ." ;;
  esac
}

main "$@"
