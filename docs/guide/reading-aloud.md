# Reading aloud

A **Reading** is the Selection spoken by a synthesizer. Select a passage in a markdown file, type
`:read`, and a voice reads it while a mark in the editor follows along. The point is not to replace
reading but to sit alongside it: a long document is easy to zone out of, and a voice sets a pace the
eye can follow.

A Reading covers the Selection and nothing else. There is no reading from the cursor, no reading of
a whole file and no fallback to the paragraph under the cursor: select, play, and when it ends select
the next passage. That is deliberate — the passage worth hearing is the one you picked.

Nothing ships with a voice built in. The synthesizer is an ordinary program on your `PATH`
(`piper` by default), the voice is a model file on your disk, and the sound leaves through an
ordinary player (`afplay` on macOS, `aplay` on Linux). See [Installing](#installing-a-voice) below
and [../install.md](../install.md).

## What can be read

| Condition | If it does not hold |
|---|---|
| The current buffer is a markdown file | Refused: `not-markdown`. Nothing else is read aloud, and the Transport is not drawn on other files. |
| There is a Selection | Refused: `nothing-selected`. |
| `speech.voice` names a model file that is on disk | Refused: `no-voice`, and Tools opens on the speech synthesizer row — `i` installs it, the same as taking it in Tools. |
| The synthesizer is running | Refused: `no-synthesizer`, same offer. |
| A player is configured and on `PATH` | Refused: `no-player`, and Tools opens on the player row. Nothing installs a player. |

What counts as a Selection is the same thing copying copies: a mouse drag, or a keyboard extend
with Shift and an arrow. Two cases catch people out:

- **A project search leaves its hit as the Selection.** `Ctrl+F`, pick a hit, `:read` — and the voice
  says the matched word, not the passage around it. Select the passage first.
- **A Preview forms no linewise selection.** On the rendered surface a markdown file opens in, a drag
  or a Shift-extend picks rendered rows and can be read; `V` and Shift+Down do nothing there. To
  select whole lines, `:preview` to cross to Source first.

What the voice receives is the passage as prose, not as markdown source: no `#`, no `*`, no link
URL — a heading is read as its words, a link as its text. This is the same stripping the Preview
draws, so Preview and Source read identically. Code blocks are **not** skipped: a Reading that
wanders into one is stopped by hand.

## Starting, pausing, skipping, stopping

Every control is a `:` command, and every one also sits on the **Transport** — the strip of controls
on the editor's top border, drawn only over a markdown buffer, clickable. The cheatsheet lists them
on one row: `:read :pause :next :prev :stop :speed`. See [editing.md](editing.md) for the rest of
the editor's keys.

| Command | Does |
|---|---|
| `:read` | Starts a Reading of the Selection. Starting one while another is in flight **replaces** it — nothing queues. |
| `:pause` | Toggles: pauses a Reading in flight, resumes a paused one from exactly where it stopped. On the Transport this is the play/pause control; pressed with no Reading in flight it starts one over the Selection, so "select, play" is the whole gesture. |
| `:next` | Skips to the next Utterance. Past the last one, the Reading ends. |
| `:prev` | Goes back one Utterance. At the first, it stays there — you hear it again. |
| `:stop` | Ends the Reading. Nothing is marked and no sound plays. |
| `:speed <n>` | Sets the pace for the **next** Reading (see below). |

An **Utterance** is one sentence-sized run of speech — what `:next` and `:prev` move by, and what
the mark on screen names. Sentences are cut the Unicode way, so a colon does not end one and neither
does the `.` in `1.0` or `e.g.`.

### The pauses you hear

A Reading is built as one continuous stream with real silence in it, not as a player per sentence.
The gaps are part of the passage as spoken:

| Between | Silence |
|---|---|
| Two sentences in the same block | 550 ms |
| The last sentence of a block and the next block | 1000 ms |

A block is what the Preview separates — a paragraph, a heading, a code fence — and each list item
besides, which nothing separates on screen but a reader hears apart.

### The mark

While a Reading plays, the Utterance being spoken is marked in the editor: a marker in the gutter
and a dim wash behind the lines, never an inversion. It follows the sound rather than the last key
you pressed — skipping moves it because skipping moves the sound — and it is drawn on the buffer the
passage came from, so switching to another file does not put the mark on the wrong text. When the
Reading ends, nothing is marked.

### Speed

`speed` is a multiplier and higher is faster — `1.0` is the voice's own pace, `1.25` a quarter
faster, `0.9` a little slower. `:speed <n>` takes any positive number; zero, negatives and things
that are not numbers are refused rather than clamped.

A speed change **applies to the next Reading**, not the one playing. Pace is baked in when the
stream is built, and re-pacing a stream mid-sentence would repeat words you had just heard. The
Transport's speed control steps a ladder instead — `0.75`, `1.0`, `1.25`, `1.5`, `2.0`, wrapping
back to the bottom — and `:speed` is how a pace between its rungs is reached. The value you set is
what `speech.speed` in the config means, so a preferred pace can be written down once.

## When nothing is installed

Reading does nothing until a synthesizer and a voice exist on the machine, and it says so rather
than staying silent — silence is also what success sounds like before the first word. Each missing
piece refuses by name (`no-voice`, `no-synthesizer`, `no-player`), and the install command for your
operating system is **typed onto the terminal's input line and not run**. Read it, edit it if your
package manager differs, then press Enter yourself. Nothing is executed, downloaded or written by
Varde; a row with no install command for this OS refuses and offers nothing.

The order of the checks is the order you fix things in: the voice first, because a synthesizer with
no model to load never starts.

### Installing a voice

A voice is two things: the **synthesizer binary** and a **model file** it loads. Take the speech
synthesizer row in Tools (or press `i` when a Reading is refused for a missing piece): the shipped
install command runs in the shell pane, installs `piper` with `uv`, and fetches the
`en_US-bryce-medium` model — an `.onnx` file and its `.onnx.json` beside it, about 61 MB, public
domain — into `~/.varde/voices/`. When it exits 0, Varde writes the row's `configures.voice` into
`~/.varde/config.toml`:

```toml
# ~/.varde/config.toml
[speech]
voice = "~/.varde/voices/en_US-bryce-medium.onnx"
```

`voice` ships blank on purpose. It is a path on your disk, and an invented one would be a row that
reads as configured and cannot work. The install fills it in, and only when it is blank: a voice you
chose is never overwritten. A `~` at its start is your home directory. A `voice` naming a file that
is not there reads `no-voice`, exactly as a blank one does.

Any other piper voice works the same way: download its `.onnx` and `.onnx.json` and point `voice`
at the `.onnx`. Any other synthesizer that reads text on stdin and writes its audio where `${dir}` says can be
named in `speech.command` and `speech.args` — Varde never checks which one it is.

The synthesizer stays resident once started (it is started when the first markdown buffer opens,
because loading a voice takes long enough to miss the first word otherwise) and holds a couple of
hundred megabytes while it lives.

## The `[speech]` table

All of these live in `~/.varde/config.toml` or the project's `.varde/config.toml`; see
[configuration.md](configuration.md) for how the two layer. The seeded project file lists
`command`, `args`, `voice` and `speed` commented out.

| Key | Shipped default | Meaning |
|---|---|---|
| `command` | `"piper"` | The synthesizer, found on `PATH`. |
| `args` | `["--model", "${voice}", "--length-scale", "${scale}", "--noise-w-scale", "1.0", "--output_dir", "${dir}"]` | Its arguments. `${voice}` is the `voice` row, `${scale}` the reciprocal of `speed` (the synthesizer scales duration, so it runs backwards — you never write the inverted number), `${dir}` where the stream is written. `--noise-w-scale 1.0` was chosen by ear over the model's 0.8. |
| `voice` | `""` | Path to the model file; a leading `~` is your home directory. Blank until the install fills it in. |
| `speed` | `1.0` | Multiplier, higher is faster. What `:speed` changes for the session. |
| `player.macos` | `"afplay"` | What plays the stream on macOS. |
| `player.linux` | `"aplay"` | What plays it on Linux (`alsa-utils`). |
| `player.windows` | — | None shipped: nothing there plays a wav from a command line without a shell of its own. |
| `install.macos`, `install.linux` | `uv tool install piper-tts && mkdir -p ~/.varde/voices && curl …` | Run in the shell pane when the speech row is taken in Tools. |
| `configures.voice` | `"~/.varde/voices/en_US-bryce-medium.onnx"` | Written into `voice` once the install exits 0, if `voice` is blank or absent in `~/.varde/config.toml`. |
| `install.windows` | — | None shipped. |

## Where the audio goes

A Reading writes its stream to `~/.varde/tmp/` — outside every workspace — plays it, and deletes
it. Nothing appears in the file tree, `git status` is unchanged and a project search finds nothing
new. The file is removed when the player exits and again when Varde exits, and the directory is
swept on the next start in case a crash escaped both; everything under it is Varde's, so all of it
can go. It is the one thing Varde writes to `~/.varde/` that is not configuration or a review.

The reasoning, if you want it: `docs/adr/0013-a-voice-is-an-installed-binary.md` for why the voice
is a program you install rather than a library Varde links, and
`docs/adr/0014-scratch-audio-lives-outside-the-workspace.md` for why the stream is not in the
project's `.varde/` or the OS temp directory.
