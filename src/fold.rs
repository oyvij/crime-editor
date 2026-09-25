//! Folding a block of code away.
//!
//! A block is derived from the indentation the file already carries rather than
//! from a brace matcher: a matcher has to be written per language and is wrong
//! in the languages nobody tried, while indentation is what every one of them
//! agrees on — and the same rule folds Python, YAML and Rust. The grammar is
//! not asked either: [`crate::highlight`] answers what *kind* a piece of text
//! is, which is a question about colour and not about structure.
//!
//! What is folded is state on the buffer; which lines that hides is derived
//! here, so no arm has to keep the two in step.

use crate::editor::Buffer;
use crate::{Place, State};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Bound, RangeBounds};

/// A foldable block: the line that opens it and the last line it covers, both
/// 1-based. The opening line stays on screen while the block is folded — it is
/// the line the toggle sits on, and the line the cursor is put on so it never
/// waits somewhere nobody can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub from: usize,
    pub to: usize,
}

/// Which way the toggle in a line's gutter is pointing. A state and not a
/// glyph, for the reason no scenario asserts a colour: what it is drawn as is
/// the renderer's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Open,
    Folded,
}

/// Every block `source` holds, outermost first at each level.
///
/// A line opens a block when the lines under it are indented deeper than it is,
/// and the block runs to the last of them. A blank line belongs to whatever
/// follows it rather than ending the block, which is what keeps a paragraph
/// break inside a function inside the fold; trailing blanks fall outside it,
/// because a fold that swallowed the gap before the next function would hide a
/// line nobody thinks of as part of this one.
pub fn blocks(source: &str) -> Vec<Block> {
    let indents: Vec<Option<usize>> = source.split('\n').map(indent).collect();
    let mut blocks = Vec::new();
    for (index, indent) in indents.iter().enumerate() {
        let Some(indent) = indent else { continue };
        let mut last = None;
        for (below, under) in indents.iter().enumerate().skip(index + 1) {
            match under {
                None => continue,
                Some(deeper) if deeper > indent => last = Some(below + 1),
                Some(_) => break,
            }
        }
        if let Some(to) = last {
            blocks.push(Block {
                from: index + 1,
                to,
            });
        }
    }
    blocks
}

/// A line's indentation in characters, or nothing for a line with nothing on
/// it: a blank line opens no block and ends none.
fn indent(line: &str) -> Option<usize> {
    (!line.trim().is_empty()).then(|| line.chars().take_while(|c| c.is_whitespace()).count())
}

/// `:toggle` — the block the cursor is in, folded or opened — and `:toggle!`,
/// which answers for the file: every block at once, or every one of them open
/// again if any is folded. "Open again" is the second press of one command
/// rather than a command of its own, which is what the ticket asked for and
/// what makes the state recoverable with the keys already typed.
pub fn toggle(buffer: &mut Buffer, all: bool) {
    let blocks = blocks(buffer.shown());
    if all {
        buffer.folded = match buffer.folded.is_empty() {
            true => blocks.iter().map(|block| block.from).collect(),
            false => Vec::new(),
        };
        return;
    }
    // The innermost block the cursor is inside: the largest opening line at or
    // above it that still covers it. A `:toggle` in a nested block means the
    // block it is *in*, not the function around it.
    let Some(block) = blocks
        .iter()
        .filter(|block| block.from <= buffer.line && buffer.line <= block.to)
        .max_by_key(|block| block.from)
    else {
        return;
    };
    if buffer.folded.contains(&block.from) {
        buffer.folded.retain(|folded| *folded != block.from);
        return;
    }
    buffer.folded.push(block.from);
    // Onto the line the fold left on screen: a cursor inside what was just
    // hidden is a caret that is not drawn at all, and the next key typed lands
    // out of sight.
    buffer.go_to_place(Place {
        line: block.from,
        column: 1,
    });
}

/// The lines the editor does not draw, 1-based: the body of every folded block
/// in the buffer in front of you. The one answer everything deriving a row from
/// a line reads — [`crate::story::rows`] and the caret, the scroll clamp and
/// the mouse hit-test behind it — so a click under a fold lands where it points.
pub fn hidden(state: &State) -> BTreeSet<usize> {
    let Some(buffer) = folding(state).filter(|buffer| !buffer.folded.is_empty()) else {
        return BTreeSet::new();
    };
    blocks(buffer.shown())
        .iter()
        .filter(|block| buffer.folded.contains(&block.from))
        .flat_map(|block| block.from + 1..=block.to)
        .collect()
}

/// The toggle each line in `lines` that opens a block carries, keyed by that
/// line. A line that opens nothing carries none: an affordance on every line is
/// an affordance nobody reads.
///
/// Read off the lines themselves rather than off [`blocks`]: a line opens a
/// block exactly when the first line under it with anything on it is indented
/// deeper, so the editor's window is answered without laying out every block
/// in the file on every frame (#101).
pub fn toggles(state: &State, lines: impl RangeBounds<usize>) -> BTreeMap<usize, Toggle> {
    let mut toggles = BTreeMap::new();
    let Some(buffer) = folding(state) else {
        return toggles;
    };
    let from = (lines.start_bound().cloned(), Bound::Unbounded);
    let mut opening: Option<(usize, usize)> = None;
    for (number, line) in crate::lines_within(buffer.shown(), from) {
        let Some(depth) = indent(line) else {
            continue;
        };
        if let Some((opener, _)) = opening.take().filter(|(_, above)| depth > *above) {
            let toggle = match buffer.folded.contains(&opener) {
                true => Toggle::Folded,
                false => Toggle::Open,
            };
            toggles.insert(opener, toggle);
        }
        if !lines.contains(&number) {
            break;
        }
        opening = Some((number, depth));
    }
    toggles
}

/// The buffer whose folds are showing, or nothing.
///
/// Only source is foldable: a Preview's rows are not its lines and a diff and a
/// walked Site draw over the pane entirely, so a fold taken into account there
/// would hide a row of something else. The state survives — leaving Source and
/// coming back shows the folds as they were.
fn folding(state: &State) -> Option<&Buffer> {
    match state.diff.is_some() || state.walking.is_some() || crate::previewing(state) {
        true => None,
        false => crate::current_buffer(state),
    }
}

#[cfg(test)]
mod tests {
    use super::{blocks, Block};

    fn spans(source: &str) -> Vec<(usize, usize)> {
        blocks(source)
            .into_iter()
            .map(|Block { from, to }| (from, to))
            .collect()
    }

    /// A brace language: the body is the block, and the closing brace is not in
    /// it — it is indented back out, which is what says the block ended.
    #[test]
    fn a_function_body_is_a_block() {
        assert_eq!(spans("fn main() {\n    go();\n    stop();\n}\n"), [(1, 3)]);
    }

    /// The same rule, with nothing but indentation to go on. No language is
    /// named anywhere here, which is the point of deriving it this way.
    #[test]
    fn an_indented_language_folds_by_the_same_rule() {
        assert_eq!(spans("def f():\n    go()\n    stop()\n"), [(1, 3)]);
    }

    #[test]
    fn a_nested_block_is_its_own_block_inside_the_one_around_it() {
        assert_eq!(
            spans("fn main() {\n    if ok {\n        go();\n    }\n}\n"),
            [(1, 4), (2, 3)]
        );
    }

    /// A blank line inside a block stays inside it; the ones before the next
    /// block do not, or folding one function would swallow the gap under it.
    #[test]
    fn a_blank_line_inside_a_block_belongs_to_it_and_a_trailing_one_does_not() {
        assert_eq!(
            spans("fn one() {\n    go();\n\n    stop();\n}\n\nfn two() {\n    go();\n}\n"),
            [(1, 4), (7, 8)]
        );
    }

    #[test]
    fn lines_at_one_depth_hold_no_block() {
        assert_eq!(spans("use std::fs;\nuse std::io;\n"), []);
    }
}
