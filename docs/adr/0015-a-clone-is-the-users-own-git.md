# A clone is the user's own git

`:story? <url>` has to fetch a repository CRIME has never seen, over SSH, from whatever host the user
named. Every other git operation in this codebase goes through `git2` — `AGENTS.md` says so, and
until now there was no counterexample, because CRIME had never written to a repository or reached the
network. It has opened repositories, read status, and diffed trees; it has never fetched.

**Cloning and fetching shell out to the user's own `git` binary. Everything else stays `git2`.**

## Why not `git2`

`git2` with default features can speak SSH. What it cannot do is authenticate the way the user's
machine already authenticates. It requires a `RemoteCallbacks` credentials closure written by us, and
that closure authenticates against ssh-agent directly — it does not read `~/.ssh/config`. Every
mechanism a developer has actually configured lives in that file or behind it: host aliases,
`IdentityFile` for a per-host key, `IdentitiesOnly`, `ProxyCommand`, a hardware key, 1Password's or
Secretive's agent, a corporate jump host. A credentials callback that tries ssh-agent and gives up is
correct for the default case and broken for `git@bitbucket.org` with a second key, which is the case
this feature exists to serve.

This is `AGENTS.md`'s own rule — *use the crate, don't write the function* — pointing somewhere
unexpected. The thing that has already found the edge cases in SSH credential resolution is not a
Rust crate. It is `git`, which the user has already configured, and which resolves credentials
correctly for hosts nobody here has tried. Reimplementing that is the most expensive kind of code
this repo recognises: it looks small, and it is wrong in ways no scenario covers.

## Why the shell pane, and not a hidden child

The two `std::process::Command` sites that already exist — the configurable test runner and the
formatter — are hidden children, and CRIME reads their exit status directly. A clone cannot be one of
those. An SSH key with a passphrase, or a host whose fingerprint `git` has not accepted, prompts for
input; a hidden child prompts a terminal nobody can see, and the clone hangs with no way to answer
and nothing on screen.

So the clone is `Effect::RunInTerminal`, in the shell pane, where the user can see progress and type
a passphrase. The cost is that CRIME cannot read a pty reliably enough to know when the command
ended, which is a problem the Refactor loop already solved: its command touches
`.crime/refactor-done` and the file watcher that is already running notices. A clone does the same.

**The sentinel carries the exit status, not merely the fact of finishing.** `git clone …; echo $? >
<done>` — never `&& touch <done>`, which writes nothing when the clone fails and leaves CRIME waiting
forever on a bad URL. A clone that failed is a refusal CRIME says out loud, which is the same rule
`queries::reply` follows: refuse out loud, never by silence.

## Consequences

**`git` becomes a runtime dependency, for one feature.** A machine with `git2` linked in but no `git`
binary can do everything CRIME did before and cannot clone. `which` is already a dependency, so the
absence is detectable and is refused with a message rather than a failure to launch.

**A second thing writes to the shell pane.** The clone command and its output interleave with
whatever the user was doing there. This is visible rather than hidden, which is the point, but it is
not invisible, and no scenario covers the edge so nothing will catch it changing.

**This is not licence to shell out for reads.** Status, diffs, trees, revparse and merge-base stay
`git2`. The distinction that earns the exception is *credentials*: an operation that must authenticate
as the user is an operation the user's own tooling should perform. Nothing else in CRIME
authenticates as anybody.
