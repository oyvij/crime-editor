//! F40 — who last committed the line the cursor is on.
//!
//! The blame is the *committed* file's, read by the edge off the commit `HEAD`
//! names and cached there against that commit — never against
//! `Buffer::revision`, which every keystroke bumps and a blame walks history to
//! answer. What the buffer has changed since is the diff's business, which is
//! why a buffer line is traced back through the patch before it names anybody:
//! a line inserted above the cursor would otherwise hand the cursor's line the
//! author of the line above it.

use crate::{State, View};
use std::collections::BTreeMap;

/// Who last committed one line of the file as the commit holds it, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authored {
    pub author: String,
    /// The authored date, `YYYY-MM-DD`. Formatted by the edge, because that is
    /// where the commit's timestamp and its author's offset are read; the core
    /// carries the string it was told.
    pub date: String,
}

/// What the editor's border says about the cursor's line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authorship {
    Committed(Authored),
    /// A line the working tree has changed, and every line of a file the commit
    /// has no copy of. There is no commit to name, not merely none found.
    NotCommittedYet,
}

/// The authorship of the line the cursor is on, or `None` for a border with
/// nothing to say: outside a repository, without git, and in either View that
/// puts its own content in the editor's title. Silence rather than a notice —
/// a border reporting "not a git repository" on every file of a folder that is
/// not one is noise, which is the Change bar's answer to the same question.
pub fn at_cursor(state: &State) -> Option<Authorship> {
    if !state.git_installed || state.view != View::Edit {
        return None;
    }
    state.repo.as_ref()?;
    let path = state.current_buffer.as_ref()?;
    let buffer = state.buffers.get(path)?;
    // A Preview row carries the source line of the block it came from, and the
    // cursor is on a row there rather than on a line (ADR-0007) — so the blame
    // is the blame of that line.
    let line = match crate::previewing(state) {
        true => crate::preview_rows(state)
            .get(buffer.row.saturating_sub(1))
            .map(|row| row.line)?,
        false => buffer.line,
    };
    let committed = state
        .committed
        .get(path)
        .and_then(Option::as_deref)
        .unwrap_or_default();
    let authors = state.blame.get(path).map(Vec::as_slice).unwrap_or_default();
    Some(
        match committed_line(committed, buffer.shown(), line).and_then(|at| authors.get(at - 1)) {
            Some(authored) => Authorship::Committed(authored.clone()),
            None => Authorship::NotCommittedYet,
        },
    )
}

/// The line of the file as the commit holds it that a buffer line came from, or
/// `None` for a line the commit does not hold.
///
/// Every line a hunk touches is answered from the hunk itself — a context line
/// names its committed line and an added line names none. A line no hunk
/// touches is answered by the offset the nearest hunk above it left behind,
/// which is what the trailing context line of that hunk states: it is the same
/// line on both sides, so the difference between its two numbers is the offset
/// every line after it carries until the next hunk.
fn committed_line(committed: &str, shown: &str, line: usize) -> Option<usize> {
    let patch =
        git2::Patch::from_buffers(committed.as_bytes(), None, shown.as_bytes(), None, None).ok()?;
    let mut hunks: BTreeMap<usize, Option<usize>> = BTreeMap::new();
    for hunk in 0..patch.num_hunks() {
        let Ok((_, lines)) = patch.hunk(hunk) else {
            continue;
        };
        for index in 0..lines {
            let Ok(line) = patch.line_in_hunk(hunk, index) else {
                continue;
            };
            if let Some(new) = line.new_lineno() {
                hunks.insert(new as usize, line.old_lineno().map(|old| old as usize));
            }
        }
    }
    if let Some(found) = hunks.get(&line) {
        return *found;
    }
    let offset = hunks
        .range(..line)
        .rev()
        .find_map(|(new, old)| Some((*old)? as isize - *new as isize))
        .unwrap_or(0);
    usize::try_from(line as isize + offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMITTED: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";

    /// The whole of the mapping's contract, which no scenario can reach: an
    /// unchanged buffer is the identity, an inserted line belongs to nobody and
    /// shifts every line under it, and a deleted one shifts them the other way.
    /// Ten lines rather than three because git's three lines of context would
    /// otherwise swallow the whole file into one hunk and hide the offset.
    #[test]
    fn a_buffer_line_is_traced_back_to_the_line_the_commit_holds() {
        let same = |line| committed_line(COMMITTED, COMMITTED, line);
        assert_eq!((same(1), same(5), same(10)), (Some(1), Some(5), Some(10)));

        let inserted = COMMITTED.replace("five\n", "five\nFIVE AND A HALF\n");
        let after = |line| committed_line(COMMITTED, &inserted, line);
        assert_eq!(after(5), Some(5));
        assert_eq!(after(6), None);
        assert_eq!((after(7), after(11)), (Some(6), Some(10)));

        let edited = COMMITTED.replace("five\n", "FIVE\n");
        let over = |line| committed_line(COMMITTED, &edited, line);
        assert_eq!((over(4), over(5), over(6)), (Some(4), None, Some(6)));

        let deleted = COMMITTED.replace("five\n", "");
        let short = |line| committed_line(COMMITTED, &deleted, line);
        assert_eq!((short(4), short(5), short(9)), (Some(4), Some(6), Some(10)));
    }

    /// A file the commit has no copy of is nobody's, line by line — the empty
    /// committed side every line of it is traced against.
    #[test]
    fn every_line_of_an_uncommitted_file_belongs_to_nobody() {
        assert_eq!(committed_line("", "fn new() {}\n", 1), None);
    }
}
