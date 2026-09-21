# Scratch audio lives outside the workspace

A Reading synthesizes a stream to a file, plays it, and deletes it. That file has to live somewhere,
and every obvious answer is wrong in a way worth recording.

**It lives in `~/.varde/tmp/`, and it is the first thing Varde has ever written to the global
directory.**

## Not in the project's `.varde/`

Everything else Varde writes goes there: `risk.json`, `state.json`, `snapshots/`, `stories/`,
`reviews/`, the seeded `config.toml`. Consistency argues for audio joining them, and consistency is
wrong here for one concrete reason: **the workspace is rendered.** `.varde/` sits in the file tree the
user is looking at, and in `git status`, and in whatever the project search walks. A file that exists
for eleven seconds and then vanishes is, in a rendered tree, a flicker with no explanation — and one
left behind by a crash is a mystery file inside somebody's repository.

The rule underneath is not "audio is special". It is that **`.varde/` holds what belongs to the
project, and a Reading belongs to the listener.** A stream of somebody's voice reading a paragraph is
not an artifact of the codebase; it does not survive a restart, it means nothing to a second person
who clones the repo, and it would be `.gitignore`'d immediately by anyone who noticed it. Anything
that would be ignored on sight does not belong in the workspace.

## Not in the OS temp directory

`std::env::temp_dir()` needs no new directory, no dependency and no decision, and it is the answer
almost every program gives. It is rejected because Varde cannot then clean up after itself with any
confidence. A process killed mid-Reading leaves a file, and the only honest sweep is "delete the
files I recognise in a directory shared with every other program on the machine" — which is a pattern
match against somebody else's filenames, on a directory whose contents Varde has no business
enumerating.

A private directory makes the sweep trivial and safe: **everything in `~/.varde/tmp/` is Varde's, so
everything in it can go.** That is a rule with no exceptions to get wrong, which is the only kind
worth writing.

It is also where the user already looks. `~/.varde/config.toml` is a path they have opened. A
mystery in `~/.varde/tmp/` is a mystery with an obvious owner; the same file under `/var/folders/…`
belongs to nobody.

## This does not reverse 0012

`docs/adr/0012-an-install-command-is-configuration.md` ends with a section titled *Nothing is ever
written to the user's config file*, and that sentence stands verbatim. It is about
`~/.varde/config.toml` — about Varde editing a file a human maintains, clobbering deliberate edits
and freezing what it wrote. None of that applies to a directory of its own making holding files it
alone creates and deletes.

The distinction is worth stating because the shorthand is easy to misremember. **Varde does not write
the user's configuration. Varde may write its own scratch.** The global directory is not one file's
folder; it is Varde's folder, and it happened to contain exactly one read-only file until now.

## Consequences

**The directory is created, not assumed.** `~/.varde/` has only ever been read — `main.rs` opens
`config.toml` there and tolerates its absence. A Reading needs `EnsureDir` on a path outside the
workspace root, which is the first of those, and it is why `AbsPath`'s workspace-root guarantees do
not reach it.

**Deletion happens twice, deliberately.** When the player exits, and again when Varde exits. Two
sites for one file looks redundant and is not: the first is the normal path and the second is the
only one that runs after a Reading was interrupted by quitting. A crash escapes both, which is what
the sweep-on-start exists for — and the sweep is safe precisely because of the rule above.

**Nothing in the workspace changes when Varde speaks.** No file appears in the tree, `git status` is
unchanged, and a project search finds nothing new. This is the assertion the scenario should make,
in the shape `AGENTS.md` asks for: an absence, deliberately checked, because it is what a plausible
implementation quietly breaks.
