# A binary install updates itself from a Release; a checkout install still does not touch the network

ADR 0003 settled how CRIME notices it is out of date: compare the Version its checkout claims with
the Running version, one file read, no git, no network. `features/self_update.feature` states the
same in its preamble — *no network, no git, no watcher and no timer* — and ADR 0011 says CRIME
never downloads a language server. Three sentences that read as one rule: CRIME does not fetch.

This decision adds a second install kind without touching that rule for the first. A **checkout
install** — the binary is `target/release/crime` under a manifest naming crime — keeps every word
of ADR 0003. A **binary install** — anything else, which today offers nothing and is described by
the feature file as "a copied binary with nothing above it" — asks this repository for its latest
Release once at startup, compares that Release's Version to the Running version by
the same `semver` ordering, and if it is strictly newer, `:update` downloads this platform's asset,
verifies its checksum, replaces the binary on disk and relaunches.

## Why the binary install may fetch when the checkout install may not

ADR 0003's argument against git was not an argument against the network as such. It was that the
*comparison* should be a pure function over two strings so a scenario can hold it, and that the
edge should not have to run a git operation to learn the checkout's state. Both survive here: the
edge performs one request and hands back a body it does not read, and the core parses, compares and
chooses — every decision is in `update` with a scenario against it. What the checkout install has
that the binary install lacks is a second source of truth on disk. Without one, the only place a
newer Version can be read is where the Releases are.

ADR 0011's argument was different again: a bootstrapper for *servers* needs a table of where each
one comes from, and that table is the provider-specific branch this repo forbids. CRIME fetching
*itself* needs no table — one URL, one asset name built from the OS and CPU it already knows — and
names no third party's product.

## Why one request at startup and nothing periodic

The self-update feature's "no timer" stays. A Release appears when the author publishes one, which
is on the order of days; a session is on the order of hours. A check per launch sees every Release
within a day of it, and a check that fails — offline, rate-limited — shows nothing and is logged, because a nag about the network is worse than a marker
a day late. The same request on a timer would buy nothing and wake the network while somebody is
typing.

## Why the relaunch is the quit

CRIME already has a Restart event, offered after a server install that only edits the shell
profile, and it *is* Quit: an unsaved buffer refuses it with `unsaved-changes`. A relaunch after an
update is held to the same refusal for the same reason — an update that could discard work is a way
around the one refusal quitting makes. The only difference is the last effect: the edge replaces its
own process with the new binary instead of exiting. The shell and the AI pane end with the old
process, as they do on quit; nothing about the session is carried across.

## Consequences

The feature file's preamble is amended to say which install kind the "no network" sentence is about.

Two edge tools appear: `curl`, already required by `install.sh`, for the request and the download;
and the `sha2` crate for the checksum, chosen over `shasum`/`sha256sum` because those are two names
for one tool on two platforms, which is the branch this repo refuses.

The repository's latest-release URL is a constant in the core and the only place the release host
is named.

A binary install that is replaced but refuses to relaunch is running a binary that is no longer on
disk. That is fine on Unix — the process holds the old inode — and the next `:update` relaunches
without fetching. The state that remembers this dies with the process, which is correct: the next
process is the new binary and starts its own check.
