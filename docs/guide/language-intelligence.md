# Language intelligence

CRIME hosts a **Language server** for each language it is configured for and asks it what the
code means: what a name is, where it is defined, what is wrong with it, and what you might be
typing. The server is a separate program on your machine — `rust-analyzer`, `gopls`,
`pyright-langserver` — that CRIME starts when you open a file in its language and talks to in the
background. There is no pane for it; you only see its answers.

Nothing here takes anything away. A language with no server configured, or whose server is not
installed, edits exactly as it would otherwise. A server that cannot be started, or that exits,
is reported once — never on every keystroke — and the editor carries on.

## What a server gives you

### Diagnostics

Errors, warnings, information and hints the server reports are marked in the gutter beside the
line they name, each severity drawn differently, and the message for the diagnostic on the
cursor's line is shown in the footer. The characters the server pointed at are underlined; resting
the pointer on an underline shows that diagnostic's message in a box beside the line, and where two
overlap the worst is the one read out. Nothing about the gutter's width changes when diagnostics
arrive.

Marks survive switching to another buffer and back. Each time the server speaks again its previous
opinion of that file is replaced, so the gutter describes the present rather than keeping a log,
and a server that stops takes exactly its own marks with it.

In Review view each changed file shows how many errors and warnings its server reports, with a
total on the border. A file whose language has no server running, or whose server has not answered
yet, reads as **not measured** — never as zero — and a total that leaves such a file out is marked
with a leading `~`. See [Review](review.md).

### Hover — what is this?

`K` asks what the symbol under the cursor is; the reply — its type, and its documentation where
the server has any — opens in a box placed so it does not cover the symbol. `Esc` dismisses it.
Most servers answer in markdown, and the box renders it: headings, emphasis, and code fences
highlighted as their language. A box taller than the pane is capped and ends with an ellipsis so
you know it was cut. A symbol the server knows nothing about is said so by name rather than shown
as an empty box, and a reply that arrives after the cursor has moved on is not shown.

The same question is asked by the mouse: **rest the pointer on a symbol for half a second** and
its hover appears, in normal or insert mode, without moving the cursor. The box describes where the
pointer is, so it goes away when the pointer moves off the symbol or leaves the editor. A pointer
crossing the pane on its way somewhere else asks nothing.

### Definition — where is this from?

`gd` asks where the name under the cursor is introduced.

- A definition in the file you are reading moves the cursor there.
- A definition in another workspace file opens that file at the line.
- Several definitions are listed in the project search's results box, so you choose rather than
  the first being taken quietly.
- No definition says so and moves nothing.
- A definition **outside the workspace root** — in a dependency's source, say — is refused, naming
  the path it would have opened. Every pane in CRIME is bounded by the root, and a file outside it
  is one the tree cannot mark and the watcher cannot follow.

A jump to a definition is recorded, so `Ctrl+P` (or `gp`) takes you back — including from a
definition in the same file. See [Editing](editing.md#going-back).

**Click to definition.** Hold `Cmd` (or `Ctrl` — the two are one gesture here, since most
terminals report a `Cmd`-click with no modifier at all) while pointing at a name in the editor and
the name is underlined; that underline is the whole affordance, since a terminal has no hand
pointer. Clicking it with the modifier held asks the same question `gd` does. A plain click only
places the cursor.

### Candidates

While you type an identifier in insert mode, CRIME asks the server what you might mean once typing
has paused for a moment, and offers the **Candidates** in a list just below the line you are on, at
the cursor's column. The list is ten rows at most and scrolls; every Candidate stays available.

| Key | Does |
|---|---|
| arrows | move the choice |
| `Enter` | accept the selected Candidate, replacing the word you were typing |
| `Esc` | close the list, leaving the buffer holding exactly the characters you typed |
| more letters | narrow the list; `Backspace` widens it again |

Dismissing never changes what you wrote — that is the promise the whole feature rests on. A word
that matches no Candidate closes the list rather than showing an empty one, and the list goes if
you jump to another file. The order is the server's own where it gave one.

**Blanks.** Some Candidates are templates — `println!("…")`, `foo(…, …)` — with places a value goes.
Accepting one inserts the text with those placeholders removed and puts the cursor on the first
blank. `Tab` moves to the next blank; at the last one the cursor lands where the code continues and
the sequence is over. `Esc` ends the sequence and keeps the text (abandoning a Candidate is not
undoing it), and opening another file ends it too. While blanks remain `Tab` means "next blank";
the rest of the time it indents, as [Editing](editing.md#typing) describes. A plain Candidate has
no sequence: its text goes in and the cursor lands after it.

### Laying out as you type

A server may name characters it wants to be told about — `}` or `;` in some languages, a newline
in others — and when you type one it may send back edits that re-indent or re-space the code
around it. Those edits are applied as one thing, so a single `u` takes them back, and are dropped
if you have typed on since they were asked for.

### `:format`

`:format` lays the whole buffer out. It asks the Language server first; where the server declares
no formatting of its own it asks the configured **Formatter** for the language instead — an
external command such as `prettier`, `black` or `rustfmt`. Either way:

- The command sees the **text on screen**, never the file on disk: the buffer goes in on the
  command's stdin and what it writes on stdout replaces the buffer. **Nothing is written** — an
  unsaved buffer stays unsaved and `:w` is yours. A tool that can only rewrite a file in place is
  therefore not supported.
- The result is **one undo step**, and the cursor rides the change.
- A command that hands back exactly what it was given says so — `nothing to format` — because a key
  that changes nothing and says nothing reads as broken.
- A command that fails says so **in its own words**, quoting the first line of its error output,
  and the buffer is left alone. A command that exits cleanly and prints nothing is treated as a
  failure too, never as "format the file empty".
- A language nothing configures refuses out loud, naming the key to write:
  `[formatter.<language>]` in `.crime/config.toml`.
- A configured command that is **not installed** puts its install command on the terminal's input
  line and moves focus there, without pressing Enter. Nothing is remembered about it being missing:
  the next `:format` after you install it just works, with no restart.
- Over a markdown Preview, `:format` crosses to Source first, so the undo and the line numbers are
  where you can see them. With nothing open it refuses.

Which language a file is, for formatting, is looked up three ways: the language the server
configuration knows it as, then the `extensions` a formatter row claims, then the file's own
extension — or, for a file with none, its name, so `[formatter.Makefile]` is a key you can write.

## Tools

`Ctrl+Space` then `v` (the palette's **Tools** entry) opens the list of everything CRIME runs,
grouped into language servers, formatters, requirements (`[facts.*]`) and speech (the synthesizer
with its voice, and the player). One row each, with the command it runs and its state on this
machine. It is probed when the list opens, so the moment after an install is the moment to look.
Opening the list starts no server.

Every template row your config files do not name is listed too, as `available`: a row you deleted,
or one a newer CRIME added. A row that differs from the template's says so, since a corrected
template never edits a row you already have.

| State | Means |
|---|---|
| `installed` | the command is on your `PATH` and, if it has been started, it is answering |
| `missing` | the command is not on your `PATH` |
| `stopped` | the command is here but its server exited — see `.crime/lsp-<language>.log` |
| `missing-requirement` | the command is here but something it needs is not, e.g. a TypeScript SDK in the workspace; the row says which |
| `partly-working` | the configuration says this server cannot do something, and the row names it |
| `no-install-command` | not installed, and nothing is configured to install it on this OS |
| `available` | a template row no config file names, so CRIME does not run it |
| `install-failed` | its install ran and exited with a failure — the output is in the terminal pane |

| Key | Does |
|---|---|
| `i` | take the row: add it to `~/.crime/config.toml` if the file lacks it, and run its install command for this OS in the terminal pane |
| `r` | re-check the row |
| `Esc` | close the list |

The row is appended after the last line of your global config, with any requirement it names,
and nothing you wrote is touched; a row the file already has is not written again. The install runs
where you can watch it and answer a `sudo` prompt, and when it ends CRIME checks for the command
again. If your global config does not parse, `i` names the fault and writes and runs nothing. A row
already `installed` refuses `i`; a `stopped` row runs its install, since reinstalling is exactly
what fixes a command that is present and does not work. Once the command appears the server
starts, with no restart.

One case needs a restart: an installer that added its directory to your shell profile, which a
running CRIME cannot see. If a re-check after installing still finds nothing, CRIME asks whether to
restart; answering yes quits (unsaved buffers are refused exactly as `Ctrl+Q` refuses them), and
declining leaves everything as it was.

## What ships

These servers are configured out of the box. Install one and it works — nothing else to configure.
The `install.sh` described in [Installing CRIME](../install.md) offers the same commands. A blank
cell means nobody has packaged that server for that OS; add an `install.<os>` key yourself (below).

| Language | Server | macOS | Linux | Windows |
|---|---|---|---|---|
| Rust | `rust-analyzer` | `rustup component add rust-analyzer` | same | same |
| TypeScript | `typescript-language-server` | `npm install -g typescript typescript-language-server` | same | same |
| JavaScript | `typescript-language-server` | same as TypeScript | same | same |
| Vue | `vue-language-server` | `npm install -g @vue/language-server` | same | same |
| Python | `pyright-langserver` | `npm install -g pyright` | same | same |
| Go | `gopls` | `go install golang.org/x/tools/gopls@latest` | same | same |
| C | `clangd` | — | `sudo apt install clangd` | `winget install LLVM.LLVM` |
| C++ | `clangd` | — | `sudo apt install clangd` | `winget install LLVM.LLVM` |
| Java | `jdtls` | `brew install jdtls` | — | — |
| Zig | `zls` | `brew install zls` | — | — |

C and C++ have no macOS command because Homebrew's `llvm` is keg-only: the install would succeed
and `clangd` would still not be on `PATH`, so the row would go on reading `missing`.

Formatters, for `:format` where the server does not format:

| Language | Formatter | Install |
|---|---|---|
| Rust | `rustfmt` | `rustup component add rustfmt` |
| Python | `black` | `pipx install black` (Windows: `pip install black`) |
| Go | `gofmt` | ships with the Go toolchain |
| JavaScript, TypeScript, Vue | `prettier` | `npm install -g prettier` |
| JSON (`.json`, `.jsonc`) | `prettier` | same |
| YAML (`.yaml`, `.yml`) | `prettier` | same |
| HTML (`.html`, `.htm`) | `prettier` | same |
| CSS (`.css`, `.scss`, `.less`) | `prettier` | same |
| Markdown (`.md`, `.markdown`) | `prettier` | same |

## Configuring a server or a formatter

Servers and formatters are configuration, never code. The shipped rows are the bottom layer;
`~/.crime/config.toml` beats them and `<project>/.crime/config.toml` beats that, **key by key** — a
project that sets one key of a shipped language keeps every other key of it. A language nobody at
CRIME has heard of is served by adding a table. See [Configuration](configuration.md).

```toml
[lsp.zig]
command = "zls"                              # what to run; found on PATH or given as a path
args = ["--stdio"]                           # optional
install.macos = "brew install zls"           # one per OS; omit where nothing is packaged
install.linux = "zig build -Doptimize=ReleaseSafe"
partial = "type errors"                      # optional: what this server cannot do, in your words
also_served_by = ["typescript"]              # optional: other languages whose servers also serve these files

[lsp.zig.initialization_options]             # optional: passed to the server verbatim, never read by CRIME
some_option = true
```

A malformed table, or a language no layer ever gave a `command`, stops CRIME from starting and
names the file and the line.

```toml
[formatter.Makefile]
command = "some-formatter"                   # must read stdin and write stdout
args = ["--stdin-filepath", "${file}"]       # ${file} tells a multi-language tool what it is reading
extensions = ["mk"]                          # optional: which extensions this row claims
install.macos = "brew install some-formatter"
```

### Paths the server needs: facts

Some servers need a path that lives inside your workspace and differs from machine to machine. The
TypeScript server, for instance, will not answer at all unless told where a usable `tsserver.js`
is, and the Vue server needs the same SDK as `--tsdk=`. CRIME ships no such path — an invented one
that does not exist is worse than an honest blank — and instead ships a **fact** that finds it at
every start:

```toml
[facts.typescript_sdk]
marker = "node_modules/typescript/lib/typescript.js"   # what to look for, from the file's folder up to the root
value = "directory"                                    # hand over the directory holding it, not the file
command = "tsc"                                        # fallback: a command on PATH...
command_marker = "../lib/typescript.js"                # ...and the marker relative to it
```

Anywhere a string reaches a server — `args`, or inside `initialization_options` — `${typescript_sdk}`
is replaced with what the search found. The shipped TypeScript row sets
`initialization_options.tsserver.path = "${typescript_sdk}/tsserver.js"`, and the Vue row passes
`--tsdk=${typescript_sdk}`. The search runs from the file being served upward, so in a monorepo each
package's own pinned TypeScript is the one used, and a global preview build with no `typescript.js`
is not mistaken for one.

A fact that cannot be found starts no server, and the row reads `missing-requirement` saying which
fact — a server launched without what it needs would only die on the first file. A fact marked
`optional = true` is dropped instead and the server starts without it: the shipped
`vue_typescript_plugin` fact is optional because it is named on the TypeScript row every
TypeScript project shares, and a machine with no Vue server must still get a TypeScript server.

### The worked example: Vue

A `.vue` file is served by two servers at once. `[lsp.vue]` names `also_served_by = ["typescript"]`,
so `gd`, `K` and diagnostics ask both; the first server with an answer wins, and "nobody knows" is
said only once the last one has answered. The TypeScript server can answer about a `.vue` file
only when loaded with the plugin the Vue server carries in its own `node_modules`, which is what the
optional `vue_typescript_plugin` fact finds. Installing the two servers from the list above is the
whole of the setup: template mistakes from one server and type errors from the other land in the
same gutter.

The Vue server also asks its client a question CRIME cannot answer; `[lsp.vue].unanswerable` names
the pair of methods so it is refused out loud and the server carries on with what it can do alone.
If a file's only server has asked such a question, an empty `gd` or `K` says the server needed a
companion rather than blaming the server for knowing nothing.

## When something goes wrong

Each server's own error output goes to `.crime/lsp-<language>.log` in the workspace, truncated
each time the server is started. The two notices that can send you there are "could not start the
language server" — check the `command` in `.crime/config.toml` and the log — and "the language
server stopped", after which there are no diagnostics, hover or completion for that language until
it runs again. Fix what the log says, then `r` in the server list or simply open a file in that
language; nothing needs restarting unless the list asks.

## See also

- [Editing](editing.md) — the keys around these: `Tab`, `u`, `Ctrl+P`, the results box.
- [Configuration](configuration.md) — where `[lsp.*]`, `[formatter.*]` and `[facts.*]` live and how
  the layers merge.
- [Installing CRIME](../install.md) — `install.sh` reads these same rows and offers to install them.
- [Review](review.md) — the error and warning counts over a change.
