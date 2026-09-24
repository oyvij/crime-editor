//! Run marks (F41): the ▶ beside whatever a `[run.*]` row's syntax-tree query
//! says can be started, and what starting it runs. Which nodes count and what
//! runs them is the rows'; nothing here names a `main` or a test.

use crate::layout::{self, Area};
use crate::preview::Refusal;
use crate::startup::Run;
use crate::{Chip, Effect, Hue, Modal, State, Tone};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor};
use unicode_width::UnicodeWidthStr;

pub const RUN: &str = "run";
pub const DEBUG: &str = "debug";

/// A line a Run mark stands on: the row whose query put it there, and what
/// that match captured, by capture name — the values `${…}` is filled from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    row: String,
    captures: BTreeMap<String, String>,
}

/// The parser for a file's extension: the grammars `rust-code-analysis`
/// already compiles in, so taking them costs no build (docs/stack.md). A
/// grammar says how a language is shaped, never what in it can be started —
/// that is the rows' — so an extension here with no row has no Run marks.
fn grammar(extension: &str) -> Option<Language> {
    match extension {
        "rs" => Some(tree_sitter_rust::language()),
        "java" => Some(tree_sitter_java::language()),
        "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::language()),
        "ts" | "mts" | "cts" => Some(tree_sitter_typescript::language_typescript()),
        "tsx" => Some(tree_sitter_typescript::language_tsx()),
        "py" => Some(tree_sitter_python::language()),
        _ => None,
    }
}

/// Why a row could never mark anything, which start refuses rather than
/// leaving a row that reads as configured and does nothing: an extension no
/// grammar parses, a query that does not compile against one, or a query with
/// no `@run` to say which line a match marks.
pub fn unusable(row: &Run) -> Option<String> {
    for extension in &row.extensions {
        let Some(language) = grammar(extension) else {
            return Some(format!("no grammar parses .{extension}"));
        };
        let query = match Query::new(language, &row.query) {
            Ok(query) => query,
            Err(error) => return Some(format!("query for .{extension}: {}", error.message)),
        };
        if query.capture_index_for_name(RUN).is_none() {
            return Some("query captures no @run".to_string());
        }
    }
    None
}

/// The Run marks of the buffer on screen, by line. Parsed on every call, so
/// the edge asks once per edit and the core only when a mark is acted on.
pub fn marks(state: &State) -> BTreeMap<usize, Mark> {
    let Some((path, buffer)) = state
        .current_buffer
        .as_ref()
        .and_then(|path| Some((path, state.buffers.get(path)?)))
    else {
        return BTreeMap::new();
    };
    found(&state.runs, path, buffer.shown())
}

fn found(runs: &BTreeMap<String, Run>, path: &Path, text: &str) -> BTreeMap<usize, Mark> {
    let mut marks = BTreeMap::new();
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return marks;
    };
    let claiming: Vec<_> = runs
        .iter()
        .filter(|(_, row)| row.extensions.iter().any(|claim| claim == extension))
        .collect();
    let Some(language) = grammar(extension).filter(|_| !claiming.is_empty()) else {
        return marks;
    };
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("a grammar compiled in is one tree-sitter can load");
    let Some(tree) = parser.parse(text, None) else {
        return marks;
    };
    for (name, row) in claiming {
        // Start refused a query that does not compile, so this is one that
        // does for some other configuration than the one Varde started on.
        let Ok(query) = Query::new(language, &row.query) else {
            continue;
        };
        let Some(run) = query.capture_index_for_name(RUN) else {
            continue;
        };
        let mut cursor = QueryCursor::new();
        for matched in cursor.matches(&query, tree.root_node(), text.as_bytes()) {
            let Some(marked) = matched.captures.iter().find(|capture| capture.index == run) else {
                continue;
            };
            let captures = matched
                .captures
                .iter()
                .map(|capture| {
                    (
                        query.capture_names()[capture.index as usize].clone(),
                        capture
                            .node
                            .utf8_text(text.as_bytes())
                            .unwrap_or_default()
                            .to_string(),
                    )
                })
                .collect();
            marks
                .entry(marked.node.start_position().row + 1)
                .or_insert_with(|| Mark {
                    row: name.clone(),
                    captures,
                });
        }
    }
    marks
}

/// Clicking a Run mark, or `␣x` on its line: the offer of Run and Debug. A
/// line with none is refused out loud, since the key was pressed at it.
pub fn offer(next: &mut State, line: usize) {
    match marks(next).contains_key(&line) {
        true => next.modal = Modal::RunMark { line },
        false => next.refusal = Some(Refusal::NoRunMark),
    }
}

/// The offer's Chips, and nothing while no offer is up. Debug is dimmed for
/// a row that names no way to debug it.
pub fn chips(state: &State) -> Vec<Chip> {
    let Modal::RunMark { line } = state.modal else {
        return Vec::new();
    };
    let debuggable = marks(state)
        .get(&line)
        .and_then(|mark| state.runs.get(&mark.row))
        .is_some_and(|row| row.debug.is_some());
    vec![
        Chip {
            action: RUN,
            name: "run",
            glyph: "\u{25b6}".to_string(),
            keys: "r",
            hue: Hue::Go,
            tone: Tone::Plain,
        },
        Chip {
            action: DEBUG,
            name: "debug",
            glyph: "\u{25ce}".to_string(),
            keys: "d",
            hue: Hue::Step,
            tone: match debuggable {
                true => Tone::Plain,
                false => Tone::Dimmed,
            },
        },
    ]
}

/// What the offer's box says: the row whose mark it is, and the name it
/// captured. Nothing while no offer is up.
pub fn offered(state: &State) -> Option<String> {
    let Modal::RunMark { line } = state.modal else {
        return None;
    };
    let mark = marks(state).remove(&line)?;
    Some(match mark.captures.get("name") {
        Some(name) => format!("{} · {name}", mark.row),
        None => mark.row,
    })
}

/// Where the offer is drawn on a `width` by `height` screen, centred as every
/// question is, with its Chips on the top border: the one rectangle `ui` draws
/// and `mouse` hit-tests.
pub fn offer_area(state: &State, width: u16, height: u16) -> Option<Area> {
    let text = offered(state)?;
    let strip = layout::strip_width(&layout::chip_labels(&chips(state), u16::MAX, 0));
    let widest = (UnicodeWidthStr::width(text.as_str()) as u16).max(strip);
    Some(layout::overlay(width, height, 1, widest))
}

/// The Chip under a press on the offer's top border.
pub fn chip_at(
    state: &State,
    width: u16,
    height: u16,
    column: u16,
    row: u16,
) -> Option<&'static str> {
    let area = offer_area(state, width, height).filter(|area| area.y == row)?;
    let chips = chips(state);
    let labels = layout::chip_labels(&chips, area.width, 0);
    Some(chips[layout::strip_at(area, &labels, column)?].action)
}

/// Run or Debug on the offered mark. Run is a command for a shell whose
/// prompt is waiting, which `update` finds or splits off (R38.5). Debug is a
/// session launched from the row's `debug`, filled for exactly this match.
pub fn choose(next: &mut State, action: &str) -> Vec<Effect> {
    let Modal::RunMark { line } = std::mem::take(&mut next.modal) else {
        return Vec::new();
    };
    let (Some(file), Some(mark)) = (next.current_buffer.clone(), marks(next).remove(&line)) else {
        return Vec::new();
    };
    let Some(row) = next.runs.get(&mark.row).cloned() else {
        return Vec::new();
    };
    let file = file.to_string_lossy();
    match action {
        // Quoted and stripped of control characters, because the captures are
        // the file's own text and the path is the folder's: both untrusted,
        // and both going to a shell through a terminal. The keyboard goes with
        // the command, so the job it starts is one `C-c` away.
        RUN => {
            next.focus = crate::Pane::Terminal;
            vec![Effect::RunInTerminal(filled(
                &row.run,
                &file,
                &mark,
                |text| {
                    let printable: String = text.chars().filter(|c| !c.is_control()).collect();
                    shlex::try_quote(&printable)
                        .map_or_else(|_| String::new(), |quoted| quoted.into_owned())
                },
            ))]
        }
        DEBUG => match row.debug {
            Some(mut launch) => {
                for value in launch.args.values_mut() {
                    fill_json(value, &file, &mark);
                }
                crate::debug::launch_with(next, launch)
            }
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn filled(template: &str, file: &str, mark: &Mark, quote: impl Fn(&str) -> String) -> String {
    let mut text = template.replace("${file}", &quote(file));
    for (name, value) in &mark.captures {
        text = text.replace(&format!("${{{name}}}"), &quote(value));
    }
    text
}

/// Every string in the launch arguments, filled as a command is but never
/// quoted: they reach the adapter as JSON, not a shell.
fn fill_json(value: &mut Value, file: &str, mark: &Mark) {
    match value {
        Value::String(text) => *text = filled(text, file, mark, str::to_string),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| fill_json(value, file, mark)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| fill_json(value, file, mark)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The rows Varde ships, as start reads them.
    fn shipped() -> BTreeMap<String, Run> {
        crate::startup::start(&crate::startup::Startup::default())
            .expect("the defaults start")
            .0
            .runs
    }

    fn lines(path: &str, text: &str) -> Vec<(usize, String, Option<String>)> {
        found(&shipped(), &PathBuf::from(path), text)
            .into_iter()
            .map(|(line, mark)| (line, mark.row, mark.captures.get("name").cloned()))
            .collect()
    }

    #[test]
    fn every_shipped_row_can_mark_something() {
        for (name, row) in shipped() {
            assert_eq!(unusable(&row), None, "[run.{name}]");
        }
    }

    #[test]
    fn rust_marks_main_and_tests_and_not_helpers() {
        let text = "fn main() {}\n\nfn helper() {}\n\n#[test]\nfn adds() {}\n\n#[cfg(test)]\nfn not_a_test() {}\n";
        assert_eq!(
            lines("src/main.rs", text),
            vec![
                (1, "rust_main".to_string(), Some("main".to_string())),
                (6, "rust_test".to_string(), Some("adds".to_string())),
            ]
        );
    }

    #[test]
    fn java_marks_main_and_tests_with_their_class() {
        let text = "class Orders {\n  public static void main(String[] args) {}\n  @Test\n  void adds() {}\n  void helper() {}\n}\n";
        let marks = found(&shipped(), &PathBuf::from("src/Orders.java"), text);
        assert_eq!(marks.keys().copied().collect::<Vec<_>>(), vec![2, 4]);
        assert_eq!(marks[&2].row, "java_main");
        assert_eq!(marks[&4].row, "java_test");
        assert_eq!(marks[&4].captures["name"], "adds");
        assert_eq!(marks[&4].captures["class"], "Orders");
    }

    #[test]
    fn javascript_and_typescript_mark_test_calls_by_their_title() {
        let text = "import { test } from 'vitest';\nfoo('x', () => {});\ntest('adds', () => {});\nit(\"subtracts\", () => {});\n";
        for path in ["a.test.js", "a.test.mjs", "a.test.ts", "a.test.tsx"] {
            assert_eq!(
                lines(path, text)
                    .into_iter()
                    .map(|(line, _, name)| (line, name))
                    .collect::<Vec<_>>(),
                vec![
                    (3, Some("adds".to_string())),
                    (4, Some("subtracts".to_string()))
                ],
                "{path}"
            );
        }
    }

    /// The shipped rows over a buffer, and no language server, whose start
    /// would be an effect of every event after the file opened.
    fn opened(path: &str, text: &str) -> State {
        let mut state = crate::startup::start(&crate::startup::Startup::default())
            .expect("the defaults start")
            .0;
        state.servers.clear();
        crate::update(
            &state,
            crate::Event::BufferOpened {
                path: PathBuf::from(path),
                contents: text.to_string(),
                preview: false,
                at: None,
            },
        )
        .0
    }

    #[test]
    fn space_x_offers_the_mark_on_the_caret_line_and_refuses_a_line_without_one() {
        let state = opened("/w/src/main.rs", "fn main() {}\nfn helper() {}\n");
        let offer = crate::keys::chord(&state, 'x').expect("a chord");
        assert_eq!(offer, crate::Event::OfferRun(1));
        let up = crate::update(&state, offer).0;
        assert_eq!(up.modal, Modal::RunMark { line: 1 });
        assert_eq!(offered(&up), Some("rust_main · main".to_string()));
        let refused = crate::update(&state, crate::Event::OfferRun(2)).0;
        assert_eq!(refused.modal, Modal::None);
        assert_eq!(refused.refusal, Some(Refusal::NoRunMark));
    }

    #[test]
    fn escape_leaves_the_offer_having_run_nothing() {
        let state = opened("/w/src/main.rs", "fn main() {}\n");
        let offered = crate::update(&state, crate::Event::OfferRun(1)).0;
        let escape = terminput::KeyEvent::new(terminput::KeyCode::Esc);
        let events = crate::keys::on_key_event(&offered, &mut Default::default(), escape, 0);
        assert_eq!(events, vec![crate::Event::Cancel]);
        let (left, effects) = crate::update(&offered, crate::Event::Cancel);
        assert_eq!((left.modal, effects), (Modal::None, Vec::new()));
    }

    #[test]
    fn a_row_with_no_debug_dims_the_debug_chip_and_starts_nothing() {
        let state = opened(
            "/w/src/Orders.java",
            "class Orders {\n  @Test\n  void adds() {}\n}\n",
        );
        let offered = crate::update(&state, crate::Event::OfferRun(3)).0;
        let tones: Vec<_> = chips(&offered).iter().map(|chip| chip.tone).collect();
        assert_eq!(tones, vec![Tone::Plain, Tone::Dimmed]);
        let (after, effects) = crate::update(&offered, crate::Event::ChooseRun(DEBUG));
        assert_eq!(effects, Vec::new());
        assert!(after.debug.is_none());
    }

    #[test]
    fn restart_reruns_a_session_a_run_mark_started() {
        let mut state = opened("/w/src/main.rs", "fn main() {}\n");
        state.adapters.insert(
            "rust".to_string(),
            crate::startup::Adapter {
                command: "codelldb".to_string(),
                args: Vec::new(),
                install: BTreeMap::new(),
                server: None,
                plugin: None,
            },
        );
        let offered = crate::update(&state, crate::Event::OfferRun(1)).0;
        let (started, effects) = crate::update(&offered, crate::Event::ChooseRun(DEBUG));
        assert!(matches!(effects.as_slice(), [Effect::StartDap { .. }]));
        let mut ended = started;
        ended.debug = None;
        let (_, again) = crate::update(&ended, crate::Event::DebugRestart);
        assert_eq!(again, effects);
    }

    #[test]
    fn a_file_no_row_claims_has_no_marks() {
        assert!(lines("notes.txt", "fn main() {}").is_empty());
        assert!(lines("a.py", "def test_adds():\n    pass\n").is_empty());
    }

    #[test]
    fn a_row_that_cannot_mark_is_named_why() {
        let row = |extensions: &[&str], query: &str| Run {
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
            query: query.to_string(),
            run: String::new(),
            debug: None,
        };
        assert_eq!(
            unusable(&row(&["zig"], "(x) @run")),
            Some("no grammar parses .zig".to_string())
        );
        assert!(unusable(&row(&["rs"], "(no_such_node) @run"))
            .is_some_and(|why| why.starts_with("query for .rs")));
        assert_eq!(
            unusable(&row(&["rs"], "(function_item)")),
            Some("query captures no @run".to_string())
        );
    }

    #[test]
    fn a_command_quotes_what_it_is_filled_with_and_launch_arguments_do_not() {
        let mark = Mark {
            row: String::new(),
            captures: BTreeMap::from([("name".to_string(), "it's $HOME".to_string())]),
        };
        let quote = |text: &str| shlex::try_quote(text).unwrap().into_owned();
        assert_eq!(
            filled("t ${file} -k ${name}", "/a b.py", &mark, quote),
            r#"t '/a b.py' -k "it's "'$HOME'"#
        );
        let mut args = serde_json::json!({ "args": ["${file}", { "k": "${name}" }], "n": 1 });
        fill_json(&mut args, "/a b.py", &mark);
        assert_eq!(
            args,
            serde_json::json!({ "args": ["/a b.py", { "k": "it's $HOME" }], "n": 1 })
        );
    }
}
