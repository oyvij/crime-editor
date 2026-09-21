# A voice is an installed binary, not a linked library

Varde reads a selection aloud. The synthesis could happen *inside* Varde — `piper-rs` turns text into
samples, `rodio` plays them, and nothing has to exist on the machine beforehand. That is the obvious
design and it is rejected here.

**A voice is an external command Varde runs, named in configuration, probed on `PATH`, and installed
by a command Varde types and does not press Enter on.** Audio leaves Varde through a second such
command. Neither is ever named in a branch.

## What the linked version actually costs

`piper-rs` depends on `ort` pinned at `=2.0.0-rc.12` — a release candidate of the ONNX Runtime
bindings — and on `espeak-rs`, which builds a C library. Playback needs `rodio`, which needs `cpal`,
which needs CoreAudio and ALSA. Today this repo has **no audio dependency and no network dependency
at all**; `Cargo.toml` is TUI, git, LSP and parsing. The linked design adds a native audio stack and a
pre-release native inference runtime in one step, to a repo whose contract says prefer one well-known
crate and justify each one in the commit.

It also has to answer a question the binary version never asks: the voice model is 61MB, and nothing
in `src/` has ever downloaded anything. Linking the synthesizer means either adding the first HTTP
client to the tree or shipping a 61MB file in the binary.

## Why the binary version costs nothing new

`docs/adr/0011-a-language-server-is-a-second-hosted-child.md` and
`docs/adr/0012-an-install-command-is-configuration.md` already built every piece:

```toml
[speech]
command = "piper"
voice = "${fact}"
player = "afplay"
install.macos = "uv tool install piper-tts && …"
install.linux  = "…"
```

A row of TOML in the bottom layer of the merge. `which` finds the command or does not. A missing
command puts the install line on the terminal's input line, unexecuted, where the user reads it
before it runs — which is how the model gets onto the machine without Varde ever making a network
request. The 61MB question dissolves: the command that installs the synthesizer fetches the voice,
in a shell the user controls, with output they can see.

The falsifiable form is 0012's grep in a third shape. **Search `src/` for a synthesizer's name —
`piper`, `espeak`, `say`, `afplay`, `aplay` — and every hit must be inside `DEFAULTS`, a fixture or a
comment.** A hit in an `if` or a `match` is this decision reversed. This is the same rule
`docs/adr/0004-hosted-panes-are-transparent.md` states for CLI providers, and it is here for the same
reason: the second synthesizer nobody has tried is the one the branch gets wrong.

## What it costs, stated plainly

**Reading does nothing until something is installed.** This is the tax, and 0012's refusal already
pays it: the status line says which command is missing and the install lands on the input line. A key
that vanishes without a word is the failure `src/keys.rs` exists to prevent; a Reading that is silent
because no voice exists must say so out loud, because silence is *also* what success sounds like
before the first word.

**A pause cannot fade.** This is the real cost and it is worth naming, because it is the one thing the
linked version does better. Stopping an external player mid-waveform is abrupt, and `SIGSTOP` is worse
than abrupt — it freezes the process while the audio device drains, so the buffer underruns and
clicks. The answer is that Varde does not stop the sound mid-sample at all: it stops the player and
**resumes by rewriting the stream from the recorded offset**, measured at 7.5ms, which is inaudible
and sample-accurate. A `rodio` sink could fade instead.

So the reversal trigger is specific rather than vague: **if trim-and-resume still clicks audibly on a
machine that matters, this decision is wrong and the linked design is right.** It was measured once,
on one Mac, and found clean.

## Why not the OS voice, which needs no install at all

macOS has `say` and every Linux has `espeak-ng` behind Speech Dispatcher, and both are already there.
They were rejected by ear. The OS voices are formant-synthetic and exhausting over paragraphs, which
defeats the entire purpose: the feature exists because a long document is hard to stay inside, and a
voice that is itself hard to stay inside has bought nothing. A neural voice is the feature; the
install is what it costs.

The voice shipped in `DEFAULTS` is public domain — `en_US-bryce-medium`. The best-sounding candidate
tested, `en_US-ryan-high`, is CC BY-NC-SA 4.0. Varde may be open-sourced, and a non-commercial clause
is cheap to avoid now and expensive to unpick later.

## Consequences

**Two commands, not one.** Synthesis and playback are separate binaries, because no synthesizer
portably plays its own output and `afplay` cannot read a stream from stdin. Both are configuration;
neither is a branch.

**The synthesizer stays resident.** Loading the voice costs ~600ms and synthesis costs ~100ms, so a
process per Reading would miss the one-second budget on the first word of every selection. It is a
third hosted child in the sense 0011 means, started when the first markdown buffer opens rather than
on the first keystroke — because a lazily-started voice pays its load on the press that wanted sound.
It holds ~238MB while it lives.

**A voice Varde cannot find is a normal state, not an error.** Same shape as a language with no
install command for this OS: the row says nothing is configured, and the user is one line of TOML from
fixing it for every future Varde on that machine.
