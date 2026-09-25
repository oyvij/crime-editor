# Varde — Stack

Rust, chosen after the spec was complete. The two components that dominate this product — an
**embedded terminal emulator** and a **modal editor with syntax highlighting** — have mature,
reusable crates in Rust and essentially no equivalent in Node.

## Why not Node/ink

Claude Code is built with ink, and that is a fair reference for what ink is good at: a single-pane
chat REPL — a scrolling list of messages, spinners, an input line. Varde is a four-pane IDE with a
live shell in one of them. In Node, `node-pty` spawns the process, but the terminal *model* is the
hard part and `xterm.js` is DOM-bound; there is no mature headless cell-grid renderer for a TUI. You
would be writing an escape-sequence parser by hand.

Go was the runner-up: `bubbletea` is an excellent TUI framework and `vt10x` is a workable terminal
model. It loses on the editor — tree-sitter via cgo, and no reference implementation of a modal
editor to learn from.

## Crates

| Concern | Crate | Notes |
|---|---|---|
| TUI framework | `ratatui` | Layout, widgets, the four-pane frame |
| Terminal backend | `crossterm` | Cross-platform input/output; also the Kitty keyboard protocol (below) |
| Key encoding | `terminput` + `terminput-crossterm` | A hosted pane forwards every key to its child as the bytes a real terminal would send. `terminput::KeyEvent` is the lossless event type — nothing is collapsed on the way in — and `Event::encode(&mut buf, Encoding::Xterm)` produces the legacy sequence. Do **not** hand-roll it: *which bytes does this modifier combination send* is a solved problem whose edge cases nobody discovers until they press the key, which is exactly what AGENTS.md's "use the crate, don't write the function" is for. `terminput-crossterm` converts the edge's crossterm events; its default feature already targets crossterm 0.29, the version in the tree. Two small crates from one author, and no second terminal stack comes with them |
| Host colours | `terminal-colorsaurus` | Asks the host terminal once, at startup, for its foreground and background (OSC 10/11), so the space dots and indentation guides can be mixed between them at a strength Varde picks rather than one DIM picks. Detecting a terminal that will not answer, without waiting out a timeout, is the edge case that makes this a crate and not a `write!`; bat and delta use it. No answer falls back to DIM |
| Terminal emulation | `vt100` | **`wezterm-term` is not published to crates.io** — corrected after trying to add it. `vt100` parses escape sequences into a cell grid and, crucially, exposes `mouse_protocol_mode()`, which is exactly R10.5's question: has the running program asked for mouse events? Its `Callbacks::unhandled_csi` is the other load-bearing capability: every sequence it does not implement — the child's questions among them — is surfaced rather than dropped, so Varde can answer them without a parser of its own. `alacritty_terminal` and `termwiz` are the alternatives if more fidelity is needed |
| Pty process | `portable-pty` | Spawns and sizes the user's shell; same family as `wezterm-term` |
| Text buffer | `ropey` | Rope structure — edits in the middle of a large file stay cheap |
| Syntax highlighting | `syntect` | **Chosen over tree-sitter.** One crate, no C grammars or highlight queries to wire, and it hands back scopes directly. tree-sitter remains the answer if a *structural* feature arrives — folding, selection by node, smart indent — because those need a parse tree, not scopes |
| Syntax set | `two-face` | `syntect`'s own `load_defaults_nonewlines()` is Sublime's default set — **75 syntaxes, naming none of** TypeScript, TSX, Vue, Svelte, Kotlin, Swift, Zig, Dart, TOML, Terraform, Nix, Elixir, protobuf, Dockerfile, GraphQL or SCSS, so Varde's own `config.toml` rendered flat while the file tree beside it coloured a `.ts` file by its language. `two-face` packages `bat`'s extended set — **220 syntaxes** — as a **prebuilt syntect dump**, so it is a drop-in for the constructor: same `SyntaxSet` type, no assets directory, no licence audit, and no build script. Vendoring `.sublime-syntax` files was rejected for the last of those: parsing 220 YAML grammars costs hundreds of milliseconds on every start, and a dump is `include_bytes!` plus a deserialise. What makes startup cost nothing is structural, not the size of that deserialise: the set sits behind an `OnceLock` whose only caller is the edge's per-buffer highlight cache, so opening Varde on a folder never touches it. The one-off figure, if anyone wants it again, was 2.9 ms against the default set's 1.3 ms — a release build calling both constructors, not a benchmark the suite keeps. **Taken with default features off and `syntect-onig` on**: the feature is a `#[cfg]` over *which dump is embedded*, and with it off the crate embeds the fancy-regex dump, which selects syntect's `regex-fancy` and compiles a second regex engine beside `onig`. Adding the crate locked exactly one package — but the lockfile is the weaker check, because it already lists `fancy-regex` whether or not a feature selects it, so a future bump could pass there and still compile two engines. `cargo tree -i syntect -e features` is the check that holds: it must print `regex-onig` and no `regex-fancy`. Its grammars are also better than the defaults': `Vec` as a type, a JavaScript object property, and Go's malformed `09` all started classifying correctly on the swap, with no change to `scope_kind` |
| File watching | `notify`, `notify-debouncer-full` | Debouncing matters: an AI writing a file fires several events |
| Git | `git2` | `Repository::statuses()` gives exactly F7's rule — uncommitted vs HEAD, untracked included, ignored excluded. For the diff itself use `diff_tree_to_workdir_with_index` against HEAD, **not** `diff_index_to_workdir`: the latter misses staged changes, and misses untracked files entirely unless `include_untracked` and `show_untracked_content` are set — so a file the AI just created lists in the review with an empty diff. A Story's range is the one diff that is *not* against the working tree: `main...HEAD` names two commits — the merge-base and the head, resolved with `merge_base` so a base that moved on since the head forked does not read as a deletion — and `diff_tree_to_tree` is what makes its hunks the range's own — diffing the working tree over a fully committed range finds nothing and the Remainder reports vacuous full coverage. Same pinned `context_lines` on every path; the untracked options are inert on the tree-to-tree one |
| Version comparison | `semver` | The update check compares the checkout's Version with the Running version. Do **not** hand-roll it: as strings `"0.10.0" < "0.9.0"`, so a byte-wise compare calls the newer checkout older and the Update never appears. Already in the lockfile as a transitive dependency, so making it direct downloads nothing |
| Checksums | `sha2` | F39's R39.4: a downloaded Asset is verified against the Release's `SHA256SUMS` before it replaces the running binary. Small, pure Rust, one call, and it keeps the verification in the library under a unit test — shelling out instead means `shasum` on macOS and `sha256sum` on Linux, which is a platform branch for a solved problem |
| Shell quoting | `shlex` | R3.5. Do **not** hand-roll quoting — see the note below |
| Gitignore matching | `ignore` | The crate behind ripgrep. R2.3 (dim ignored paths) and R7.2 (exclude them from review) are both ignore-semantics problems, and those semantics are far subtler than they look |
| Layered config | `figment` (or `serde` + `toml` directly) | R9.4 is global → project → defaults precedence, which is exactly what figment does. Evaluate before hand-writing a deep merge |
| Config | `serde` + `toml` | F9's `config.toml` |
| Editing the global config | `toml_edit` | ADR 0018: taking a Tools row appends the template's row (and any `[facts.*]` row it names) to `~/.varde/config.toml`. `toml` reads a file into values and writes values back, which would drop every comment and hand alignment the reader made; `toml_edit` keeps a document's text, so the row is cut out of the template with the comment above it and its sub-tables whole — `Item::to_string` leaves the sub-tables out, which is why the row is rendered as a document. The reader's text is never reparsed and rewritten: the rendered rows go after its last byte. The same `toml-rs` family as `toml`, sharing its `toml_parser`, so it locked one package |
| State | `serde` + `serde_json` | F9's `state.json` |
| Text wrapping | `textwrap` | An overlay is sized to its longest line and then clamped to the screen, so a long `why` or Nudge was drawn clipped — the tail of the sentence simply was not there. The lines are wrapped before the box is measured rather than by `Paragraph::wrap`, because the height is computed from the line count and a paragraph that wraps at render time makes that count a lie. Do **not** hand-roll it: word breaking, an over-long word with nowhere to break, and Unicode width are each a solved problem. Already in the lockfile behind `clap`, so making it direct downloads nothing — the same reason `semver` is direct |
| Markdown parsing | `pulldown-cmark` | F26's Preview. The CommonMark pull parser everything in Rust builds on, with no transitive tail of its own. It emits events, not HTML, which is what lets the layout stay a pure function in `src/preview.rs` returning rows rather than a string somebody has to re-parse. GFM is opt-in per flag — `ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS` — and `ENABLE_YAML_STYLE_METADATA_BLOCKS` is why frontmatter arrives as its own event instead of rendering as a horizontal rule followed by stray text. Taken with `default-features = false`: the defaults pull in an HTML renderer and `getopts` for a binary this never builds, and Varde wants events, not HTML. Brings `unicase` |
| Mermaid diagrams | `mermaid-text` | F26's R26.6. `render_with_width(src, Some(cols)) -> Result<String, Error>` — text to text, with a column budget: no browser, no `mmdc` subprocess, no Kitty or sixel image protocol, so a diagram is laid out under a unit test like everything else rather than becoming an `Effect` the edge executes. `mermaid-svg` and `mermaid-rs-renderer` cover more diagram types and emit SVG, which is nothing in a cell grid. **Named honestly: this is a niche 0.x crate from one author, and it brings `ascii-dag`, `unicode-width` and `chrono`.** It was taken anyway because the alternative to a crate here is a graph layout engine — the "looks small, and is wrong in ways no scenario covers" code AGENTS.md forbids — and because R26.6's error path is the whole behaviour without it, so backing it out is deleting one call. `render_with_width` never actually returns `Error::TooWide` — that variant needs `RenderOptions::max_width_strict`, which the plain function does not expose — so "too wide for the pane" is decided in `src/preview.rs` after the fact, not read off the crate |
| Display width | `unicode-width` | `render_with_width`'s own compaction pass measures in display columns, so the too-wide check that follows it has to as well — `str::chars().count()` undercounts a wide character (CJK text in a node label) and would silently pass a diagram still too wide for the pane, or refuse one that fit. Already in the lockfile behind `mermaid-text` and `ratatui`, so making it direct downloads nothing — the same reason `semver` is direct |
| Civil dates | `chrono` | F40's Authorship. A commit's timestamp is seconds since the epoch plus the offset its author was at, and the border shows a date — so the conversion is calendar arithmetic, which is leap years, leap-year exceptions and the day a timezone shifted. Exactly the "looks small, and is wrong in ways no scenario covers" code AGENTS.md forbids hand-rolling. `DateTime::from_timestamp` and `FixedOffset` are the whole of what is used, in `short_date` in `src/main.rs` and nowhere else: the core carries the formatted string it was told. Taken with `default-features = false` and `std` only — no `clock`, because the only instant that matters is the one git recorded, and the system clock is not it. Already in the lockfile behind `mermaid-text`, so making it direct downloads nothing — the same reason `unicode-width` and `semver` are direct |
| Code metrics | `rust-code-analysis` | F27's Risk. Mozilla's tree-sitter-based analyser, pinned to the published **0.0.25** — a git dependency was rejected: the release is old and carries older grammars, and the consequence is Unparsed files, which Varde surfaces rather than hides. That is the accepted trade against depending on an unversioned moving target. It gives per-space metrics with names and start lines for the six languages that actually implement cyclomatic and cognitive complexity — Python, Rust, C/C++, Java, JavaScript, TypeScript/TSX — so no other analyser is shelled out to and no metric is hand-rolled. Two constraints come with the pin. **Its types reach `src/main.rs` and nowhere else**: `0.0.x` may break its API on any upgrade, so `src/risk.rs` defines Varde's own `Space`/`Metrics` and the edge converts field for field — grep `rust_code_analysis` and every hit is in the edge. And **`tree-sitter` is held at 0.20.9 in `Cargo.lock`**, because `tree-sitter-rust` 0.20.3 asks for `>=0.20` and resolving that to 0.27 gives two incompatible `Language` types and a compile error inside the analyser's own macro; if a `cargo update` breaks the build here, that is what happened — `cargo update tree-sitter@<new> --precise 0.20.10` puts it back |
| Run marks | `tree-sitter` + `tree-sitter-{rust,java,javascript,typescript,python}` | F41. The structural feature the `syntect` row said tree-sitter was waiting for: a `[run.*]` row finds what can be started with a tree-sitter **query**, which is data, so no arm names a `main` or a test. **Taken at exactly the versions `rust-code-analysis` already locked** — tree-sitter 0.20.9 and those five grammars — so adding them compiled nothing new and locked no package; a second tree-sitter runtime is not an option, because both would link the same `ts_*` C symbols. The grammar table in `src/run.rs` is the one place an extension meets a parser, and it is only as wide as that set: a row for a language outside it is refused at start as `no grammar parses .<ext>` rather than read as configured. Loading grammars from shared libraries would lift that limit, and was left out: it is native code named by configuration, which a project config — the opened folder — can supply |
| LSP wire types | `lsp-types` | F31. The protocol's request, notification and reply shapes as `serde` structs — pure data with no I/O, which is why they may cross into the library at all: `src/lsp.rs` builds every outgoing message from them and the edge decides nothing. Taken at **0.94** rather than 0.97 for `Url::from_file_path`: 0.97 swapped `url::Url` for `fluent-uri`'s `Uri`, which has no path constructor, and building a `file://` URI by hand means percent-encoding — a solved problem, and one that is silently wrong for the first path with a space in it. Brings `url` |
| LSP framing | `lsp-server` | F31. `Content-Length` framing over a child's stdio: `Message::read` off a `BufRead` and `Message::write` to a `Write`. rust-analyzer's own crate, **synchronous, and it brings no async runtime** — which is the whole reason it was chosen over `tower-lsp` or `async-lsp`: `tokio` is a dev-dependency here and must stay one. Do **not** hand-roll the framing: a header parser that is subtly wrong desynchronises the stream and every message after it is garbage. `src/rpc.rs` is the only file that names it, the way `pty.rs` is the only file that names `portable-pty` |
| DAP messages and framing | none — `serde_json` and `src/rpc.rs` | F45, ADR 0021. **No crate, on purpose, after looking.** `dap` (0.4.1-alpha, 2023) is written for implementing an adapter: its requests only deserialize and its responses and events only serialize, the opposite of what a client needs. `dap-types` is a 0.0.1 stub. So `src/debug.rs` builds requests with `serde_json::json!` and reads replies and events off `serde_json::Value` — the protocol's shapes are JSON the core decides about, and a `Value` read of the few fields Varde uses is less code than a type per message. The framing is LSP's `Content-Length` header, but `lsp-server` cannot be reused: its `Message` requires a `jsonrpc` field DAP does not have. `rpc::read_frame` is the one hand-rolled parser, under a unit test for the two ways it goes wrong — a length counted in characters rather than bytes, and a header it does not know |
| Finding a command | `which` | F31's R31.23: whether a configured server command is on this machine, probed at the edge when the palette's server list is opened. `which::which(command)` answers for a bare name off `PATH`, for an absolute path, and for Windows' `PATHEXT` — the three cases a hand-rolled `PATH` split gets wrong in the order nobody notices (the executable bit first). Named only in `src/main.rs`: the answer reaches the core as `State::commands_on_path`, which `update` reads and never writes. One small crate, and it locked **no new package at all** on this platform: its only dependency is `libc`, already in the tree |
| Process liveness | `libc` | The Sidecar sweep on start (ADR 0016) deletes every `~/.varde/paths/` directory whose Varde is gone, and *gone* is `kill(pid, 0)` — no signal delivered, only the question asked. `libc` was already in the tree under `which`, so the crate is free; the alternative was shelling out to `kill -0` per candidate directory, which spawns a process to ask a question the syscall answers and reads `EPERM` as death. Named only in `src/main.rs`, in `alive`, which is the one `unsafe` block in Varde: `EPERM` counts as alive, because the one mistake this check exists to avoid is deleting the Sidecar of a running instance. **Unix-only, and the first thing in the tree that is**: `kill` has no Windows spelling, so a port needs its own answer here — Varde already drives a pty and reads `stty`, so this is not where portability is lost |
| Sentence boundaries | `unicode-segmentation` | F35's R35.4. An Utterance is a sentence, and *where a sentence ends* is UAX #29, not `[.!?]`: this repo's prose writes `1.0`, `R3.3`, `e.g.` and colons constantly, and a hand-rolled split makes "It goes to 1." an Utterance and "An IDE TUI:" another — the prototype produced exactly those. `unicode_sentences()` also drops a fragment with nothing to say, so an empty piece never reaches the voice. Named only in `src/reading.rs`, in one pure function under unit test. Already in the lockfile behind `textwrap`, `syntect` and `mermaid-text`, so making it direct downloads nothing — the same reason `unicode-width` and `semver` are direct |
| WAV container | `hound` | F35's R35.4 at the edge. A Reading is one continuous stream, so the synthesizer's per-Utterance files and the silence between them are stitched into a single wav before a player is spawned — a player per Utterance was rejected by ear. The container is where hand-rolling costs: a RIFF header carries the length in two places, and a patched one is wrong in a way nothing in Varde would hear until a player refused the file, on a path **no test covers by design**. `hound` reads and writes 16-bit PCM with **no dependencies of its own** — one locked package — and is named only in `src/main.rs`, in `stitch`, the way `portable-pty` is named only in `pty.rs`. `symphonia` and `rodio` decode and play far more, which is a decoder and an audio device Varde deliberately does not own: the player is a `[speech]` row, per ADR 0013 |
| Finding URLs | `linkify` | R10.9: the URL under a jump-modifier click on a pty's row. Do **not** hand-roll it: where a URL *ends* is the whole problem — a sentence's trailing full stop is not part of it, a `)` closing a `(` inside a Wikipedia path is, and a `)` closing the parenthesis the URL sat in is not. `linkify` balances exactly those; a split on whitespace gets every one of them wrong. Named only in `src/editor.rs`, in `link_at`, which also refuses every scheme but `http` and `https` — the crate finds any scheme, and the operating system's opener would launch whatever a child printed. One package, and it brings only `memchr`, already in the tree |
| CLI args | `clap` | The workspace path argument, F1 |
| Errors | `thiserror` (lib), `anyhow` (bin) | Typed errors in `src/`, context at the edge |
| BDD tests | `cucumber` (cucumber-rs) | Runs the existing `features/*.feature` unchanged |
| Unit tests | built-in `#[cfg(test)]` | No dependency needed |
| Fixture directories | `tempfile` (dev only) | R31.27's edge resolution answers a question about a directory layout — a `typescript/lib` holding `typescript.js` is an SDK and one holding only `tsc.js` is the 7.0 native preview, installed and useless — so the test that pins the difference needs real directories. A **dev-dependency**, and the only place a test touches a filesystem — `main.rs`'s edge tests ask it the same questions the edge asks, and nothing in `src/` reads one. `tempfile::tempdir()` cleans up on drop, including after a panicking test, which is the part a hand-rolled name under `std::env::temp_dir()` gets wrong |

## Where the spec pins the stack

Several crates are load-bearing for a decision already made, not free choices:

- **`crossterm`'s keyboard enhancement flags** are how F6's `Ctrl+Ctrl` is detectable at all. The
  Kitty keyboard protocol is what reports a bare modifier keypress; pushing those flags is the
  implementation of R6.1, and the terminals that reject them are exactly the ones that fall back to
  `Ctrl+Space`.
- **`git2`'s status API** matches R7.1 and R7.2 directly — uncommitted against HEAD, untracked
  included, ignored excluded. Any other choice means reimplementing ignore semantics.
- **`notify` with debouncing** serves F5. Undebounced, a single AI write can fire create+modify and
  produce two reload prompts.
- **`rust-code-analysis`'s parent-space metrics are inclusive**, which is what makes F27's "a closure
  counts toward the Function holding it" true rather than aspirational: the `*_sum()` getters carry
  every child space's contribution, so the conversion reads those and the Function rule stops
  descending at the first function it meets. That was an assumption until a fixture test in
  `src/main.rs` settled it, and the test is what goes red if a future release makes subspace metrics
  exclusive.
- **`vt100` + `portable-pty`** are what make F3 and R10.5 work: the injected command goes into a
  terminal model Varde owns, and `mouse_protocol_mode()` answers whether the child wants the click.

## Quoting: `shlex` matches the spec

Verified empirically against `shlex` 2.0.1 — its output is exactly what the F3 Outline already
specifies, including the apostrophe row:

```
/home/me/projects/varde/src              (bare)
'/home/me/projects/varde/my notes'       (single-quoted)
"/home/me/projects/varde/don't"          (double-quoted)
'/home/me/projects/varde/a;b'            (single-quoted)
```

shlex picks the quoting style per input rather than always single-quoting, which is why it agrees
with the spec. Q34 — a path containing both quote characters — is handled too:
`/tmp/don't"quote.txt` becomes `"/tmp/don't\"quote.txt"`.

Do not hand-roll quoting, and do not unit-test shlex's output: that is testing someone else's code.
The F3 Outline is the contract, and it passes as written.

## Highlighting: scopes, not colours

`syntect` returns Sublime scopes, over the 220-grammar set `two-face` supplies. Three things worth
knowing before touching `src/highlight.rs`:

- `fn`, `let` and `const` come back as **`storage.type`**, not `keyword.*`. They are keywords to a
  reader, so the classifier treats `storage` as a keyword. Testing for `keyword` alone finds nothing.
- A quoted string arrives in three pieces — the opening quote, the text, the closing quote — because
  the quotes also carry `punctuation.definition.string`. Adjacent tokens of the same kind are
  coalesced so a string arrives whole.
- A grammar is looked up by extension first and by *name* second, because a fenced code block in
  Markdown preview has no filename to invent — its info string is all there is. Both paths reach the
  same set; a language neither resolves comes back as one plain token rather than failing.

Scenarios assert the *kind* a piece of text has, never its colour. Colour lives in `src/ui.rs` and
is selected by `editor.theme` through the F9 config layering.

## Testing shape

- Behaviour: `features/*.feature`, run by `cargo test --test cucumber`. Step definitions live in
  `tests/` and call into `src/`.
- Units: `#[cfg(test)] mod tests` beside the code they cover.
- The clock is injected, never real — F6 specifies a 300 ms double-tap window and a 450 ms non-tap,
  and neither should cost wall-clock time in the suite.
- Nothing in the suite spawns a real pty or scrapes ANSI. See AGENTS.md.
