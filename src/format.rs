//! F32 — laying a file out with a command the project named.
//!
//! Runs nothing. The core decides which command, with which arguments, over
//! which text, and returns it as [`Effect::RunFormatter`]; the edge spawns it,
//! feeds it the Buffer on stdin, and hands back what it made of it. Which
//! command a language gets is a `[formatter.<language>]` row and never an arm —
//! the line `docs/adr/0011-a-language-server-is-a-second-hosted-child.md` draws
//! for a server, drawn again for a tool, and amended for the install command by
//! `docs/adr/0012-an-install-command-is-configuration.md`.

use crate::{lsp, Effect, Place, State};
use std::path::Path;

/// What running one came to, as only the edge can observe it. The three are one
/// event because they are one fact arriving — the process ran, or it did not —
/// and because the staleness rule underneath them is one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// What it wrote on stdout, which is the whole file laid out.
    Done(String),
    /// The command is not on this process's `PATH`. Asked of the machine at the
    /// moment it mattered rather than remembered from a probe: nothing is held,
    /// so nothing can go stale, which is the whole of "format the moment it is
    /// installed, with no restart" (R32.9).
    Missing,
    /// It ran and refused, with what it said about why. Kept as the command's
    /// own words: a formatter's complaint about a syntax error is the one
    /// sentence that says which line to look at, and swallowing it leaves a
    /// notice nobody can act on.
    Failed(String),
}

/// `:format` where no Language server will answer. The refusals are the
/// interesting half: a language nothing configures is told which key to write,
/// because the library is the only party that knows which names a message may
/// interpolate (R31.27).
pub fn run(state: &State) -> Vec<Effect> {
    let Some(path) = state.current_buffer.clone() else {
        return Vec::new();
    };
    let Some(buffer) = state.buffers.get(&path) else {
        return Vec::new();
    };
    let language = language(state, &path);
    let Some(formatter) = state.formatters.get(&language) else {
        return vec![Effect::notify_about(
            "no-formatter-configured",
            format!("[formatter.{language}] in .varde/config.toml"),
        )];
    };
    // `${file}` beside every `[facts.*]` name, filled and dropped by the rule a
    // server's arguments already go through: an argument naming something
    // nobody could resolve is worse than an absent one, because a command that
    // fails on a configured-looking value reads as Varde's bug.
    let mut names = lsp::facts(state);
    names.insert(
        "file".to_string(),
        Some(path.to_string_lossy().into_owned()),
    );
    vec![Effect::RunFormatter {
        language,
        command: formatter.command.clone(),
        args: formatter
            .args
            .iter()
            .filter_map(|arg| lsp::filled(arg, &names))
            .collect(),
        path: path.clone(),
        revision: buffer.revision(),
        text: buffer.shown().to_string(),
    }]
}

/// Which language this file is, for the purpose of finding a command to lay it
/// out: the `[formatter.*]` row claiming its extension, else the name the file
/// gives itself — which is what makes a `[formatter.<anything>]` a reader
/// invents reachable without Varde having heard of it. The `[lsp.*]` rows are
/// not asked: a file's server and its formatter are separate choices
/// (ADR 0018).
fn language(state: &State, path: &Path) -> String {
    // A file with no extension is named by itself — `Makefile`, `Dockerfile` —
    // and that name is a key a reader can write, because the last lookup *is*
    // the map lookup: `[formatter.Makefile]` finds a file called `Makefile`.
    // The empty extension it would otherwise fall back to names
    // `[formatter.]`, which is a refusal telling somebody to write a key TOML
    // will not accept.
    let named = path
        .extension()
        .or_else(|| path.file_name())
        .and_then(|named| named.to_str())
        .unwrap_or_default();
    state
        .formatters
        .iter()
        .find(|(_, formatter)| formatter.extensions.iter().any(|claim| claim == named))
        .map(|(language, _)| language.clone())
        .unwrap_or_else(|| named.to_string())
}

/// What the command made of it, put back into the Buffer it came out of.
///
/// The staleness rule is the one every reply that *changes* text goes through:
/// a reader who typed while the command ran is holding different text, and the
/// place a diff named in the old one names something else in the new (R31.7).
pub fn answered(
    state: &mut State,
    language: &str,
    path: &Path,
    revision: u64,
    answer: Answer,
) -> Vec<Effect> {
    match answer {
        // Typed into the terminal and never run, and Varde composed none of it
        // — it is the string configuration carried, so a default that is wrong
        // for this machine is one word away from being right (ADR 0012). A row
        // with nothing for this OS offers nothing rather than an invented
        // command, which is a row that cannot make itself true.
        Answer::Missing => {
            let formatter = state.formatters.get(language);
            let install = formatter.and_then(|formatter| formatter.install.get(&state.os));
            let mut effects: Vec<Effect> = install
                .cloned()
                .into_iter()
                .map(Effect::SetTerminalInput)
                .collect();
            effects.push(Effect::notify_about(
                "formatter-missing",
                formatter
                    .map(|formatter| formatter.command.clone())
                    .unwrap_or_default(),
            ));
            effects
        }
        // Its own first line, because that is the one that names the line to
        // look at; the rest is a stack of paths the status row has no space for.
        Answer::Failed(why) => vec![Effect::notify_about(
            "formatter-failed",
            why.lines().next().unwrap_or_default().trim().to_string(),
        )],
        Answer::Done(text) => {
            let named = state
                .formatters
                .get(language)
                .map(|formatter| formatter.command.clone())
                .unwrap_or_default();
            let Some(buffer) = state.buffers.get_mut(path) else {
                return Vec::new();
            };
            if buffer.revision() != revision {
                return Vec::new();
            }
            // Nothing on stdout from a command that exited fine is not an
            // answer, and read as one it is the whole file deleted with no
            // notice at all — which is exactly what a formatter that rewrites
            // the file in place prints, the one shape Varde does not support
            // (R32.4) and the misconfiguration a hand-written row reaches
            // first. It fails like a command that failed, because it did.
            // `trim` on the text that is there rather than `is_empty`: nothing
            // is a true answer for a Buffer holding only whitespace.
            if text.is_empty() && !buffer.shown().trim().is_empty() {
                return vec![Effect::notify_about("formatter-failed", named)];
            }
            let spans = spans(buffer.shown(), &text);
            // A command that handed back what it was given is the same nothing
            // the Language server half says out loud, and it is said for the
            // same reason: a key was pressed, and a key that changes nothing
            // and says nothing reads as a broken key. Both halves of `:format`
            // reach for one slug, because the reader pressed one key and does
            // not know which half answered (R32.2).
            if spans.is_empty() {
                return vec![Effect::Notify("nothing-to-format")];
            }
            buffer.reformat(&spans);
            Vec::new()
        }
    }
}

/// The command's answer as the edits that reach the Buffer, hunk by hunk rather
/// than as one span over the whole file. One span "works" and parks the cursor
/// on the last character of the file, because there is nowhere else for a
/// cursor inside a replacement to go — [`crate::editor::Buffer::reformat`]
/// rides the cursor over each edit, and edits it can ride are what keep the
/// reader where they were reading.
///
/// Diffed here rather than through [`crate::story::hunks`], which asks the same
/// crate the same question: that one carries three lines of context because a
/// reviewer reads around a change, and context inside a *span* is text replaced
/// with itself — an edit the cursor has to ride for no reason, and undo
/// covering lines nothing touched. Zero context is what makes a span the change.
///
/// Whole lines, which is the granularity a diff has. A hunk that replaces no
/// lines at all is an insertion, and a unified diff names the line it goes
/// *after*, which is the one place this arithmetic is not the obvious one.
fn spans(old: &str, new: &str) -> Vec<(Place, Place, String)> {
    let lines: Vec<&str> = new.split('\n').collect();
    let mut options = git2::DiffOptions::new();
    options.context_lines(0);
    let Ok(patch) = git2::Patch::from_buffers(
        old.as_bytes(),
        None,
        new.as_bytes(),
        None,
        Some(&mut options),
    ) else {
        return Vec::new();
    };
    (0..patch.num_hunks())
        .filter_map(|index| patch.hunk(index).ok())
        .map(|(hunk, _)| {
            let at = match hunk.old_lines() {
                0 => hunk.old_start() as usize + 1,
                _ => hunk.old_start() as usize,
            };
            let from = hunk.new_start() as usize - usize::from(hunk.new_lines() > 0);
            // The last segment of the new text carries no newline, because
            // there is none after it: a file whose formatter left the final
            // line unterminated must not grow one, and a `split` counts the
            // empty tail after a trailing newline as a segment of its own, so
            // "is this the last one" is the whole of the question.
            let text = lines
                .iter()
                .enumerate()
                .skip(from)
                .take(hunk.new_lines() as usize)
                .map(|(index, line)| match index + 1 == lines.len() {
                    true => (*line).to_string(),
                    false => format!("{line}\n"),
                })
                .collect();
            (
                Place {
                    line: at,
                    column: 1,
                },
                Place {
                    line: at + hunk.old_lines() as usize,
                    column: 1,
                },
                text,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::spans;
    use crate::Place;

    /// The whole file changed is still one span per hunk, and it covers exactly
    /// the lines the old text had.
    #[test]
    fn one_changed_line_is_one_span_over_that_line() {
        assert_eq!(
            spans("a\nb\nc\n", "a\nB\nc\n"),
            vec![(
                Place { line: 2, column: 1 },
                Place { line: 3, column: 1 },
                "B\n".to_string()
            )]
        );
    }

    /// A hunk that replaces nothing names the line it goes after, which is the
    /// one place the arithmetic is not the obvious one.
    #[test]
    fn an_inserted_line_lands_after_the_line_the_hunk_names() {
        assert_eq!(
            spans("a\nc\n", "a\nb\nc\n"),
            vec![(
                Place { line: 2, column: 1 },
                Place { line: 2, column: 1 },
                "b\n".to_string()
            )]
        );
    }

    /// And a hunk that puts nothing back covers the lines it takes away.
    #[test]
    fn a_deleted_line_is_a_span_with_nothing_in_it() {
        assert_eq!(
            spans("a\nb\nc\n", "a\nc\n"),
            vec![(
                Place { line: 2, column: 1 },
                Place { line: 3, column: 1 },
                String::new()
            )]
        );
    }

    /// A Buffer holding only whitespace, which the refusal in [`answered`]
    /// must not stand in front of: nothing is a true answer for a file with no
    /// text in it. No Scenario can reach it — a Gherkin docstring is dedented,
    /// so a row of spaces arrives as a row of nothing.
    #[test]
    fn nothing_answered_over_a_buffer_of_whitespace_is_applied() {
        let path = std::path::PathBuf::from("/w/thing.json");
        let mut state = crate::State::default();
        state
            .buffers
            .insert(path.clone(), crate::editor::Buffer::open("   \n", false, 4));
        let revision = state.buffers[&path].revision();
        let effects = super::answered(
            &mut state,
            "json",
            &path,
            revision,
            super::Answer::Done(String::new()),
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(state.buffers[&path].shown(), "");
    }

    /// A formatter's own words are a child's bytes, and a child that writes an
    /// escape sequence into its stderr is writing to the status line. The
    /// escape goes and the sentence stays, so the notice still names the line
    /// to look at. A unit test rather than a Scenario for the reason the
    /// Language server's twin is one: the scenarios assert the slug, never the
    /// wording, so nothing there can see the bytes.
    #[test]
    fn a_failure_says_its_first_line_without_the_control_characters_in_it() {
        assert_eq!(
            super::answered(
                &mut crate::State::default(),
                "json",
                std::path::Path::new("/w/thing.json"),
                0,
                super::Answer::Failed("\u{1b}[2Jboom: line 1\nstack".to_string()),
            ),
            vec![crate::Effect::NotifyAbout {
                slug: "formatter-failed",
                about: "[2Jboom: line 1".to_string(),
            }]
        );
    }

    /// Text a command handed back unchanged is no edit at all, which is what
    /// keeps `:format` off the undo stack when there was nothing to do.
    #[test]
    fn nothing_changed_is_no_span() {
        assert!(spans("a\nb\n", "a\nb\n").is_empty());
    }

    /// A file whose last line has no newline does not grow one, which a span
    /// built by putting a newline after every line it takes would give it.
    #[test]
    fn an_unterminated_last_line_stays_unterminated() {
        assert_eq!(
            spans("{\"a\":1}", "{\n  \"a\": 1\n}"),
            vec![(
                Place { line: 1, column: 1 },
                Place { line: 2, column: 1 },
                "{\n  \"a\": 1\n}".to_string()
            )]
        );
    }
}
