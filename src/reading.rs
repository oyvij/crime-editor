//! F35 — reading a Selection aloud.
//!
//! Decides *which words* and *at what pace*, and nothing else: the synthesizer
//! child, the player child and the stream file are the edge's, reached only
//! through [`Effect::Speak`] and [`Effect::StopSpeaking`]. No name of a
//! synthesizer, a voice or a player appears here — they are `[speech]` rows in
//! `startup::PROGRAMS` (`docs/adr/0013-a-voice-is-an-installed-binary.md`).

use crate::{preview, Effect, Selection, State};
use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;

/// A Reading in flight — the Utterances the voice was handed, in order, and
/// where the sound has got to in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub utterances: Vec<Utterance>,
    /// Where each Utterance begins in the stream, in milliseconds, told with
    /// [`crate::Event::Speaking`]: how long a voice takes to say something is
    /// not in its text, so it is the one thing about a Reading the core cannot
    /// work out for itself. Empty until the stream exists, which is why a seek
    /// asked for before then is [`Seek::Nowhere`] rather than a guess.
    pub offsets: Vec<u32>,
    /// Where the sound is within the whole Reading. It is the pause offset
    /// too: pausing is what stops it moving, and resuming rewrites the stream
    /// from here rather than from the start (R35.6).
    pub at_ms: u32,
    /// Paused, which is a consequence of a key and so the core's — unlike
    /// whether a player is still running, which only the edge can see.
    pub paused: bool,
    /// Which buffer the passage was read off, so the mark is drawn on the
    /// text it belongs to and not on whatever file is open when the voice
    /// gets there. `None` for a Selection with no lines to anchor to — one
    /// read off a screen grid has rows — which plays and marks nothing.
    pub file: Option<PathBuf>,
}

/// Where an ask for another Utterance lands. Three answers rather than an
/// `Option<u32>`, because nowhere-to-go means two different things: forward
/// past the last Utterance is the end of the Selection and ends the Reading,
/// back from the first one leaves it exactly where it is — and a stream that
/// is not built yet is neither of those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    To(u32),
    Ended,
    Nowhere,
}

impl Reading {
    /// Which Utterance the sound is inside, counting from zero. Derived rather
    /// than stored: the mark, the seek and the position report would otherwise
    /// be three authors of one fact, and R35.12 says the mark follows elapsed
    /// time against the offsets.
    pub fn at(&self) -> usize {
        self.offsets
            .partition_point(|start| *start <= self.at_ms)
            .saturating_sub(1)
    }

    pub fn forward(&self) -> Seek {
        if self.offsets.is_empty() {
            return Seek::Nowhere;
        }
        match self.offsets.get(self.at() + 1) {
            Some(start) => Seek::To(*start),
            None => Seek::Ended,
        }
    }

    /// Back one, and the first Utterance's previous is its own start: R35.6
    /// says previous moves one Utterance, and there is no Utterance before the
    /// first — so it is re-heard rather than the Reading being ended by a key
    /// that means "again".
    pub fn back(&self) -> Seek {
        match self.offsets.get(self.at().saturating_sub(1)) {
            Some(start) => Seek::To(*start),
            None => Seek::Nowhere,
        }
    }
}

/// Which buffer lines the Utterance being spoken covers, ends included and
/// counting from one — the mark R35.12 asks for, as a span rather than a
/// drawing, since `ui` draws it and no scenario asserts a colour.
///
/// Derived from [`Reading::at`], which is elapsed time against the offsets:
/// the mark follows the voice rather than the last keypress, and a skip moves
/// it because a skip moves the sound.
pub fn mark(state: &State) -> Option<(usize, usize)> {
    let reading = state.reading.as_ref()?;
    if reading.file.as_deref()? != state.current_buffer.as_deref()? {
        return None;
    }
    Some(reading.utterances.get(reading.at())?.lines)
}

/// One sentence-sized run of speech, and the silence that follows it. The
/// silence is *part of the artifact*, not an accident of scheduling: a player
/// per Utterance was built and rejected by ear, because it left a gap the
/// machine chose (~150ms of process spawn) exactly where the listener needed
/// one the writer chose (R35.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    pub text: String,
    /// Milliseconds of silence after it, zero on the last — nothing follows it
    /// to be separated from.
    pub gap_ms: u32,
    /// The lines it was written on, ends included: relative to the passage as
    /// [`utterances`] returns them, and shifted to the buffer's own numbering
    /// by [`start`], which is the only place that knows where the passage sat.
    pub lines: (usize, usize),
}

/// Everything a run of Utterances says, as one string. The Utterances are the
/// artifact and this is a projection of them, so what the core holds and what
/// reached the voice are compared without a second split to disagree with.
pub fn words(utterances: &[Utterance]) -> String {
    utterances
        .iter()
        .map(|one| one.text.as_str())
        .collect::<Vec<&str>>()
        .join(" ")
}

/// How long the silence is, chosen by ear on the prototype and recorded in
/// `.scratch/reading-aloud/prototype/README.md`. A paragraph gets longer than a
/// sentence because that is the difference the reader is listening for: where
/// one thought ends rather than where one sentence does.
pub const SENTENCE_GAP_MS: u32 = 550;
pub const PARAGRAPH_GAP_MS: u32 = 1000;

/// What a `[speech]` row names, resolved for the OS this binary was built for.
/// `player` and `install` are per-OS tables in the file and one string by the
/// time they are here, looked up under the name `Startup` carried in rather
/// than branched on (R31.22).
///
/// The core reads `voice` only for whether it is empty and `install` only to
/// run it; `command`, `args` and `player` it never reads at all — they are the
/// edge's copy of the same rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Speech {
    pub command: String,
    pub args: Vec<String>,
    pub voice: String,
    pub speed: f32,
    pub player: String,
    pub install: String,
}

/// The file `voice` names, with a leading `~` as the home directory: the value
/// is written as the row spells it and expanded only here, where it is read.
/// A blank voice names no file.
pub fn voice_file(voice: &str, home: &Path) -> Option<PathBuf> {
    if voice.is_empty() {
        return None;
    }
    Some(match Path::new(voice).strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => PathBuf::from(voice),
    })
}

/// The duration scale a synthesizer is handed for a speed.
///
/// The one place the inversion happens. `speed` is a multiplier where higher is
/// faster, which is what every player calls it and what a human writes in the
/// config file; the synthesizer's parameter scales *duration* and so runs
/// backwards (R35.7). A speed of zero would be a division by it, and is the
/// number a hand-edited config can hold — it reads as "stopped", which a
/// Reading has no meaning for, so it is the default pace rather than an
/// infinity handed to a child.
pub fn duration_scale(speed: f32) -> f32 {
    if speed > 0.0 {
        1.0 / speed
    } else {
        1.0
    }
}

/// Why a Reading was refused, out loud, and the Tools speech row that would
/// fix it when the refusal is about what is installed.
pub type Refused = (&'static str, Option<&'static str>);

/// Start a Reading over the Selection, or say out loud why not.
///
/// The order is the request before the machine: whether there is a passage to
/// read at all is about what the reader did, and whether anything can say it is
/// about what is installed — so a reader with nothing selected is told that
/// rather than being sent to install a synthesizer they would then still have
/// no use for.
pub fn start(state: &State) -> Result<(Reading, Vec<Effect>), Refused> {
    let Some(path) = state.current_buffer.as_ref() else {
        return Err(("nothing-selected", None));
    };
    if !preview::is_markdown(path) {
        return Err(("not-markdown", None));
    }
    let Some(source) = state.selected_text() else {
        return Err(("nothing-selected", None));
    };
    if let Some(missing) = missing(state) {
        return Err(missing);
    }
    // Where the passage sits in the buffer, so the mark lands on the lines the
    // Utterances were written on rather than on the passage's own numbering.
    // A Selection read off a screen grid has rows and anchors nothing.
    let anchor = match state.selection.as_ref() {
        Some(Selection::Lines { from, .. }) => Some(*from),
        Some(charwise @ Selection::Buffer { .. }) => {
            charwise.buffer_span().map(|(from, _)| from.line)
        }
        Some(Selection::Screen { .. }) | None => None,
    };
    let mut utterances = utterances(&source);
    if let Some(first) = anchor {
        for one in &mut utterances {
            one.lines = (one.lines.0 + first - 1, one.lines.1 + first - 1);
        }
    }
    Ok((
        Reading {
            utterances: utterances.clone(),
            offsets: Vec::new(),
            at_ms: 0,
            paused: false,
            file: anchor.is_some().then(|| path.clone()),
        },
        vec![Effect::Speak {
            utterances,
            speed: state.speech.speed,
        }],
    ))
}

/// Which piece a Reading needs this machine has not got, and the row that
/// installs it: the voice rides on the synthesizer's install.
///
/// The voice first, because nothing else can be observed without it: a
/// synthesizer with no model to load never starts, so asking whether one is
/// running would report the wrong missing piece to whoever has not filled the
/// row in yet. A voice named and not on disk is the same missing piece as one
/// never named.
fn missing(state: &State) -> Option<Refused> {
    if !state.voice_installed {
        return Some(("no-voice", Some("synthesizer")));
    }
    if !state.voice_running {
        return Some(("no-synthesizer", Some("synthesizer")));
    }
    if !state.player_installed {
        return Some(("no-player", Some("player")));
    }
    None
}

/// The Selection's markdown as the Utterances a voice receives, one per
/// sentence.
///
/// [`preview::rows`] and nothing else, at zero columns so nothing is wrapped:
/// Preview and Source then strip through one implementation, and a `#` that
/// leaks has one place to leak from (R35.2). Every block kind is kept,
/// including code — R35.2a says skipping fences was specified and removed.
///
/// A row is a line on screen and a sentence can span several, so a block's
/// rows are joined with a space before it is cut into sentences. The blank rows
/// [`preview`] puts between blocks carry no pieces, and they are what a
/// paragraph boundary *is* here — which is why the longer gap can be the real
/// one rather than the prototype's guess at it from sentence length.
///
/// The cut is [`unicode_segmentation`]'s, so a colon does not end an Utterance
/// and neither does the `.` in `1.0`, `R3.3` or `e.g.` — the prototype's
/// hand-rolled version produced fragments like "An IDE TUI:", and this repo's
/// prose is full of both.
pub fn utterances(source: &str) -> Vec<Utterance> {
    let mut spoken = Vec::new();
    for block in blocks(source) {
        spoken.extend(block.text.unicode_sentences().map(|sentence| Utterance {
            text: sentence.trim().to_string(),
            gap_ms: SENTENCE_GAP_MS,
            lines: (block.from, block.to),
        }));
        if let Some(last) = spoken.last_mut() {
            last.gap_ms = PARAGRAPH_GAP_MS;
        }
    }
    if let Some(last) = spoken.last_mut() {
        last.gap_ms = 0;
    }
    spoken
}

/// The Preview's rows gathered back into blocks, one string each.
///
/// A row with no pieces is the separator [`preview::rows`] puts between blocks.
/// A list item is the other boundary, and it needs naming because there is no
/// separator between two of them — `close_frame`'s "two list rows never
/// separate" is a layout decision about a blank line on screen, and hearing it
/// as one block reads a three-item list as one breathless run with no pause
/// anywhere in it. The row that *starts* an item carries a marker; a wrapped
/// continuation row of the same item does not, and stays where it is.
fn blocks(source: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut fresh = true;
    for row in preview::rows(source, 0) {
        if row.pieces.is_empty() {
            fresh = true;
            continue;
        }
        if fresh || starts_an_item(&row) {
            blocks.push(Block {
                text: String::new(),
                from: row.line,
                to: row.line,
            });
            fresh = false;
        }
        let last = blocks.last_mut().expect("a block to put the row in");
        if !last.text.is_empty() {
            last.text.push(' ');
        }
        last.text.push_str(&row.text());
    }
    blocks.retain(|block| !block.text.trim().is_empty());
    let lines: Vec<&str> = source.lines().collect();
    for index in 0..blocks.len() {
        let next = blocks
            .get(index + 1)
            .map_or(lines.len(), |block| block.from - 1);
        let mut to = next.max(blocks[index].from);
        // The blank line between two blocks belongs to neither, and a mark
        // that took it in would dim a row with nothing on it. A line past the
        // end is trimmed the same way rather than indexed: a passage ending in
        // a newline has one more line start than it has lines.
        while to > blocks[index].from && lines.get(to - 1).is_none_or(|line| line.trim().is_empty())
        {
            to -= 1;
        }
        blocks[index].to = to;
    }
    blocks
}

/// One block's rows as the string the sentence split runs over, and the source
/// lines it was written on.
///
/// The span is the *block's*, not the sentence's: [`preview::rows`] gives one
/// unwrapped row per block and the line it starts on, so where inside a
/// wrapped paragraph a sentence sits is not a fact this returns — a mark on a
/// paragraph is what it can say honestly. Sentence-precise marking needs
/// `preview` to carry a source offset per piece, which is a change to a module
/// six other features read.
struct Block {
    text: String,
    from: usize,
    to: usize,
}

fn starts_an_item(row: &preview::Row) -> bool {
    matches!(
        row.kind,
        preview::RowKind::List(preview::ListItem {
            marker: Some(_),
            ..
        })
    )
}

// ---- R35.8: the Transport ----

/// The Transport's controls. Named once, because the core routes them, the
/// mouse hit-tests them and the border draws them, and three spellings of one
/// string is three places for it to drift.
pub const PREVIOUS: &str = "reading-previous";
pub const PLAY_PAUSE: &str = "reading-play-pause";
pub const NEXT: &str = "reading-next";
pub const STOP: &str = "reading-stop";
pub const SPEED: &str = "reading-speed";

/// What the speed control steps through, slowest first. A ladder rather than a
/// free number, because the control is one column wide to press: the free
/// number is `:speed 1.4`, and this is the gesture beside it.
pub const SPEEDS: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];

/// The next speed up, wrapping at the top. Compared with a margin, because the
/// stored speed is a float a config file wrote and `1.25` read back is not
/// always `1.25`.
pub fn next_speed(current: f32) -> f32 {
    SPEEDS
        .iter()
        .copied()
        .find(|speed| *speed > current + 0.01)
        .unwrap_or(SPEEDS[0])
}

/// The Chips on the editor's top border, left to right. Read by `ui` to draw
/// them and by `mouse` to hit-test them, so the two cannot disagree about which
/// control is where.
///
/// Empty for anything that cannot be read aloud — absent rather than greyed:
/// a Transport drawn over a buffer nothing can read is a row of controls the
/// reader presses twice before believing it. A diff and a Story are drawn in
/// the editor's rectangle but are not the buffer, so they have no Reading
/// either. Within a Transport that is drawn, previous, next and stop are dimmed
/// while nothing is in flight, since then there is nothing for them to act on.
///
/// Play leads, as continue leads the debugger's, so every Transport opens on
/// the Chip that says whether something is moving.
///
/// The glyphs are deliberately narrow and non-emoji. U+23EE and its family
/// report as narrow to `unicode-width` and are rendered emoji-wide by most
/// terminal fonts, which is a bar one column wider than anything measured it —
/// in the prototype it wrapped off the border entirely.
///
/// Which is also why play and stop are U+25BA and U+25A0 rather than the
/// U+25B6 and U+25FC a glyph picker offers first: those two carry the `Emoji`
/// property, so a terminal that honours it draws them two columns wide, and
/// the pointer-and-square pair is the same size on the screen without it. The
/// small U+25B8/U+25AA they replaced were legible only to somebody who already
/// knew what they were.
pub fn transport(state: &State) -> Vec<crate::Chip> {
    use crate::{Chip, Hue, Tone};
    if state.diff.is_some() || state.walking.is_some() {
        return Vec::new();
    }
    let readable = state
        .current_buffer
        .as_ref()
        .is_some_and(|path| preview::is_markdown(path));
    if !readable {
        return Vec::new();
    }
    let playing = matches!(&state.reading, Some(reading) if !reading.paused);
    let chip = |action, name, glyph: &str, keys, hue, needs_reading: bool| Chip {
        action,
        name,
        glyph: glyph.to_string(),
        keys,
        hue,
        tone: match state.transport_lit == Some(action) {
            true => Tone::Lit,
            false if needs_reading && state.reading.is_none() => Tone::Dimmed,
            false => Tone::Plain,
        },
    };
    vec![
        // The play control says what pressing it does, which is a pause while
        // a Reading plays — and is the whole of "shows whether one is in
        // flight".
        match playing {
            true => chip(PLAY_PAUSE, "pause", "\u{25ae}", ":pause", Hue::Hold, false),
            false => chip(PLAY_PAUSE, "play", "\u{25ba}", ":pause", Hue::Go, false),
        },
        chip(PREVIOUS, "previous", "\u{ab}", ":prev", Hue::Step, true),
        chip(NEXT, "next", "\u{bb}", ":next", Hue::Step, true),
        chip(STOP, "stop", "\u{25a0}", ":stop", Hue::Halt, true),
        Chip {
            glyph: format!("{:.2}x", state.speech.speed),
            ..chip(SPEED, "speed", "", ":speed", Hue::Plain, false)
        },
    ]
}

/// The wav a synthesizer says it wrote, read off one line of its output — the
/// last whitespace-separated token, when that names a wav.
///
/// Not the whole line: a synthesizer that logs rather than prints answers
/// `INFO:…:Wrote /tmp/x.wav`, and taking the line whole hands the edge a path
/// that does not exist. Not the first `.wav` either — a prefix is arbitrary
/// text and may hold anything. Anything else on the line is a log line the
/// caller skips, which is what lets one channel carry both the answer and
/// whatever the child says while starting up. No provider is named: `.wav` is
/// the format `stitch` reads (ADR 0013).
pub fn wrote(line: &str) -> Option<&str> {
    line.split_whitespace()
        .next_back()
        .filter(|token| token.ends_with(".wav"))
}

#[cfg(test)]
mod tests {
    use super::{
        duration_scale, mark, next_speed, transport, utterances, voice_file, words, wrote, Reading,
        Seek, Utterance, NEXT, PARAGRAPH_GAP_MS, PLAY_PAUSE, PREVIOUS, SENTENCE_GAP_MS, SPEED,
        STOP,
    };
    use crate::State;
    use std::path::{Path, PathBuf};

    /// What a Reading says, which is what R35.2's cases are about — the split
    /// into Utterances is the next block of tests down.
    fn spoken(source: &str) -> String {
        words(&utterances(source))
    }

    #[test]
    fn a_heading_is_read_as_the_words_it_contains() {
        assert_eq!(spoken("# Setup"), "Setup");
        assert_eq!(spoken("### Deeper still"), "Deeper still");
    }

    #[test]
    fn emphasis_and_code_spans_lose_their_punctuation() {
        assert_eq!(spoken("Install **now**."), "Install now.");
        assert_eq!(spoken("Run `cargo test` first."), "Run cargo test first.");
    }

    #[test]
    fn a_link_is_read_as_its_text_and_never_its_url() {
        assert_eq!(spoken("See [the guide](http://x.test)."), "See the guide.");
    }

    #[test]
    fn markers_are_not_spoken() {
        assert_eq!(spoken("- one"), "one");
        assert_eq!(spoken("> Careful."), "Careful.");
    }

    /// R35.2a. The fence's own line is not text and the code inside it is.
    #[test]
    fn a_code_block_is_read_like_any_other_block() {
        assert_eq!(
            spoken("Run this:\n\n```sh\ncargo test\n```"),
            "Run this: cargo test"
        );
    }

    /// The blank rows between blocks would otherwise reach the voice as a run
    /// of spaces, which is a pause the text chose rather than one the stream
    /// holds (R35.4).
    #[test]
    fn the_blank_rows_between_blocks_are_not_spoken() {
        assert_eq!(spoken("One.\n\nTwo.\n\nThree."), "One. Two. Three.");
    }

    fn said(source: &str) -> Vec<String> {
        utterances(source)
            .iter()
            .map(|one| one.text.clone())
            .collect()
    }

    fn gaps(source: &str) -> Vec<u32> {
        utterances(source).iter().map(|one| one.gap_ms).collect()
    }

    #[test]
    fn one_utterance_per_sentence_in_order() {
        assert_eq!(
            said("Install it. Then run it. Read the output."),
            ["Install it.", "Then run it.", "Read the output."]
        );
    }

    #[test]
    fn a_question_and_an_exclamation_end_one_too() {
        assert_eq!(
            said("Is it on? Try it! Again."),
            ["Is it on?", "Try it!", "Again."]
        );
    }

    /// The prototype's hand-rolled split produced "An IDE TUI:" as an Utterance
    /// of its own, and this repo's prose is full of colons.
    #[test]
    fn a_colon_does_not_end_an_utterance() {
        assert_eq!(
            said("An IDE TUI: a terminal UI that opens on a folder."),
            ["An IDE TUI: a terminal UI that opens on a folder."]
        );
    }

    /// The other half of the same rule: a `.` that is not a sentence's end.
    /// `unicode-segmentation` knows the difference; a regex on `[.!?]` does
    /// not, and this repo writes `R3.3` and `1.0` constantly.
    #[test]
    fn a_dot_inside_a_number_or_an_abbreviation_does_not_end_one() {
        assert_eq!(
            said("It goes to 1.0 rather than 0.8."),
            ["It goes to 1.0 rather than 0.8."]
        );
        assert_eq!(
            said("R3.3 says paths are absolute."),
            ["R3.3 says paths are absolute."]
        );
    }

    /// A sentence spanning two source lines is one Utterance: a row is a line
    /// on screen, not a sentence.
    #[test]
    fn a_sentence_wrapped_across_rows_is_one_utterance() {
        assert_eq!(
            said("Install it\nand then run it."),
            ["Install it and then run it."]
        );
    }

    /// R35.4. The silence is the artifact's, and the last one has nothing to be
    /// separated from.
    #[test]
    fn silence_between_sentences_and_none_after_the_last() {
        assert_eq!(
            gaps("Install it. Then run it. Read the output."),
            [SENTENCE_GAP_MS, SENTENCE_GAP_MS, 0]
        );
    }

    #[test]
    fn a_paragraph_ends_with_a_longer_silence_than_a_sentence() {
        assert_eq!(
            gaps("One. Two.\n\nThree. Four."),
            [SENTENCE_GAP_MS, PARAGRAPH_GAP_MS, SENTENCE_GAP_MS, 0]
        );
    }

    /// A heading is its own block, so the pause after it is a paragraph's —
    /// which is the pause a heading wants anyway.
    #[test]
    fn a_heading_is_a_block_of_its_own() {
        assert_eq!(
            utterances("# Setup\n\nInstall it."),
            [
                Utterance {
                    text: "Setup".to_string(),
                    gap_ms: PARAGRAPH_GAP_MS,
                    lines: (1, 1)
                },
                Utterance {
                    text: "Install it.".to_string(),
                    gap_ms: 0,
                    lines: (3, 3)
                },
            ]
        );
    }

    /// Nothing separates two list items on screen, but a reader hears them as
    /// separate thoughts — and joined into one block they were read as
    /// "one two three" with no pause anywhere in it.
    #[test]
    fn each_list_item_is_its_own_utterance_with_a_pause_after_it() {
        assert_eq!(said("- one\n- two\n- three"), ["one", "two", "three"]);
        assert_eq!(
            gaps("- one\n- two\n- three"),
            [PARAGRAPH_GAP_MS, PARAGRAPH_GAP_MS, 0]
        );
    }

    /// The other half of it: a wrapped item is still one item. Only the row a
    /// marker starts begins a block.
    #[test]
    fn a_list_item_holding_two_sentences_is_two_utterances_and_one_item() {
        assert_eq!(
            said("- Install it. Then run it.\n- Read the output."),
            ["Install it.", "Then run it.", "Read the output."]
        );
        assert_eq!(
            gaps("- Install it. Then run it.\n- Read the output."),
            [SENTENCE_GAP_MS, PARAGRAPH_GAP_MS, 0]
        );
    }

    fn marked(source: &str) -> Vec<(usize, usize)> {
        utterances(source).iter().map(|one| one.lines).collect()
    }

    /// The mark is a line number and a line number means nothing without the
    /// document it counts in: a Reading left playing while the reader opens
    /// something else would otherwise dim whichever lines happen to be there.
    #[test]
    fn nothing_is_marked_over_a_buffer_the_passage_did_not_come_from() {
        let guide = std::path::PathBuf::from("/w/guide.md");
        let mut state = State {
            current_buffer: Some(guide.clone()),
            reading: Some(Reading {
                file: Some(guide),
                ..playing(0)
            }),
            ..State::default()
        };
        assert_eq!(mark(&state), Some((1, 1)));
        state.current_buffer = Some(std::path::PathBuf::from("/w/other.md"));
        assert_eq!(mark(&state), None);
    }

    /// R35.12's arithmetic: the lines are the passage's own, one-based, and
    /// two sentences on one line are both on it.
    #[test]
    fn an_utterance_carries_the_lines_it_was_written_on() {
        assert_eq!(marked("One.\n\nTwo.\n\nThree."), [(1, 1), (3, 3), (5, 5)]);
        assert_eq!(marked("One. Two."), [(1, 1), (1, 1)]);
    }

    /// A block is what a mark covers, so a paragraph written across two lines
    /// is marked on both — and both its sentences are marked the same, since
    /// a row carries the line it starts on and not one per sentence.
    #[test]
    fn a_paragraph_is_marked_on_every_line_it_was_written_across() {
        assert_eq!(marked("Install it\nand then run it."), [(1, 2)]);
        assert_eq!(marked("Install it\nand run it. Read it."), [(1, 2), (1, 2)]);
    }

    /// The blank line between two blocks belongs to neither: a wrapped list
    /// item followed by a paragraph would otherwise dim the empty row between
    /// them.
    #[test]
    fn the_blank_line_after_a_block_is_not_marked() {
        assert_eq!(
            marked("- one\n- two\n  wrapped\n\nAfter."),
            [(1, 1), (2, 3), (5, 5)]
        );
    }

    /// A Reading with a stream built over three Utterances of a second each,
    /// the sound `at_ms` into it. The offsets are the edge's answer, so they
    /// are stated here rather than derived — that is the whole point of their
    /// being told.
    fn playing(at_ms: u32) -> Reading {
        Reading {
            utterances: utterances("One. Two. Three."),
            offsets: vec![0, 1_550, 3_100],
            at_ms,
            paused: false,
            file: None,
        }
    }

    #[test]
    fn the_current_utterance_is_the_one_the_sound_is_inside() {
        assert_eq!(playing(0).at(), 0);
        assert_eq!(playing(1_549).at(), 0);
        assert_eq!(playing(1_550).at(), 1);
        assert_eq!(playing(9_000).at(), 2);
    }

    /// Before the stream exists there are no offsets, and nothing to be inside
    /// of — a Reading a keypress beat by 200ms is still on its first Utterance.
    #[test]
    fn a_reading_with_no_stream_yet_is_on_its_first_utterance() {
        assert_eq!(playing(0).at(), 0);
        assert_eq!(
            Reading {
                offsets: Vec::new(),
                ..playing(0)
            }
            .at(),
            0
        );
    }

    #[test]
    fn a_voice_is_read_with_the_home_directory_for_its_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(
            voice_file("~/.varde/voices/v.onnx", home),
            Some(PathBuf::from("/home/me/.varde/voices/v.onnx"))
        );
        assert_eq!(
            voice_file("/voices/v.onnx", home),
            Some(PathBuf::from("/voices/v.onnx"))
        );
        assert_eq!(
            voice_file("~other/v.onnx", home),
            Some(PathBuf::from("~other/v.onnx"))
        );
        assert_eq!(voice_file("", home), None);
    }

    #[test]
    fn forward_and_back_land_on_an_utterance_start() {
        assert_eq!(playing(200).forward(), Seek::To(1_550));
        assert_eq!(playing(1_600).back(), Seek::To(0));
        assert_eq!(playing(3_100).back(), Seek::To(1_550));
    }

    /// R35.6 and R35.1 in one: back from the first re-hears it, forward from
    /// the last is the end of the Selection and not the start of it again.
    #[test]
    fn back_at_the_first_stays_and_forward_at_the_last_ends() {
        assert_eq!(playing(200).back(), Seek::To(0));
        assert_eq!(playing(3_500).forward(), Seek::Ended);
    }

    /// A seek before the stream exists has nowhere to go, which is not the
    /// same as the Reading being over: ending it would make a next pressed
    /// during the build a Reading that stopped for no reason.
    #[test]
    fn a_seek_with_no_stream_yet_goes_nowhere() {
        let building = Reading {
            offsets: Vec::new(),
            ..playing(0)
        };
        assert_eq!(building.forward(), Seek::Nowhere);
        assert_eq!(building.back(), Seek::Nowhere);
    }

    /// R35.7. The stored number is the multiplier and the voice gets its
    /// reciprocal, so a config nobody sanitises cannot divide by zero.
    #[test]
    fn the_duration_scale_is_the_reciprocal_of_the_speed() {
        assert!((duration_scale(1.00) - 1.00).abs() < 0.005);
        assert!((duration_scale(1.25) - 0.80).abs() < 0.005);
        assert!((duration_scale(0.90) - 1.11).abs() < 0.005);
        assert_eq!(duration_scale(0.0), 1.0);
    }

    /// R35.8. Absent, not greyed: a control that is drawn and refuses is one
    /// the reader presses twice before believing it.
    #[test]
    fn only_a_markdown_buffer_has_a_transport() {
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            ..State::default()
        };
        assert_eq!(names(&state), [PLAY_PAUSE, PREVIOUS, NEXT, STOP, SPEED]);
        state.current_buffer = Some(std::path::PathBuf::from("/w/main.rs"));
        assert_eq!(transport(&state), []);
        state.current_buffer = None;
        assert_eq!(transport(&state), []);
    }

    /// The play control says what pressing it does, which is the whole of what
    /// "shows whether a Reading is in flight" buys.
    #[test]
    fn the_play_control_turns_into_a_pause_while_a_reading_plays() {
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            ..State::default()
        };
        let glyph = |state: &State| transport(state)[0].glyph.clone();
        assert_eq!(glyph(&state), "\u{25ba}");
        state.reading = Some(Reading {
            utterances: utterances("One."),
            offsets: Vec::new(),
            at_ms: 0,
            paused: false,
            file: None,
        });
        assert_eq!(glyph(&state), "\u{25ae}");
        state.reading.as_mut().expect("a reading").paused = true;
        assert_eq!(glyph(&state), "\u{25ba}");
    }

    /// The speed control is the one that carries a value, so the value is what
    /// it says.
    #[test]
    fn the_speed_control_says_the_speed() {
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            ..State::default()
        };
        state.speech.speed = 1.25;
        assert_eq!(transport(&state)[4].glyph, "1.25x");
    }

    /// Wrapping at the top, and stepping off a rung a config file wrote —
    /// `1.25` read back out of a float is not always `1.25`, and a comparison
    /// without a margin would stick there.
    #[test]
    fn the_speed_control_steps_the_ladder_and_wraps() {
        assert_eq!(next_speed(1.0), 1.25);
        assert_eq!(next_speed(1.2499999), 1.5);
        assert_eq!(next_speed(2.0), 0.75);
        assert_eq!(next_speed(3.0), 0.75);
    }

    fn names(state: &State) -> Vec<&'static str> {
        transport(state)
            .into_iter()
            .map(|chip| chip.action)
            .collect()
    }

    fn tones(state: &State) -> Vec<crate::Tone> {
        transport(state).into_iter().map(|chip| chip.tone).collect()
    }

    /// Previous, next and stop act on a Reading in flight, so with none they
    /// are dimmed — never hidden, so the Transport keeps one shape.
    #[test]
    fn with_nothing_in_flight_only_play_and_speed_are_available() {
        use crate::Tone::{Dimmed, Plain};
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            ..State::default()
        };
        assert_eq!(tones(&state), [Plain, Dimmed, Dimmed, Dimmed, Plain]);
        state.reading = Some(Reading {
            utterances: utterances("One."),
            offsets: Vec::new(),
            at_ms: 0,
            paused: true,
            file: None,
        });
        assert_eq!(tones(&state), [Plain; 5]);
    }

    /// Lit is the last action taken and nothing else — no timer, and it
    /// outranks dimmed, so a stop that ended the Reading still says it did.
    #[test]
    fn the_last_action_taken_is_lit() {
        use crate::Tone::{Dimmed, Lit, Plain};
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            transport_lit: Some(STOP),
            ..State::default()
        };
        assert_eq!(tones(&state), [Plain, Dimmed, Dimmed, Lit, Plain]);
        state.transport_lit = Some(PLAY_PAUSE);
        assert_eq!(tones(&state), [Lit, Dimmed, Dimmed, Dimmed, Plain]);
    }

    /// One cell wide in every font, or every click right of the glyph lands a
    /// column off (ADR 0022). `unicode-width` alone cannot say so — U+23EE is
    /// narrow to it and emoji-wide on screen — so each glyph is also held to
    /// ASCII, Latin-1, or the Geometric Shapes that carry no `Emoji` property.
    #[test]
    fn every_glyph_is_one_cell_in_every_font() {
        use unicode_width::UnicodeWidthChar;
        const EMOJI: [char; 8] = [
            '\u{25aa}', '\u{25ab}', '\u{25b6}', '\u{25c0}', '\u{25fb}', '\u{25fc}', '\u{25fd}',
            '\u{25fe}',
        ];
        let mut state = State {
            current_buffer: Some(std::path::PathBuf::from("/w/guide.md")),
            ..State::default()
        };
        let mut glyphs: Vec<String> = transport(&state).into_iter().map(|c| c.glyph).collect();
        state.reading = Some(Reading {
            utterances: utterances("One."),
            offsets: Vec::new(),
            at_ms: 0,
            paused: false,
            file: None,
        });
        glyphs.extend(transport(&state).into_iter().map(|c| c.glyph));
        for glyph in glyphs.iter().flat_map(|glyph| glyph.chars()) {
            assert_eq!(glyph.width(), Some(1), "{glyph:?}");
            let safe = glyph.is_ascii()
                || ('\u{a0}'..='\u{ff}').contains(&glyph)
                || (('\u{25a0}'..='\u{25ff}').contains(&glyph) && !EMOJI.contains(&glyph));
            assert!(safe, "{glyph:?} is not one cell in every font");
        }
    }

    #[test]
    fn nothing_to_say_is_no_utterances() {
        assert_eq!(utterances(""), []);
        assert_eq!(utterances("\n\n"), []);
    }

    /// A synthesizer that logs its answer rather than printing it bare is the
    /// shape that froze a Reading for good: the path was on a line Varde was
    /// not reading, so the wait for it never ended.
    #[test]
    fn the_wav_is_the_last_token_of_the_line_that_names_one() {
        assert_eq!(wrote("/tmp/a/1.wav\n"), Some("/tmp/a/1.wav"));
        assert_eq!(
            wrote("INFO:__main__:Wrote /tmp/a/1.wav\n"),
            Some("/tmp/a/1.wav")
        );
        assert_eq!(wrote("INFO:__main__:Loaded /voices/en_US.onnx"), None);
        assert_eq!(wrote(""), None);
        assert_eq!(wrote("\n"), None);
    }
}
