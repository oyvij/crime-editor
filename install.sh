#!/usr/bin/env bash
# Install or update CRIME and everything it shells out to.
#
#   curl -fsSL https://raw.githubusercontent.com/oyvij/CRIME/main/install.sh | bash
#
# Interactive: every prompt reads /dev/tty, so it works piped from curl. A
# `crime` already on PATH means the checkout it links into is pulled and rebuilt;
# otherwise the repo is cloned and linked the way docs/install.md describes.
#
# What CRIME can be configured to run — language servers, formatters, the voice —
# is read straight out of `DEFAULTS` in src/startup.rs, so a new row there with an
# `install.<os>` key is installable here with no change to this file. Only what
# the edge runs *without* configuration is spelled out below: the build
# toolchain, git, the default AI CLI and the URL opener.
#
# Windows is not covered: this is a bash script, and `DEFAULTS` carries its own
# `install.windows` rows for a hand install.

set -euo pipefail

REPO="${CRIME_REPO:-https://github.com/oyvij/CRIME.git}"
BIN="${CRIME_BIN:-$HOME/.local/bin/crime}"

case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux) OS=linux ;;
  *) echo "install.sh runs on macOS and Linux only" >&2; exit 1 ;;
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

# ---- rows out of DEFAULTS ----------------------------------------------------

# Prints `kind|name|command|install` for every [lsp.*], [formatter.*] and
# [speech] table in DEFAULTS, install being the `install.<os>` key or blank.
rows() {
  awk -v os="$OS" '
    /^pub const DEFAULTS: &str = r#"/ { on = 1; next }
    on && /^"#;/ { flush(); exit }
    !on { next }
    /^\[/ { flush(); sect = $0; gsub(/[][]/, "", sect); split(sect, parts, "."); kind = parts[1]; name = parts[2]; if (kind == "speech") name = "speech" }
    /^command = "/ { cmd = $0; sub(/^command = "/, "", cmd); sub(/"$/, "", cmd) }
    $0 ~ "^install\\." os " = \"" { inst = $0; sub(/^install\.[a-z]+ = "/, "", inst); sub(/"$/, "", inst); gsub(/\\"/, "\"", inst) }
    function flush() {
      if ((kind == "lsp" || kind == "formatter" || kind == "speech") && cmd != "") print kind "|" name "|" cmd "|" inst
      cmd = ""; inst = ""; kind = ""; name = ""
    }
  ' "$CHECKOUT/src/startup.rs"
}

# Player is `player.<os>` on the speech row: no install key, since afplay ships
# with macOS and aplay is alsa-utils.
player() {
  awk -v os="$OS" '$0 ~ "^player\\." os " = \"" { sub(/^player\.[a-z]+ = "/, ""); sub(/"$/, ""); print; exit }' "$CHECKOUT/src/startup.rs"
}

install_rows() { # install_rows <kind> — one y/N per missing command, deduplicated
  local seen=" "
  while IFS='|' read -r kind name cmd inst; do
    [ "$kind" = "$1" ] || continue
    [[ "$seen" == *" $cmd "* ]] && continue
    seen="$seen$cmd "
    if have "$cmd"; then
      echo "  $cmd ($name): installed"
      continue
    fi
    if [ -z "$inst" ]; then
      echo "  $cmd ($name): missing, and DEFAULTS has no install command for $OS — install it by hand"
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
  inst=$(rows | awk -F'|' '$1 == "speech" { print $4 }')
  dir=$(grep -oE -- '--output-dir [^ ]+' <<<"$inst" | cut -d' ' -f2)
  url=$(grep -oE 'https?://[^ ]+\.onnx( |$)' <<<"$inst" | head -1)
  [ -n "$dir" ] && [ -n "$url" ] && echo "${dir/#\~/$HOME}/$(basename "${url% }")"
}

reading() {
  install_rows speech
  local p
  p=$(player)
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

# ---- the checkout -------------------------------------------------------------

locate() { # sets CHECKOUT and MODE
  if have crime; then
    local link
    link=$(readlink "$(command -v crime)" || true)
    if [[ "$link" == */target/release/crime ]] && grep -qs '^name = "crime"' "${link%/target/release/crime}/Cargo.toml"; then
      CHECKOUT="${link%/target/release/crime}"; MODE=update; return
    fi
  fi
  MODE=fresh
  # Run from inside a checkout — `./install.sh` after a clone by hand — that
  # checkout is the default, so a private repo never needs a second clone.
  local here default="$HOME/.crime/src"
  here=$(cd "$(dirname "${BASH_SOURCE[0]:-.}")" 2>/dev/null && pwd)
  grep -qs '^name = "crime"' "$here/Cargo.toml" && default=$here
  local where
  read -r -p "Where should CRIME's checkout live? [$default] " where </dev/tty || where=""
  CHECKOUT="${where:-$default}"
  CHECKOUT="${CHECKOUT/#\~/$HOME}"
}

fetch_and_build() {
  if [ $MODE = update ]; then
    say "Updating $CHECKOUT"
    run "git -C '$CHECKOUT' pull --ff-only"
  elif [ -d "$CHECKOUT/.git" ]; then
    say "Reusing the checkout at $CHECKOUT"
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
}

# ---- the menu -----------------------------------------------------------------

FEATURES=("AI pane (claude)" "Language servers" "Formatters" "Reading aloud (piper voice)")
ON=(1 1 1 1)

menu() {
  local i pick
  while :; do
    say "Features to install or check (the editor, git and the build toolchain are always required):"
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
  CHECKOUT=$PWD
  local link
  link=$(readlink "$(command -v crime 2>/dev/null || true)" 2>/dev/null || true)
  [[ "$link" == */target/release/crime ]] && CHECKOUT="${link%/target/release/crime}"
  [ -f "$CHECKOUT/src/startup.rs" ] || { echo "run --list from a CRIME checkout, or with crime installed" >&2; exit 1; }
  printf '%-10s %-12s %-28s %s\n' kind name command state
  rows | while IFS='|' read -r kind name cmd inst; do
    printf '%-10s %-12s %-28s %s\n' "$kind" "$name" "$cmd" "$(have "$cmd" && echo installed || echo "missing${inst:+ ($inst)}")"
  done
  printf '%-10s %-12s %-28s %s\n' ai default claude "$(have claude && echo installed || echo missing)"
  printf '%-10s %-12s %-28s %s\n' player speech "$(player)" "$(have "$(player)" && echo installed || echo missing)"
}

main() {
  [ "${1:-}" = --list ] && { list; exit 0; }
  [ -r /dev/tty ] || { echo "install.sh is interactive and needs a terminal" >&2; exit 1; }
  say "CRIME installer ($OS)"
  locate
  [ $MODE = update ] && echo "crime is installed from $CHECKOUT; updating it." || echo "Fresh install."
  menu
  say "Required toolchain"
  toolchain
  fetch_and_build
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
