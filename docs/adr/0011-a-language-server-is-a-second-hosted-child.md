# A language server is a second hosted child

> **Amended by `docs/adr/0018-the-global-config-is-the-list-of-programs.md`:** server rows are no
> longer a bottom layer in `startup::DEFAULTS` but a template written into `~/.crime/config.toml`, and
> `lsp::language` is replaced by an `extensions` key on each row.

CRIME hosts a shell and an AI CLI, and `docs/adr/0004-hosted-panes-are-transparent.md` forbids any
branch anywhere that tests which CLI is running in one. A language server is the same kind of thing:
a child process CRIME spawns, talks a documented protocol to, and must have no opinions about. It
differs from the two hosted panes in one respect only — it has no pane, because its channel is
JSON-RPC over stdio rather than a grid of cells. Everything ADR-0004 argues about a hosted pane
therefore transfers, and this ADR records the transfer rather than restating it.

The rule, stated for servers: **no branch in `src/` names a language server.** Not `rust-analyzer`,
not `tsserver`, not `gopls`, not by command, not by version, not by sniffing its capabilities reply
for a string only one server sends. A server CRIME has never been run against works for the same
reason the others do.

The cost ADR-0004 pays — CRIME's own bindings given up inside a hosted pane — has no analogue here,
because a server owns no keyboard. What this rule costs instead is the workaround: the moment one
server is wrong about something, the fix is in the protocol handling, the document sync or the
staleness rule, and fixing it there fixes it for the servers nobody has tried. That is the same trade
and the same reason.

## Why shipping defaults is not naming a provider

CRIME must work on a fresh install with nothing configured, and that means it must know that Rust is
usually served by `rust-analyzer`. Those two sentences look like they contradict the rule above, and
they do not, because *where* the name lives is the whole of the distinction.

Servers are named in `[lsp.<language>]` tables, and the defaults ship as **TOML data in
`startup::DEFAULTS`** — already the bottom layer of F9's layered merge, already the way
`view.double_tap_ms` and `risk.threshold` get their values. The alternative is a match arm:

```rust
// forbidden
match language {
    Language::Rust => "rust-analyzer",
    Language::TypeScript => "typescript-language-server",
    _ => return,
}
```

Both spell the same string. Three things separate them, and each one is a property of the merge layer
rather than a matter of taste:

**Overridable.** A default in the bottom layer is beaten by `~/.crime/config.toml`, and that by
`<project>/.crime/config.toml`, key by key, because F9's merge already works that way. A user who
prefers a different server for Rust, or a different one in one repository, changes a file. A match
arm can only be beaten by a fork.

**Inspectable.** The defaults are a string a user can read, and F9's effective-setting machinery can
print what CRIME actually resolved. A match arm's answer is visible only by reading CRIME's source
and reasoning about which arm fired.

**Extensible without a release.** A language nobody at CRIME has heard of is served by adding four
lines of TOML. Under a match arm it is served by a CRIME release, which is exactly the "a newly bound
provider feature needs no CRIME release" consequence ADR-0004 claims.

So the falsifiable form of the rule is a grep: search `src/` for any server's command name, and
every hit must be inside the `DEFAULTS` string or a test fixture. A hit in an `if`, a `match`, or a
comparison is the defect this ADR exists to name. That is a build-time-checkable statement about
where a string lives, which is why it was chosen over the honest-sounding but unenforceable "don't
special-case servers".

The rule is about *identity*, not about behaviour: branching on a **capability the server declared in
its own initialize reply** is not naming it. A server that says it does not do hover is asked for no
hover, and that branch reads the protocol's own answer rather than guessing from a name. The
distinction is the same one ADR-0004 draws when it answers a child's queries: what the child told us
is data; what we assume about the child from its name is a provider branch.

Two adjacent decisions are settled by the same argument, so they are recorded here rather than
discovered later:

**CRIME never installs, downloads, updates or bootstraps a server.** It runs what is configured. A
bootstrapper needs a table of where each server comes from and how it is built, which is the match
arm above wearing a package manager's clothes, and it would make CRIME's behaviour a function of a
network it cannot see.

**A missing binary is a message, never a fallback to a different server.** "rust-analyzer was not
found, trying rls" is a provider preference expressed in control flow. The configured command either
runs or it does not, and if it does not the user is told once, by name, and the editor carries on.

## What the core may know, and what only the edge may observe

`AGENTS.md` states the rule and names the failure: `ai_running` was core state, set when a spawn was
*asked for* and cleared by an event, so a spawn that failed cleared nothing. The core then routed
keys to a pane the edge did not hold and refused `:ai` because a session was "already running" — a
dead pane with no way back. **A `State` field the core sets and the edge also observes has two
authors and will diverge.** A language server is a second child process, so the field that field was
is now available in a second shape, and the shape is the same mistake.

The split, for servers:

**Only the edge may observe** — that a process exists, that it is still alive, that it exited, that
its command was not found on `PATH`, that its stdin closed. Every one of those is a fact about an
operating-system object the core has never held a handle to. They reach the core the way the hosted
panes' facts do: written by `tell_core` in `main.rs` from what the edge actually holds, each pass,
before `update` runs. `update` reads them and **never writes them**.

**The core may hold** — everything derived from bytes that arrived: whether the handshake has
completed for a language, what the server declared it can do, the diagnostics published per path, the
identifier of every request sent and not yet answered, the document version last sent for a buffer,
and which reply the popup on screen came from. None of these is a claim about a process. Each one is a
claim about a conversation, and the core is the only party to that conversation, so it is the only
place they can live without two authors.

**A conversation begins against a process, never against the asking for one.** `sync` returns
`Effect::StartLsp` and writes nothing down; the edge spawns, comes to hold the child, and says so —
`tell_core` puts the language in `lsp_running` and `Event::LspStarted` brings a pass around to read
it. Only then is `initialize` sent and the `Conversation` created. Recording the conversation at the
moment the spawn was *asked for* is `ai_running`'s shape exactly, and it worked only because effects
happen to be executed in the order they are returned — an unstated coupling between a rule in `src/`
and a loop in `main.rs`. Reading the fact instead of remembering the request also means a server the
edge holds for any other reason is spoken to rather than ignored, which is what lets a Scenario say
"a server is already running" without a spawn to point at.

The seam between them is `Effect::LspSend` and `Event::LspReceived`: the core decides what to say and
the edge decides nothing at all, in either direction. Which is the same division `mouse::report` and
`queries::reply` already draw — bytes are the library's to build, because a decision in `main.rs` is a
decision without a test.

`ai_running` also showed what a derived field cannot carry: the *consequence* of losing a session. A
review queued for an AI session must not be handed to the next one, so every site that stops holding
a pane queues `AiExited`. The same consequence exists here and is larger, because a server death
invalidates state the core is holding on screen: the diagnostics published for that language, and
every request sent and not answered. So a server that stops is told, and the telling is what clears
them — not a timeout, and not the next request noticing that nothing came back. A diagnostic left in
the gutter by a dead server is the gutter making a claim about the present out of a conversation that
ended.

## Consequences

**A workspace with no server configured for its language is not a degraded workspace.** It spawns
nothing and behaves exactly as CRIME does today. That is the falsifiable form of "adding this feature
takes nothing away", and it is asserted as an absence in `features/language_intelligence.feature`,
the way this suite asserts every other promise not to do something.

**One server per language, not per buffer.** A language is what a server serves; opening a second
Rust file joins the conversation already running. This is a consequence of the core keying its
conversation state by language, not a performance optimisation.

**Nothing a server says is trusted about the present.** Every reply carries either a document version
or a request identifier, and the core drops what does not match what it is holding — the stale-version
drop. A server is a correct participant in a conversation that has moved on; it is not wrong to answer
a question late. It is CRIME that would be wrong to draw the answer.

**The server sees the buffer, never the disk.** The same rule writing already follows. A server told
about the file on disk answers questions about a file the user is not looking at, and every position
in its reply is then off by whatever has been typed.

*Except where there is no buffer.* Review view asks a question about files nobody has opened — whether
the change under review carries errors — and the only text those files have is the text on disk. So a
changed file is synced read-only: the edge reads it (`Effect::ReadForReview`, because the core reads
no files), the server is told it is open at one fixed Document version, and it is told nothing about
it ever again until the file stops being part of the change — at which point it is told closed, and
what it said about the file is dropped rather than kept. A Buffer's marks survive being switched away
from, because the file is still the file; a file put back to what HEAD holds is a file those marks
describe a version of that no longer exists. That does not weaken the rule above, because the two never describe the same file: a
file with a Buffer is synced from the Buffer, and this covers only the files without one. Nothing is
spawned for them — telling a running server about a document and starting a process are different
acts, and only the second is a side effect nobody asked for.

**No provider-specific code, anywhere — extended to a second kind of child.** A future reader will
find a language whose server needs a quirk, and will want one arm for it. That is this decision,
reversed. The quirk belongs in the protocol handling, or in `[lsp.<language>]`, or nowhere.

**One file, several servers — and which ones is data.** `lsp::language` gave a path exactly one
language, which is one server, which made a `.vue` file unfixable in principle rather than in
practice: it is served by the Vue server *and* by a TypeScript one. The same shape is a linter server
beside a type server, and every arrangement other editors reach by attaching several clients to one
buffer. So `[lsp.<language>].also_served_by` names the other languages whose servers also serve this
language's files, and one keystroke becomes one question put to all of them.

Named from the *file's* side rather than the serving server's, which was the alternative: a server
declaring the extensions it fancies reads more naturally, but resolving a path would then mean
scanning every configured server and asking each one, and the thing that needs two answers is the
file. From this side it is one lookup in the table the path already found.

The arbitration is then core state and a pure decision over replies — the first non-empty answer
wins, the empty-handed notice waits for the last server, and a question whose last outstanding server
dies is answered by nobody. It is written once over the `About` a key asked rather than once per key,
because a hover and a definition that arbitrate differently is a difference nobody could predict from
the keys.

**And what one server says about a file is not what the other says.** Two clients on one buffer means
two publishers of diagnostics, so `State::diagnostics` is keyed by path *and by the language that
spoke*: a push is one server's whole current opinion, and keyed by path alone the second publisher
erased the first. It is also what lets a server that dies take exactly its own marks, rather than
every mark on every file it served — which is how a Vue server exiting came to take a TypeScript
server's type errors with it.

The shipped `.vue` row is the case this section was written for, and it needed one thing beyond the
mechanism: the TypeScript server answers about a `.vue` file only when it is loaded with the plugin
Volar ships, which is a path — so it is a `[facts.*]` search, declared `optional` because it is named
on the row every TypeScript project shares
(`docs/adr/0012-an-install-command-is-configuration.md`). No arm names either server, which is the
falsifiable form: grep `src/` and every hit is in `DEFAULTS`, a fixture or a comment.
