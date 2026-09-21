# A Debug adapter is a hosted child, reached three ways

Varde debugs through the Debug Adapter Protocol: one client in Varde, and one Debug adapter per
language doing the language-specific work — codelldb for Rust, js-debug for JavaScript and
TypeScript, java-debug for Java. That is the only design in which "a debugger for many languages" is
a client and a table rather than a debugger per language, and it is the design
`docs/adr/0011-a-language-server-is-a-second-hosted-child.md` already made for language servers.
This ADR records that the argument transfers, and the one place it had to stretch.

**No branch in `src/` names a Debug adapter or the language it serves.** Not `codelldb`, not
`js-debug`, not Java — not by command, not by version, not by sniffing a reply for a string only one
adapter sends. Adapters are `[dap.<language>]` rows in the programs template of
`docs/adr/0018-the-global-config-is-the-list-of-programs.md`, with `install.<os>` keys so Tools can
install them, and a Launch configuration is a named entry in either config layer whose launch or
attach arguments are handed to the adapter untouched. The falsifiable form is ADR-0011's grep,
widened: every hit for an adapter's name in `src/` is in the template, a fixture or a comment.

## Three ways in, because the three languages that matter use all three

ADR-0011's child is spawned and spoken to over stdio. For debugging that covers one of the three
languages this was designed for, so a row says how its adapter is reached, as data:

- **stdio** — spawn `command` with `args`, speak over its standard streams. `lldb-dap` works this
  way.
- **server** — spawn `command` with `args` in which `${port}` is filled in, then connect to that
  port over TCP. codelldb and js-debug work this way.
- **through a language server** — name an `[lsp.*]` row, a plugin to load into that server when it
  starts, and a command to send it; the server answers with a port, and Varde connects to it over
  TCP. Java's adapter works this way: java-debug is not a program but a plugin inside jdtls, because
  mapping a file and line to a class needs the classpath only the language server has.

The third shape looks like the Java special case this repo forbids, and it is not one, because what
it names is a *relationship* any server could have — "this language's language server hosts its
Debug adapter" — and the row says which server, which plugin and which command. A second language
that hosts its adapter the same way is a second row. What would be the forbidden shape is Varde
knowing that `vscode.java.startDebugSession` is the command, and it never does.

Stdio alone was the alternative, and it was rejected on a concrete workflow rather than on
principle: attaching to a local Java service's JDWP port, where requests routed through a proxy
pause at the right line, is the debugging this was built for. Supporting only the shape language
servers already use would have left out the one language whose attach flow was the reason to build a
debugger.

## What carries over from ADR-0011 unchanged

**Only the edge observes the process.** That an adapter is running, that it exited, that its port
never answered — these are told to the core by the edge each pass, never remembered by the core from
having asked for a spawn. `ai_running` is the failure this rule names, and a Debug session is a
third place it could recur.

**Branching on a declared capability is not naming a provider.** An adapter that reports it cannot
set a variable gets a dimmed Chip. The Exception filters shown are the ones it lists. The Evaluator
is offered for hovers only if the adapter says it evaluates for hovers. Each of those reads the
protocol's own answer.

**A missing adapter is a message, never a fallback.** The configured adapter either runs or the user
is told once, by name.

## Consequences

**Adapter bookkeeping stays out of the user's model.** js-debug opens a child session per worker or
subprocess and asks the client to start it. Varde folds each one into the one Debug session, as more
threads. A session picker would be the adapter's structure leaking into the UI, and nothing Java or
Rust does needs one.

**Varde does not make up for a weak adapter.** Rust's evaluation under LLDB reads fields and does
arithmetic but mostly cannot call a method, and RustRover hits the same limit. The Evaluator shows
the adapter's refusal as the adapter gives it. Varde injecting compiled code into a paused process
would be per-language knowledge, and it was declined in the spec. When an adapter improves, Varde
improves with it without a release, which is the benefit ADR-0004 claimed for the same rule.

**The DAP framing is not the LSP framing, though it looks the same.** Both use `Content-Length`
headers, but DAP messages have no `jsonrpc` field, so the language server's message parser cannot
read them. The channel is a second one, not a reuse.

The behaviour this serves is specified in issue #45.
