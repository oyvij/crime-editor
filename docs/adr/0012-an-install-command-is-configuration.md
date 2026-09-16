# An install command is configuration, not a branch

`docs/adr/0011-a-language-server-is-a-second-hosted-child.md` states, flatly: **CRIME never
installs, downloads, updates or bootstraps a server. It runs what is configured.** This decision
amends that sentence. It does not reverse the argument behind it, because the argument was never
about installing — it was about CRIME *knowing how* to install.

Read 0011's own reasoning back:

> A bootstrapper needs a table of where each server comes from and how it is built, which is the
> match arm above wearing a package manager's clothes, and it would make CRIME's behaviour a
> function of a network it cannot see.

Two costs, and neither one is "a server got installed". The first is the table. The second is the
invisible network. 0011 then spends a section establishing that a server's *name* can be shipped
without paying either cost, provided it is **data in the bottom layer of the merge** rather than a
match arm — overridable by a file, inspectable as a string, extensible without a release.

An install command is the same kind of string, and the same three properties are available to it:

```toml
[lsp.zig]
command = "zls"
install.macos = "brew install zls"
```

So the decision is the smallest one that closes the gap: **an install command is one more key in the
`[lsp.<language>]` table, shipped as TOML data, and CRIME never composes it.** The forbidden thing
is unchanged and now stated in a form a grep can check: search `src/` for a package manager's name —
`brew`, `npm`, `pip`, `apt`, `cargo`, `rustup`, `go`, `winget` — and every hit must be inside
`DEFAULTS` or a fixture, exactly as 0011 already requires for the servers themselves. One grep, two
classes of name.

## An initialization option is the same key, for the same three reasons

A server also has to be told things about its own world before it will work — most often where its
toolchain lives. `typescript-language-server` resolves TypeScript itself, finds nothing usable on a
normal machine, and then **never answers `initialize` at all** unless it is handed
`initializationOptions.tsserver.path` pointing at a classic `tsserver.js`. That is not an exotic case:
it is one of the languages `DEFAULTS` already names.

So `[lsp.<language>]` carries an `initialization_options` table too, and the argument above is the
whole of the argument for it — overridable by a file, inspectable as a string, extensible without a
release. A `match` on language would spell the same values while giving up all three, and it would be
worse than the command case rather than equal to it: an arm naming a server is one string, an arm
naming a server's *toolchain layout* is a string plus an assumption about a directory on somebody
else's disk.

The falsifiable form is the same grep in a second shape. **Nothing in `src/` reads inside the
table.** It is deserialized straight out of the TOML into the JSON map the message carries and
handed to `InitializeParams::initialization_options` untouched — serde does not care which format a
value came from, so there is no conversion at all and certainly no walk deciding what each shape
means. Re-serialising a `toml::Table` into JSON later was the first shape and is worse for one
concrete reason: TOML has `nan` and JSON has no number for it, so the conversion is fallible, and its
one failure mode would surface as a panic in a running TUI rather than as R9.5's file and line.
Search `src/` for any option's own key
name — `tsserver`, `tsdk`, `procMacro`, `plugins` — and every hit must be in `DEFAULTS`, a fixture or
a comment.
A hit in an `if` or a `match` is this decision reversed.

**And an option may be conditional on nothing but a fact.** The plugin that lets a TypeScript server
answer about a `.vue` file is one more entry in this table, naming a path a `[facts.*]` search finds —
but it is named on the `[lsp.typescript]` row *every* TypeScript project shares, and a machine that
never installed a Vue server has no such path. Making that a requirement would take the TypeScript
server away in every project on that machine, so the fact declares itself `optional` and the key is
dropped when it is not found. The alternative — sending the option only while a `.vue` file is open —
was rejected here rather than in passing: it would make the option a function of the buffer set,
which means either restarting a server when a second language appears or reading the table to decide
when it applies, and the second is this section's promise reversed. One boolean on the *fact*, read
only by the spawn gate, keeps the option opaque.

This is not the rule `src/lsp.rs` states for *capabilities* ("CRIME asks for nothing yet, so it claims
nothing"), and the distinction is worth keeping. A capability is a claim about what CRIME can do, and
announcing one before the code that reads it invites a server to send what nobody looks at. An
initialization option is a claim about nothing: it is a value going the other way, and the party it
describes is the server.

**And it ships blank — a *name*, never a path.** The values these keys want are paths inside the
workspace — `node_modules/typescript/lib` — so `DEFAULTS` carries none of them literally, for the
reason the `install` table carries no `clangd` command for macOS. What it does ship is the
`${fact}` a `[facts.*]` search resolves at every spawn, which is the same refusal one layer up: a
declared search is not a path on somebody else's disk. An invented path that does not exist is worse than an honest
blank, because a server that fails on a configured-looking value reads as CRIME's bug rather than as a
row to fix. The same applies to `[lsp.vue]`, whose `args` want `--tsdk=<dir>`: the flag is correct and
the directory is machine-specific, so the arg is not shipped half-finished either. A path on this
machine is the edge's to find and tell the core, which is R31.23's split and a later decision.

## The command is typed, never run

CRIME types the install command into the terminal pane and does not press Enter — `SetTerminalInput`,
not `RunInTerminal`. The tree's actions already work this way, and the reason transfers exactly: an
install is a command with consequences on a machine CRIME does not own, and the person who owns it is
sitting in front of the pane.

This is also what answers 0011's second cost without any machinery. The network is not invisible: the
request is made by a command the user read first, in a shell they control, whose output lands where
they can see it. There is nothing for CRIME to interpret, which matters more than it looks —
`docs/adr/0004-hosted-panes-are-transparent.md` forbids reading what a pane prints, so a design that
needed to know whether `brew` succeeded would have had to break that rule or guess. This design needs
neither: **what changes CRIME's behaviour is the probe finding the command**, on a later pass, and a
probe is a fact rather than a claim.

A consequence worth stating, because it is a feature and reads like a limitation: a default install
command that is wrong for this machine is **editable in place**. It is sitting on the terminal's input
line, and a Debian default on Fedora is one word away from being right. A command CRIME had already
executed would instead be a failure to diagnose.

## Per-OS, because a command is not portable and a branch is still forbidden

The install command varies by operating system where the server's name does not, so the key is a
small table — `install.macos`, `install.linux`, `install.windows` — and which one applies is selected
by the OS the binary was compiled for. That arrives the way `running_version` already arrives: a
field on `Startup`, handed in by `main.rs` from `std::env::consts::OS`, rather than read in `src/`.
Not because a compile-time constant would be a side effect, but because a value handed in is a value a
scenario can set, and "the Linux row shows the Linux command" is otherwise unspecifiable on a Mac.

**A language with no install command for this OS is a normal row, not an error.** Several servers are
genuinely not packaged anywhere — `zls` on Linux is a tarball or a build from source, `jdtls` is not
in most distributions — and inventing a plausible command for them would be worse than admitting the
gap. The row says there is nothing configured, and the user is one line of TOML away from fixing it
for themselves and every future CRIME on that machine.

## Nothing is ever written to the user's config file

The install commands ship **inside the binary**, as `startup::DEFAULTS`, which is already the bottom
layer of F9's merge. This is the whole of "the commands are added automatically when CRIME is
installed or updated": a new version carries new and corrected commands, they are live on first run,
and no file was touched.

Writing them into `~/.crime/config.toml` instead was considered and rejected, and each reason is
independent:

**It would clobber deliberate edits.** A merge of shipped defaults into a file a human has been
editing is a merge with no conflict resolution and no undo, on the one file a broken copy of which
locks the user out of CRIME (0011's sibling concern: `ConfigError` exists to make that recoverable).

**It would freeze what it wrote.** A command written to disk at install time is a command later CRIME
versions can never improve, because the user's file now beats the defaults — by design, key by key.
The feature would work perfectly once and then rot, silently, in the direction of looking configured.

The sharpest form of this is a *path*, and it was measured rather than imagined. `@vue/typescript-plugin`
sits inside the globally installed `@vue/language-server`, which on a machine using a Node version
manager lives under a directory naming the version:
`…/fnm/node-versions/v22.19.0/lib/node_modules/@vue/language-server/node_modules/@vue/typescript-plugin`.
A setup pass that resolved that path once and wrote it into the project's config would be correct
until the next `node` upgrade and then point at a directory that no longer exists — a server failing
on a configured-looking value, which is the failure mode two sections up. R31.27's declared fact
resolves the same path **at every spawn**, from the file being served upward and then from the command
on `PATH`, so an upgrade is invisible and a monorepo package that installs its own copy gets its own
answer. **That is what "understand the project" is: a search shipped in the binary, not a finding
written into a folder.** A written answer is a fact with no way to notice it went stale.

**An update touching a user's config is a new class of act.** F30's self-update replaces a binary. A
self-update that also edits configuration is a migration, and migrations need versioning, backups and
a way to decline. That is a large feature to buy something the merge already provides for free.

## Rejected alternative: asking the AI session

The first draft of this ADR had the palette hand the install to the hosted AI session — a prompt
naming the language and the command, and no package manager anywhere. It pays the same two costs
correctly, and it covers the unpackaged long tail that `install` keys cannot: a CLI standing on the
machine can work out what `zls` needs on this distribution.

It is rejected as the *mechanism* for three reasons. It makes an editor dependency require a running,
authenticated AI session, which is a large tax on a small feature. It is non-deterministic — the same
row installs differently on two runs, and possibly with a `sudo` nobody asked for. And it puts an
unreviewable act between the keystroke and the machine, where this design puts a command on a line
the user reads.

It remains the obvious fallback if the `install` keys prove too thin in practice, and the two compose
cleanly: a row with no install command for this OS is exactly the case the AI session is good at. That
is a later decision, and it is not blocked by this one.

## Amendment: the first configured command CRIME runs against your source

Everything above is about a command CRIME *types*. `:format` (F32) is the first time CRIME **executes**
a command a workspace's own configuration named, against the text on screen, with no keystroke between
the decision and the process. That is a different act from the install command, and the difference is
worth naming rather than discovering.

It is acceptable here for the reason `docs/adr/0010-crime-owns-the-test-gate.md` already accepted the
larger version of it: CRIME runs the project's tests, out of the project's own config, and a test
suite is arbitrary code with far more reach than a formatter. What makes both acceptable is not the
size of the command but who asked for it. A formatter runs because a reader typed `:format`, over the
buffer they are looking at, and what it is handed is that buffer's text on stdin and — where a row
asked for it — the file's *name* (`${file}`, which is what lets one `prettier` tell JSON from YAML).
It is never handed the file's contents and CRIME writes no file, so a row configured to rewrite the
file in place formats stale bytes nobody is looking at and prints nothing back, which R32.11 refuses
out loud: the shape is unsupported rather than prevented. The command is split
with `shlex` and executed directly, never through a shell, so nothing from the workspace is
interpolated into one.

The install half stays typed, and the asymmetry is the point. An install changes the machine; a format
changes a buffer that `u` puts back. An install is a network request whose output the reader has to be
able to read, in a shell they control, which is the whole of how this design answers 0011's
invisible-network cost without machinery. And an install command that is wrong for this machine is
editable in place on the input line, where a formatter that is wrong is one `u` and one line of TOML.
So the two live in one table, one is run and one is offered, and each is the smaller act of its kind.

## Consequences

**Nothing installs without a keystroke, and nothing runs without a second one.** CRIME does not offer
to install a server because a file was opened, and does not press Enter for you.

**A command that appears is a reason to forget that it was missing.** A failed spawn writes a `Gone`
conversation and `lsp::sync` skips any language that has one — deliberately, so that opening a second
file of that language does not re-run a binary that is not there, and consequently the language is
written off for the session. So the probe reporting a command CRIME holds a `Gone` conversation for
**drops the conversation**, and the next pass spawns it exactly as a fresh start would. No restart, no
retry loop, no timer.

One case survives: an installer that appends its directory to a shell profile is invisible to a
process that inherited its environment at launch. It is distinguishable from the outside — the user
re-checks and the probe still finds nothing — and a restart is then **offered, never taken**.

**A workspace with nothing installed is still not a degraded workspace.** 0011's consequence holds
verbatim.
