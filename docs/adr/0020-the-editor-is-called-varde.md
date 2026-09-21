# The editor is called Varde

CRIME was an acronym — Command · Review · Integrated · Modal · Editor — and the word it spelled was
doing work nobody asked it to do. A tool whose name has to be explained before it can be
recommended is a tool that does not get recommended, and "I run my reviews through CRIME" is a
sentence with a cost attached in every room it is said in. The acronym was also the only reason the
name was all-caps, and the only reason `Cargo.toml`'s `description` was a list of five words rather
than a sentence about what the program does.

**The editor is called Varde. A *varde* is a stone cairn somebody stacked to mark the way for
whoever comes next — which is what a review, a story and a test gate are.** The binary, the crate,
the state directories, the release assets and the name the terminal answers to are all `varde`;
prose says `Varde`; the XTVERSION reply says `VARDE(x.y.z)`, uppercase because that is the
convention of the sequence and not because anything is spelled out.

## Why this word and not a better-known one

Two candidates were rejected for reasons worth keeping, because both will be proposed again.

*Lumo* — light, short, pronounceable — is Proton's AI assistant, launched and marketed into the
same corner of the world this editor lives in. *Los* — a maritime pilot who boards a ship and
guides it through hazardous water — is a real word for exactly this product's job, and is unusable:
an English reader sees "loss", and cannot say it aloud without being corrected.

The third requirement was one only this repository has. A rename here is 3,454 case-insensitive
hits across 123 files, and the check that it finished is `grep -rIi <name>` returning only what was
left on purpose. That check is worth something only if the new name does not otherwise occur in
prose about editors, tests and reviews — which rules out `spark`, `beacon`, `lark` and every other
comfortable English word. `varde` had zero hits in this tree before the rename, and will have
exactly as many as the rename puts there.

## The release lineage ends at 0.161.1

An installed CRIME asks `api.github.com/repos/oyvij/crime-editor/releases/latest` for an asset named
`crime-<os>-<arch>`. After the rename GitHub still redirects the URL, but that asset no longer
exists — so an old binary finds no Update, reports nothing, and stays where it is forever.

**Nothing bridges this, deliberately.** Uploading `crime-*` assets alongside `varde-*` for a few
releases was specified and then dropped: it permanently doubles
`asset_names_match_the_release_workflow`, adds a delete-me-later step to `release.yml` that nobody
is scheduled to delete, and protects an install base of one person who knows about the rename
because they performed it. Varde starts at **0.162.0** and CRIME's last release is **0.161.1**, with
no version in between that can update into the other.

Not 1.0.0, which the "major for a breaking change" rule would otherwise produce: 0.x already says
nothing is stable, and 1.0.0 additionally promises that the config format, the state layout and the
key bindings will be held. That promise is worth making on purpose, not as a side effect of typing
a different word 3,454 times.

## Migration is the installer's job, and `src/` has none

`install.sh` copies `~/.crime/config.toml` to `~/.varde/config.toml`, rewriting the string
`~/.crime` inside it — without which `configures.voice` and the piper rows point at a directory
that no longer exists and the voice fails with no message. It moves a `~/.crime/src` checkout to
`~/.varde/src` and repoints its remote rather than deleting it, because that directory is a git
repository that may hold unpushed commits and the installer has no undo. Then, and only after the
copy succeeded, it removes the old `crime` binary and `~/.crime/`. The step runs for both install
kinds: a source install never downloads an asset, so it cannot live in the binary branch.

**No migration code exists in `src/`** — no module, no `Effect`, no scenario, no field. Two designs
that would have put it there were specified in full and rejected:

- *Rename a project's `.crime/` on open.* This writes into the user's repository on every start,
  unasked. If the directory is tracked, it appears as a large unstaged rename in somebody else's
  working tree; if it is ignored, the `.gitignore` rule has to be rewritten too, and `lib.rs`
  already documents that a project's `.crime/` is the user's. For a Bare workspace it is worse than
  untidy: `docs/adr/0016-a-bare-workspace-leaves-nothing-behind.md` promises Varde writes nothing
  into a folder it was only lent, and this would be the first thing it ever wrote there.
- *Read `.crime/` when `.varde/` is absent.* Cheaper and safer — no writes at all — but it is a
  compatibility branch in the library, with its own scenarios, and the repository's own history says
  such a branch is never deleted. It would outlive everyone who remembers what CRIME was.

**A project's `.crime/` is therefore untouched and unmentioned by the code.** Varde creates
`.varde/` beside it and ignores it completely; the old directory is the user's to delete, and the
release notes say so in one line. Anyone reading this later and reaching for a fallback: the cost
was measured, and the answer was no.
