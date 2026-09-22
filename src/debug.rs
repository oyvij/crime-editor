//! The debugger's pure half. Breakpoints so far: core state that exists with or
//! without a Debug session, carried with their lines as a Buffer is edited.

use std::path::PathBuf;

/// A line the program pauses at. `text` is what the line held, trimmed, so a
/// project that remembers it can tell at load whether the line still does —
/// and re-indenting a block does not make every Breakpoint in it Stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint {
    pub file: PathBuf,
    pub line: usize,
    pub text: String,
    /// Remembered against text its line no longer holds. It keeps the text it
    /// was remembered with and the line it was set on: never re-pointed at
    /// whatever moved into its place.
    pub stale: bool,
}

/// How the gutter draws a Breakpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Plain,
    Stale,
}

/// The Breakpoints of the buffer on screen, by line, as the gutter draws them.
pub fn marks(state: &crate::State) -> std::collections::BTreeMap<usize, Mark> {
    state
        .breakpoints
        .iter()
        .filter(|breakpoint| Some(&breakpoint.file) == state.current_buffer.as_ref())
        .map(|breakpoint| {
            let mark = match breakpoint.stale {
                true => Mark::Stale,
                false => Mark::Plain,
            };
            (breakpoint.line, mark)
        })
        .collect()
}

/// Every Breakpoint in the workspace as the Breakpoint list draws it: by path,
/// then line. Read by `ui` to draw the rows, by `mouse` to hit-test them and by
/// `update` to act on the one selected, so the three cannot disagree about
/// which Breakpoint a row is.
pub fn list(state: &crate::State) -> Vec<&Breakpoint> {
    let mut rows: Vec<&Breakpoint> = state.breakpoints.iter().collect();
    rows.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    rows
}

/// The row the keyboard is on in the Breakpoint list, if it names one.
pub fn selected(state: &crate::State) -> Option<&Breakpoint> {
    list(state).get(state.breakpoints_selection).copied()
}

pub const REMOVE: &str = "remove-breakpoint";
pub const CLEAR_ALL: &str = "clear-all-breakpoints";

/// What the focused row offers: removing the Breakpoint it names.
pub fn row_actions(state: &crate::State) -> Vec<&'static str> {
    match selected(state) {
        Some(_) => vec![REMOVE],
        None => Vec::new(),
    }
}

/// The Chips on the Breakpoint list's top border. Clearing is dimmed with
/// nothing to clear, and never lit: once it has run there is nothing left for
/// it to say it did.
pub fn transport(state: &crate::State) -> Vec<crate::Chip> {
    vec![crate::Chip {
        action: CLEAR_ALL,
        name: "clear-all",
        glyph: "\u{2715}".to_string(),
        keys: "D",
        hue: crate::Hue::Halt,
        tone: match state.breakpoints.is_empty() {
            true => crate::Tone::Dimmed,
            false => crate::Tone::Plain,
        },
    }]
}

/// What 1-based `line` of `text` holds, trimmed — the text a Breakpoint is
/// remembered against — or nothing for a line `text` does not have.
pub fn held(text: &str, line: usize) -> Option<&str> {
    Some(text.split('\n').nth(line.checked_sub(1)?)?.trim())
}

/// Carries `file`'s Breakpoints from `old` to `new`: down or up with the lines
/// inserted or deleted above them, off the list with their own line deleted.
/// A line edited in place keeps its Breakpoint, and one that holds its text
/// takes the line's new text with it.
///
/// Zero context, for the reason `format::spans` diffs with none: a hunk is then
/// exactly the lines that changed, so a line inside one was replaced — kept if
/// the replacement has a line in its place, gone if not — and a line past one
/// moves by what the hunk added less what it took.
pub fn follow(breakpoints: &mut Vec<Breakpoint>, file: &std::path::Path, old: &str, new: &str) {
    let mut options = git2::DiffOptions::new();
    options.context_lines(0);
    let Ok(patch) = git2::Patch::from_buffers(
        old.as_bytes(),
        None,
        new.as_bytes(),
        None,
        Some(&mut options),
    ) else {
        return;
    };
    // A hunk that holds no lines on a side names the line it sits *after*
    // there, which is the one place a unified diff's arithmetic is not the
    // obvious one.
    let hunks: Vec<(usize, usize, usize, usize)> = (0..patch.num_hunks())
        .filter_map(|index| patch.hunk(index).ok())
        .map(|(hunk, _)| {
            let (old_lines, new_lines) = (hunk.old_lines() as usize, hunk.new_lines() as usize);
            let start = |at: u32, lines: usize| at as usize + usize::from(lines == 0);
            (
                start(hunk.old_start(), old_lines),
                old_lines,
                start(hunk.new_start(), new_lines),
                new_lines,
            )
        })
        .collect();
    let carried = |line: usize| {
        let mut offset = 0isize;
        for &(old_start, old_lines, new_start, new_lines) in &hunks {
            if line < old_start {
                break;
            }
            if line >= old_start + old_lines {
                offset += new_lines as isize - old_lines as isize;
                continue;
            }
            let within = line - old_start;
            return (within < new_lines).then_some(new_start + within);
        }
        usize::try_from(line as isize + offset).ok()
    };
    breakpoints.retain_mut(|breakpoint| {
        if breakpoint.file != file {
            return true;
        }
        let Some(line) = carried(breakpoint.line) else {
            return false;
        };
        breakpoint.line = line;
        if !breakpoint.stale {
            breakpoint.text = held(new, line).unwrap_or_default().to_string();
        }
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn on(line: usize, text: &str) -> Breakpoint {
        Breakpoint {
            file: PathBuf::from("/w/a.rs"),
            line,
            text: text.to_string(),
            stale: false,
        }
    }

    fn followed(breakpoints: &[Breakpoint], old: &str, new: &str) -> Vec<(usize, String)> {
        let mut breakpoints = breakpoints.to_vec();
        follow(&mut breakpoints, Path::new("/w/a.rs"), old, new);
        breakpoints
            .into_iter()
            .map(|breakpoint| (breakpoint.line, breakpoint.text))
            .collect()
    }

    /// The whole of what an edit can do to a Breakpoint: nothing above it moves
    /// nothing, a line inserted or deleted above carries it, and deleting its
    /// own line takes it.
    #[test]
    fn a_breakpoint_rides_the_lines_above_it_and_dies_with_its_own() {
        let old = "a\nb\nc\nd";
        let both = [on(1, "a"), on(3, "c")];
        assert_eq!(
            followed(&both, old, "a\nnew\nb\nc\nd"),
            [(1, "a".to_string()), (4, "c".to_string())]
        );
        assert_eq!(
            followed(&both, old, "a\nc\nd"),
            [(1, "a".to_string()), (2, "c".to_string())]
        );
        assert_eq!(followed(&both, old, "a\nb\nd"), [(1, "a".to_string())]);
        assert_eq!(
            followed(&both, old, "new\na\nb\nc\nd"),
            [(2, "a".to_string()), (4, "c".to_string())]
        );
    }

    /// Typing on a Breakpoint's own line is not deleting it, which a diff
    /// alone would say it was: the line is replaced by a line in its place.
    /// Its text follows, so the project remembers what the line holds now.
    #[test]
    fn a_line_edited_in_place_keeps_its_breakpoint_and_takes_its_text() {
        assert_eq!(
            followed(&[on(2, "b")], "a\nb\nc", "a\n    b2\nc"),
            [(2, "b2".to_string())]
        );
        // Two lines replaced by one keeps the first and takes the second.
        assert_eq!(
            followed(&[on(2, "b"), on(3, "c")], "a\nb\nc\nd", "a\nx\nd"),
            [(2, "x".to_string())]
        );
    }

    /// A Stale breakpoint still rides its line, but keeps the text it was
    /// remembered with: it is Stale because of that text, and quietly taking
    /// the line's would make it current without anybody having looked.
    #[test]
    fn a_stale_breakpoint_moves_but_keeps_its_remembered_text() {
        let stale = Breakpoint {
            stale: true,
            ..on(2, "let limit = 9;")
        };
        let mut breakpoints = vec![stale];
        follow(&mut breakpoints, Path::new("/w/a.rs"), "a\nb", "new\na\nb");
        assert_eq!(breakpoints[0].line, 3);
        assert_eq!(breakpoints[0].text, "let limit = 9;");
    }

    #[test]
    fn another_files_breakpoints_are_left_alone() {
        let mut breakpoints = vec![Breakpoint {
            file: PathBuf::from("/w/b.rs"),
            ..on(2, "b")
        }];
        follow(&mut breakpoints, Path::new("/w/a.rs"), "a\nb", "b");
        assert_eq!(breakpoints[0].line, 2);
    }
}
