//! F40 — who last committed the line the cursor is on.
//!
//! Git's word is *blame*, and what git blames is a whole file; what the border
//! says is one line's Authorship, which is why `CONTEXT.md` names the concept
//! that way and this module with it.
//!
//! It is the *committed* file's answer, read by the edge off the commit `HEAD`
//! names and cached there against that commit — never against
//! `Buffer::revision`, which every keystroke bumps and a walk of a file's
//! history is not a thing to do per keystroke. What the buffer has changed
//! since is the diff's business, which is why a buffer line is traced back
//! through the patch before it names anybody: a line inserted above the cursor
//! would otherwise hand the cursor's line the author of the line above it.
//!
//! [`traced`] is that patch, and it is the only one: the Change bar bars
//! exactly the lines it answers `None` for ([`crate::changed_lines`]), so the
//! gutter and the border cannot disagree about which lines the commit holds.

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

/// The Authorship of the line the cursor is on, or `None` for a border with
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
    // cursor is on a row there rather than on a line (ADR-0007) — so the
    // Authorship is that line's.
    let line = match crate::previewing(state) {
        true => crate::preview_rows(state)
            .get(buffer.row.saturating_sub(1))
            .map(|row| row.line)?,
        false => buffer.line,
    };
    let authors = state
        .authorship
        .get(path)
        .map(|lines| &lines[..])
        .unwrap_or_default();
    let at = traced_lines(state)?
        .get(line.checked_sub(1)?)
        .copied()
        .flatten();
    Some(match at.and_then(|at| authors.get(at - 1)) {
        Some(authored) => Authorship::Committed(authored.clone()),
        None => Authorship::NotCommittedYet,
    })
}

/// [`traced`] for the buffer on screen, as the edge last told it
/// ([`State::traced`](crate::State::traced)) — nothing while it has not, which
/// the border says nothing about rather than claiming a line nobody has
/// looked up yet was never committed. Nothing for a trace of another revision
/// either: after an edit that added or removed a line, every entry past it
/// names the wrong line.
pub fn traced_lines(state: &State) -> Option<&[Option<usize>]> {
    let (path, revision, lines) = state.traced.as_ref()?;
    let buffer = crate::current_buffer(state)?;
    (state.current_buffer.as_ref() == Some(path) && buffer.revision() == *revision)
        .then_some(&lines[..])
}

/// Which line of the file as the commit holds it each buffer line came from,
/// one entry per line of `shown` and `None` for a line the commit does not hold.
///
/// The one diff of the two sides. Both things Varde says about a changed file
/// read it: the Authorship indexes the commit's blame through it, and the
/// Change bar bars exactly the lines it answers `None` for. Two derivations
/// would be two answers to which lines the commit holds, and the disagreement
/// would be a gutter bar beside a line the border credits to somebody.
///
/// Every line a hunk touches is answered from the hunk itself — a context line
/// names its committed line and an added line names none. A line no hunk
/// touches is answered by the offset the nearest hunk above it left behind,
/// which is what the trailing context line of that hunk states: it is the same
/// line on both sides, so the difference between its two numbers is the offset
/// every line after it carries until the next hunk.
///
/// Nothing at all for a file the commit has no copy of: every line of it would
/// be a Change bar, which is the one thing R37.1 says a mark is not, and a line
/// with no entry is one the commit does not hold, which is what the Authorship
/// says of all of them.
pub fn traced(committed: Option<&str>, shown: &str) -> Vec<Option<usize>> {
    let Some(committed) = committed else {
        return Vec::new();
    };
    let count = shown.split('\n').count();
    let Ok(patch) =
        git2::Patch::from_buffers(committed.as_bytes(), None, shown.as_bytes(), None, None)
    else {
        return vec![None; count];
    };
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
    (1..=count)
        .map(|line| match hunks.get(&line) {
            Some(found) => *found,
            None => {
                let offset = hunks
                    .range(..line)
                    .rev()
                    .find_map(|(new, old)| Some((*old)? as isize - *new as isize))
                    .unwrap_or(0);
                usize::try_from(line as isize + offset).ok()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMITTED: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";

    /// #101: the Change bars and the Authorship read the trace the edge told
    /// and never diff the file themselves — both are asked on every frame, and
    /// the diff is a whole file's worth of work. Keyed by the buffer and its
    /// revision, so another file's trace is not this one's and nor is the
    /// trace of the text before an edit (#104).
    #[test]
    fn the_marks_read_the_trace_the_edge_told() {
        let path = std::path::PathBuf::from("/w/main.rs");
        let mut state = State {
            current_buffer: Some(path.clone()),
            ..State::default()
        };
        state.buffers.insert(
            path.clone(),
            crate::editor::Buffer::open("a\nb\n", false, 4),
        );
        state
            .committed
            .insert(path.clone(), Some("a\n".to_string()));
        assert!(crate::changed_lines(&state).is_empty(), "diffed on its own");

        let revision = state.buffers[&path].revision();
        state.traced = Some((path.clone(), revision, vec![Some(1), None, None].into()));
        assert_eq!(crate::changed_lines(&state), [2, 3]);

        state.traced = Some((path, revision - 1, vec![None, Some(1), None].into()));
        assert!(
            crate::changed_lines(&state).is_empty(),
            "an older revision's"
        );

        let other = std::path::PathBuf::from("/w/other.rs");
        state.traced = Some((other, revision, vec![None].into()));
        assert!(crate::changed_lines(&state).is_empty());
    }

    /// The whole of the mapping's contract, which no scenario can reach: an
    /// unchanged buffer is the identity, an inserted line belongs to nobody and
    /// shifts every line under it, and a deleted one shifts them the other way.
    /// Ten lines rather than three because git's three lines of context would
    /// otherwise swallow the whole file into one hunk and hide the offset every
    /// line after a hunk carries.
    #[test]
    fn a_buffer_line_is_traced_back_to_the_line_the_commit_holds() {
        let at =
            |committed: &str, shown: &str, line: usize| traced(Some(committed), shown)[line - 1];

        let same = |line| at(COMMITTED, COMMITTED, line);
        assert_eq!((same(1), same(5), same(10)), (Some(1), Some(5), Some(10)));

        let inserted = COMMITTED.replace("five\n", "five\nFIVE AND A HALF\n");
        let after = |line| at(COMMITTED, &inserted, line);
        assert_eq!((after(5), after(6)), (Some(5), None));
        assert_eq!((after(7), after(11)), (Some(6), Some(10)));

        let edited = COMMITTED.replace("five\n", "FIVE\n");
        let over = |line| at(COMMITTED, &edited, line);
        assert_eq!((over(4), over(5), over(6)), (Some(4), None, Some(6)));

        let deleted = COMMITTED.replace("five\n", "");
        let short = |line| at(COMMITTED, &deleted, line);
        assert_eq!((short(4), short(5), short(9)), (Some(4), Some(6), Some(10)));
    }

    /// A file the commit has no copy of is nobody's, line by line — the empty
    /// committed side every line of it is traced against.
    ///
    /// Lines are `split('\n')`, which is how the rest of the core counts a
    /// Buffer's, so text ending in a newline has an empty last line and both
    /// sides carry one. Only the degenerate pairing of an empty commit against
    /// a file that does is lopsided, and there the answer past the end is not
    /// reached: an untracked file has no rows of authors to index either.
    #[test]
    fn every_line_of_an_uncommitted_file_belongs_to_nobody() {
        assert_eq!(traced(Some(""), "fn new() {}"), [None]);
        assert_eq!(traced(Some(""), "fn new() {}\nfn old() {}"), [None, None]);
    }

    /// The Change bar's half of the same answer, which used to be its own diff
    /// in `story::added_lines` and is now the lines this one has no commit for.
    /// The cases are that function's, so the gutter's contract survived the
    /// merge: an edited line and an inserted one are both changes, a deleted one
    /// is not a line to mark, and an unchanged file marks nothing.
    #[test]
    fn the_lines_the_commit_does_not_hold_are_the_ones_with_no_committed_line() {
        let changed = |committed: &str, shown: &str| -> Vec<usize> {
            traced(Some(committed), shown)
                .into_iter()
                .enumerate()
                .filter(|(_, at)| at.is_none())
                .map(|(index, _)| index + 1)
                .collect()
        };
        let old = "a\nb\nc\n";
        assert_eq!(changed(old, old), Vec::<usize>::new());
        assert_eq!(changed(old, "a\nx\nb\nc\n"), vec![2]);
        assert_eq!(changed(old, "a\nB\nc\n"), vec![2]);
        assert_eq!(changed(old, "a\nc\n"), Vec::<usize>::new());
        assert_eq!(changed(old, "1\na\nb\nc\n2\n"), vec![1, 5]);
    }
}
