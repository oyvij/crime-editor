//! The debugger's pure half. Breakpoints: core state that exists with or
//! without a Debug session, carried with their lines as a Buffer is edited.
//! And the Debug session: the Debug Adapter Protocol's requests built and its
//! replies and events read, as JSON the edge frames and moves without deciding
//! anything (`docs/adr/0021-a-debug-adapter-is-a-hosted-child-reached-three-ways.md`).
//!
//! By hand over `serde_json::Value` rather than a crate's types: `dap`, the one
//! maintained crate, is written for implementing an adapter, so its requests
//! only deserialize and its responses and events only serialize — the
//! opposite of what a client needs.

use crate::preview::Refusal;
use crate::{layout, Effect, Pane, Place, State};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
pub const TOGGLE_OUTPUT: &str = "toggle-output";
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

/// The Chips on the Variables' top border. One so far: hiding the Program
/// output and showing it again are one control, so they are one Chip, named
/// for what pressing it does — the Reading Transport's play and pause, one
/// pane over. None at all with no program running, for the reason the `Debug`
/// Group tab is offered only while a session exists.
pub fn strip_transport(state: &crate::State) -> Vec<crate::Chip> {
    if !state.output_running {
        return Vec::new();
    }
    vec![crate::Chip {
        action: TOGGLE_OUTPUT,
        name: match state.output_hidden {
            true => "show-output",
            false => "hide-output",
        },
        glyph: match state.output_hidden {
            true => "\u{25a3}".to_string(),
            false => "\u{25a2}".to_string(),
        },
        keys: "\u{2423}h",
        hue: crate::Hue::Plain,
        tone: match state.output_unseen {
            true => crate::Tone::Marked,
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

/// A Debug session, laid over Edit view. One field of `State`: a session
/// exists or it does not. Everything here is a claim about a conversation the
/// core is party to. Whether the adapter's process exists is the edge's to
/// say: a session asked for is `Spawning` until `Event::DapStarted`, and
/// gone at `Event::DapGone`, never assumed from the spawn having been asked.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// The adapter's row name, which `initialize` names it by, and the command
    /// that runs it, which a refusal names — both taken at the start, with the
    /// request below.
    adapter: String,
    command: String,
    pub phase: Phase,
    /// `launch` or `attach`, and what that request carries — taken when the
    /// session starts, so a config edited mid-session changes the next one.
    request: String,
    args: serde_json::Map<String, Value>,
    /// The `seq` the last request went out with, and every one still
    /// unanswered, by `seq`: a response names its request only by number.
    seq: i64,
    asked: BTreeMap<i64, Ask>,
    /// A pause asked for before any thread was known, so it waits on the
    /// `threads` answer that names one.
    pausing: bool,
    /// What the Corner and the Strip held when the session began, given back
    /// when it ends.
    corner: layout::Corner,
    strip: layout::Group,
}

/// A request waiting for its answer: the command, which is how the response is
/// read, and the Variables reference it asked about — which a `variables`
/// response carries nowhere in itself, so an answer would otherwise be a list
/// of members belonging to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ask {
    command: String,
    reference: i64,
}

/// Where a session has got to. An enum rather than flags, for the reason
/// `Modal` is one: Running and Paused at once has no answer for F9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Asked of the edge, which has not yet said it holds an adapter.
    Spawning,
    /// `initialize` sent, and nothing else until it is answered.
    Initializing,
    /// Launched or attached, waiting for the adapter's `initialized` before
    /// the Breakpoints go: one that arrives any earlier would send them ahead
    /// of the program they are for.
    Starting,
    /// Running, holding what the last pause showed so it can stay on screen,
    /// dimmed, while the program runs — `None` until the first pause. The
    /// pause has one home either way: a second field holding it beside the
    /// phase is two authors for one fact.
    Running(Option<Pause>),
    Paused(Pause),
    /// `disconnect` sent, waiting for its answer before the adapter is let go:
    /// dropped at once, it could take a launched program down with it before
    /// it had heard it was being stopped. A second stop does not wait.
    Stopping,
}

/// One thread stopped, the call stack it stopped in, and which Frame is being
/// inspected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pause {
    pub thread: i64,
    pub why: Why,
    /// What the adapter said paused it, where it said anything: the first
    /// Variables row, since an exception is what the reader is looking for.
    exception: Option<String>,
    /// Empty until the adapter answers `stackTrace`, which the pause asks for.
    pub frames: Vec<Frame>,
    pub chosen: usize,
    /// The chosen Frame's scopes, as the adapter named them, and the children
    /// fetched for every reference that has been opened. A member is asked
    /// for when it is opened and never before: a tree walked whole at every
    /// pause is a debugger that stops for seconds on a deep structure.
    scopes: Vec<Member>,
    children: BTreeMap<i64, Vec<Member>>,
    open: BTreeSet<i64>,
    /// How many children of a reference have arrived, which is where its next
    /// page starts.
    fetched: BTreeMap<i64, usize>,
}

/// One member of the Variables tree as the adapter named it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
    name: String,
    value: String,
    /// What to ask for this member's children by; 0 for one that has none.
    reference: i64,
    hint: Hint,
    /// How many indexed children the adapter says it has, which is what
    /// decides whether it is read a page at a time. 0 for a member the
    /// adapter did not count, which is every member small enough not to need
    /// counting.
    indexed: usize,
}

/// How a member is drawn, as the adapter's presentation hints say — never as
/// Varde guesses from the language, which it does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    Plain,
    Private,
    ReadOnly,
    /// A member the adapter will only compute when it is asked for, which is
    /// what opening it does.
    Lazy,
}

impl Hint {
    pub fn as_str(self) -> &'static str {
        match self {
            Hint::Plain => "plain",
            Hint::Private => "private",
            Hint::ReadOnly => "read-only",
            Hint::Lazy => "lazy",
        }
    }
}

/// One row of the Variables as it is drawn: the tree flattened to what is
/// open, which is what `ui` draws, what the mouse hit-tests and what Enter
/// acts on — the three reading one list, for the reason the Breakpoint list's
/// rows are one list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub value: String,
    pub depth: usize,
    pub hint: Hint,
    pub open: bool,
    pub opens: Opens,
}

/// What opening a row asks the adapter for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opens {
    /// A member with no children to ask for.
    Nothing,
    Children {
        reference: i64,
        indexed: usize,
    },
    /// The row that stands for the rest of a collection too big to have come
    /// whole, and the index it carries on from.
    NextPage {
        reference: i64,
        start: usize,
    },
}

/// How many members of an indexed collection are asked for at a time. A
/// hundred is more rows than any pane shows and few enough that an adapter
/// answers at once; the alternative — asking for all of them — is a ten
/// thousand element vector serialized into a pane twenty rows tall.
const PAGE: usize = 100;

/// Why the program paused, as far as the Paused line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Paused,
    Exception,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Why::Paused => "paused",
            Why::Exception => "exception",
        }
    }
}

/// How far one step goes: over a call, into it, or out of the one being
/// inspected. Named for the gesture rather than for the protocol, because it is
/// what a key, a Chip and a cheatsheet row all say; `step` below is the one
/// place the protocol's spelling for each of them lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Over,
    Into,
    Out,
}

/// One call on the stack. `file` is absent for a Frame with no source the
/// adapter can name — a call inside a library shipped without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The adapter's number for it, which is the only way to ask for its
    /// scopes: a Frame is named to the reader and numbered to the adapter.
    pub id: i64,
    pub name: String,
    pub file: Option<PathBuf>,
    pub line: usize,
}

/// Why the edge stopped holding an adapter. Missing is its own case because it
/// is the one the reader fixes by installing something, so it is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    Missing,
    FailedToStart,
    Exited,
}

/// The names the launch list offers, in the order it draws them.
pub fn launches(state: &State) -> Vec<&str> {
    state.launches.keys().map(String::as_str).collect()
}

/// Starts the Launch configuration `name`: refused by name if it names no
/// configured adapter, and otherwise a spawn asked of the edge, whose answer
/// is the session's first fact.
pub fn start(next: &mut State, name: &str) -> Vec<Effect> {
    if next.debug.is_some() {
        next.refusal = Some(Refusal::SessionRunning);
        return Vec::new();
    }
    let Some(launch) = next.launches.get(name) else {
        return Vec::new();
    };
    let Some(adapter) = next.adapters.get(&launch.adapter) else {
        next.refusal = Some(Refusal::NoDebugAdapter(launch.adapter.clone()));
        return Vec::new();
    };
    let effect = Effect::StartDap {
        command: adapter.command.clone(),
        args: adapter.args.clone(),
    };
    next.debug = Some(Session {
        adapter: launch.adapter.clone(),
        command: adapter.command.clone(),
        phase: Phase::Spawning,
        request: launch.request.clone(),
        args: launch.args.clone(),
        seq: 0,
        asked: BTreeMap::new(),
        pausing: false,
        corner: next.corner,
        strip: next.strip,
    });
    vec![effect]
}

/// The edge holds the adapter now, so the conversation begins.
pub fn started(next: &mut State) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut().filter(|s| s.phase == Phase::Spawning) else {
        return Vec::new();
    };
    session.phase = Phase::Initializing;
    let adapter = session.adapter.clone();
    vec![ask(
        session,
        "initialize",
        json!({
            "clientID": "varde",
            "clientName": "Varde",
            "adapterID": adapter,
            "pathFormat": "path",
            "linesStartAt1": true,
            "columnsStartAt1": true,
        }),
    )]
}

/// The edge stopped holding the adapter. Whatever the session was waiting on
/// went with it; a session that was being stopped has already ended.
pub fn gone(next: &mut State, why: Gone) -> Vec<Effect> {
    let Some(session) = next.debug.as_ref() else {
        return Vec::new();
    };
    let command = session.command.clone();
    // Said twice: the refusal answers the event in the footer, and the notice
    // stays in the status line, because this arrives when the edge notices
    // rather than when anybody pressed anything, and the next event of any
    // kind takes a refusal down.
    let (refusal, notice) = match (why, &session.phase) {
        (_, Phase::Stopping) => (None, None),
        (Gone::Missing, _) => (
            Some(Refusal::NoDebugAdapter(command.clone())),
            Some(Effect::notify_about("no-debug-adapter", command)),
        ),
        (Gone::FailedToStart, _) => (
            Some(Refusal::DebugAdapterFailed),
            Some(Effect::Notify("debug-adapter-failed")),
        ),
        (Gone::Exited, _) => (
            Some(Refusal::DebugAdapterExited),
            Some(Effect::Notify("debug-adapter-exited")),
        ),
    };
    next.refusal = refusal;
    end(next);
    notice.into_iter().collect()
}

/// One message the adapter sent, exactly as the edge read it.
pub fn received(next: &mut State, json: &str) -> Vec<Effect> {
    let Ok(message) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    if next.debug.is_none() {
        return Vec::new();
    }
    match message["type"].as_str() {
        Some("response") => answered(next, &message),
        Some("event") => told(next, &message),
        Some("request") => {
            let session = next.debug.as_mut().expect("a session");
            let command = message["command"].as_str().unwrap_or_default();
            // The adapter asking for a terminal to run the debugged program
            // in. It gets the Debug group's own, never a shell: the Strip's
            // shells are the reader's, and a program started in one would
            // print over whatever was running there and end with the next
            // `:split`.
            if command == "runInTerminal" {
                let arguments = &message["arguments"];
                let argv: Vec<String> = arguments["args"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(printable)
                    .collect();
                if argv.is_empty() {
                    return vec![reply(
                        session,
                        &message,
                        json!({
                            "success": false,
                            "command": command,
                            "message": "runInTerminal named no program",
                        }),
                    )];
                }
                // The adapter's, which is untrusted input, so it is never
                // interpolated into a shell command: the argv goes to the pty
                // as it stands and the environment is a map of names to
                // values, both handed to the edge rather than spelled out as
                // a line something else would parse.
                let env = arguments["env"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(name, value)| Some((printable(name), printable(value.as_str()?))))
                    .collect();
                let cwd = arguments["cwd"].as_str().map(printable).map(PathBuf::from);
                return vec![
                    reply(
                        session,
                        &message,
                        json!({ "success": true, "command": command, "body": {} }),
                    ),
                    Effect::RunProgram { argv, cwd, env },
                ];
            }
            // A reverse request nothing here answers is refused out loud: an
            // adapter left waiting on a reply is a session that hangs.
            vec![reply(
                session,
                &message,
                json!({
                    "success": false,
                    "command": command,
                    "message": format!("Varde does not answer {command}"),
                }),
            )]
        }
        _ => Vec::new(),
    }
}

fn answered(next: &mut State, message: &Value) -> Vec<Effect> {
    let session = next.debug.as_mut().expect("a session");
    let Some(Ask { command, reference }) = message["request_seq"]
        .as_i64()
        .and_then(|seq| session.asked.remove(&seq))
    else {
        return Vec::new();
    };
    if message["success"] != Value::Bool(true) {
        return match command.as_str() {
            "initialize" | "launch" | "attach" => {
                // The adapter's words, stripped of anything that could drive
                // the terminal they are about to be drawn on.
                // The readable text is the error's `format` where the
                // adapter sent one; `message` is often only a short code.
                let why = printable(
                    message["body"]["error"]["format"]
                        .as_str()
                        .or(message["message"].as_str())
                        .unwrap_or(&command),
                );
                end(next);
                next.refusal = Some(Refusal::LaunchFailed(why.clone()));
                // And in the status line, for the reason `gone` says it twice.
                vec![Effect::notify_about("launch-failed", why), Effect::StopDap]
            }
            // The answer that lets the adapter go, whatever it says.
            "disconnect" => {
                end(next);
                vec![Effect::StopDap]
            }
            // A pause that could not learn a thread is not still waiting on
            // one, or F9 would never pause again.
            "threads" => {
                session.pausing = false;
                Vec::new()
            }
            _ => Vec::new(),
        };
    }
    match command.as_str() {
        "initialize" => {
            session.phase = Phase::Starting;
            let (request, args) = (session.request.clone(), session.args.clone());
            vec![ask(session, &request, Value::Object(args))]
        }
        "stackTrace" => {
            let frames = message["body"]["stackFrames"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|frame| Frame {
                    id: frame["id"].as_i64().unwrap_or_default(),
                    name: printable(frame["name"].as_str().unwrap_or_default()),
                    file: frame["source"]["path"]
                        .as_str()
                        .map(printable)
                        .map(PathBuf::from),
                    line: frame["line"].as_u64().unwrap_or_default() as usize,
                })
                .collect();
            let Phase::Paused(pause) = &mut session.phase else {
                return Vec::new();
            };
            pause.frames = frames;
            pause.chosen = 0;
            next.frames_selection = 0;
            inspect(next)
        }
        // One level of one reference. Appended, never replacing: the only
        // second request for a reference is its next page, and a page that
        // overwrote the one before it is a collection that never grows.
        "variables" => {
            let members: Vec<Member> = message["body"]["variables"]
                .as_array()
                .into_iter()
                .flatten()
                .map(member)
                .collect();
            let Phase::Paused(pause) = &mut session.phase else {
                return Vec::new();
            };
            let held = pause.children.entry(reference).or_default();
            held.extend(members);
            let fetched = held.len();
            pause.fetched.insert(reference, fetched);
            Vec::new()
        }
        // The chosen Frame's scopes become the top rows, each open unless the
        // adapter called it expensive — a scope it says costs something to
        // read is one nobody asked to read.
        "scopes" => {
            let scopes: Vec<(Member, bool)> = message["body"]["scopes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|scope| (member(scope), scope["expensive"] == Value::Bool(true)))
                .collect();
            let Phase::Paused(pause) = &mut session.phase else {
                return Vec::new();
            };
            pause.scopes = scopes.iter().map(|(scope, _)| scope.clone()).collect();
            pause.children.clear();
            pause.fetched.clear();
            let opened: Vec<(i64, usize)> = scopes
                .iter()
                .filter(|(_, expensive)| !expensive)
                .map(|(scope, _)| (scope.reference, scope.indexed))
                .collect();
            pause.open = opened.iter().map(|(reference, _)| *reference).collect();
            opened
                .into_iter()
                .map(|(reference, indexed)| fetch(session, reference, paged(indexed, 0)))
                .collect()
        }
        "threads" if session.pausing => {
            session.pausing = false;
            match message["body"]["threads"][0]["id"].as_i64() {
                Some(thread) => vec![ask(session, "pause", json!({ "threadId": thread }))],
                None => Vec::new(),
            }
        }
        "disconnect" => {
            end(next);
            vec![Effect::StopDap]
        }
        _ => Vec::new(),
    }
}

fn told(next: &mut State, message: &Value) -> Vec<Effect> {
    let body = &message["body"];
    match message["event"].as_str() {
        Some("initialized") => configured(next),
        Some("stopped") => {
            let session = next.debug.as_mut().expect("a session");
            let thread = body["threadId"].as_i64().unwrap_or_default();
            // Only a running program, or the thread being inspected, moves the
            // inspection: a second thread pausing leaves the view where it is,
            // and a session being stopped is not brought back.
            match &session.phase {
                Phase::Running(_) => {}
                Phase::Paused(pause) if pause.thread == thread => {}
                _ => return Vec::new(),
            }
            session.phase = Phase::Paused(Pause {
                thread,
                why: match body["reason"].as_str() {
                    Some("exception") => Why::Exception,
                    _ => Why::Paused,
                },
                exception: body["text"].as_str().map(printable),
                frames: Vec::new(),
                chosen: 0,
                scopes: Vec::new(),
                children: BTreeMap::new(),
                open: BTreeSet::new(),
                fetched: BTreeMap::new(),
            });
            let effect = ask(session, "stackTrace", json!({ "threadId": thread }));
            // The Debug group and the Frames come forward; the keyboard stays
            // where it was, since a pause is something the program did, not
            // the reader. What the reader shows instead is not moved again
            // until the next pause.
            next.corner = layout::Corner::Frames;
            next.strip = layout::Group::Debug;
            vec![effect]
        }
        Some("continued") => {
            let session = next.debug.as_mut().expect("a session");
            if let Phase::Paused(pause) = &session.phase {
                session.phase = Phase::Running(Some(pause.clone()));
            }
            Vec::new()
        }
        // The program is gone, so the adapter is told the session is over
        // and let go once it has answered, as a stop is: it may have more to
        // clean up than the program.
        Some("terminated") => {
            let session = next.debug.as_mut().expect("a session");
            if session.phase == Phase::Stopping {
                return Vec::new();
            }
            session.phase = Phase::Stopping;
            vec![ask(session, "disconnect", json!({}))]
        }
        _ => Vec::new(),
    }
}

/// `initialized`: the Breakpoints, the Exception filters and then
/// `configurationDone`, in the protocol's order and in one batch.
fn configured(next: &mut State) -> Vec<Effect> {
    let mut files: BTreeMap<&Path, Vec<usize>> = BTreeMap::new();
    for breakpoint in &next.breakpoints {
        files
            .entry(&breakpoint.file)
            .or_default()
            .push(breakpoint.line);
    }
    let files: Vec<(PathBuf, Vec<usize>)> = files
        .into_iter()
        .map(|(file, lines)| (file.to_path_buf(), lines))
        .collect();
    let session = next.debug.as_mut().expect("a session");
    if session.phase != Phase::Starting {
        return Vec::new();
    }
    let mut effects: Vec<Effect> = files
        .into_iter()
        .map(|(file, lines)| {
            let breakpoints: Vec<Value> =
                lines.iter().map(|line| json!({ "line": line })).collect();
            ask(
                session,
                "setBreakpoints",
                json!({ "source": { "path": file }, "breakpoints": breakpoints }),
            )
        })
        .collect();
    effects.push(ask(
        session,
        "setExceptionBreakpoints",
        json!({ "filters": [] }),
    ));
    effects.push(ask(session, "configurationDone", json!({})));
    session.phase = Phase::Running(None);
    effects
}

/// F9: continue the inspected thread while Paused, pause the program while
/// Running. The phase moves as the request goes, since a `continue` answered
/// is the adapter's word that it ran and a `continued` event is optional.
pub fn resume(next: &mut State) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    match &session.phase {
        Phase::Paused(pause) => {
            let (thread, last) = (pause.thread, pause.clone());
            session.phase = Phase::Running(Some(last));
            vec![ask(session, "continue", json!({ "threadId": thread }))]
        }
        // Pausing needs a thread, and a program that has never paused has
        // named none, so the adapter is asked for them first.
        Phase::Running(_) if !session.pausing => {
            session.pausing = true;
            vec![ask(session, "threads", json!({}))]
        }
        _ => Vec::new(),
    }
}

/// F8, F7 and Shift+F8, and the `n`, `i` and `o` chords: the inspected thread
/// runs on by one step. Only while Paused — a program that is running is
/// already between steps, and an adapter asked to step one that is not stopped
/// answers with an error.
///
/// The phase moves as the request goes, for `resume`'s reason: the program is
/// running until the `stopped` event says where it got to.
pub fn step(next: &mut State, step: Step) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    let Phase::Paused(pause) = &session.phase else {
        return Vec::new();
    };
    let (thread, last) = (pause.thread, pause.clone());
    let request = match step {
        Step::Over => "next",
        Step::Into => "stepIn",
        Step::Out => "stepOut",
    };
    session.phase = Phase::Running(Some(last));
    vec![ask(session, request, json!({ "threadId": thread }))]
}

/// Ctrl+F2: a launched program is terminated and an attached one left
/// running. A session already stopping is let go at once.
pub fn stop(next: &mut State) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    match session.phase {
        Phase::Stopping | Phase::Spawning => {
            end(next);
            vec![Effect::StopDap]
        }
        _ => {
            let terminate = session.request == "launch";
            session.phase = Phase::Stopping;
            vec![ask(
                session,
                "disconnect",
                json!({ "terminateDebuggee": terminate }),
            )]
        }
    }
}

/// The Frame at `index` of the Frames becomes the inspected one, and the
/// Paused line moves to its call.
pub fn choose(next: &mut State, index: usize) -> Vec<Effect> {
    let Some(Phase::Paused(pause)) = next.debug.as_mut().map(|s| &mut s.phase) else {
        return Vec::new();
    };
    if index >= pause.frames.len() {
        return Vec::new();
    }
    pause.chosen = index;
    next.frames_selection = index;
    inspect(next)
}

/// What is on screen from the last pause: the pause itself while one holds,
/// and the one it left behind while the program runs — the Frames and the
/// Variables stay drawn, dimmed, so a program that runs on does not blank the
/// panes the reader was reading.
fn showing(state: &State) -> Option<&Pause> {
    match state.debug.as_ref().map(|session| &session.phase) {
        Some(Phase::Paused(pause)) => Some(pause),
        Some(Phase::Running(last)) => last.as_ref(),
        _ => None,
    }
}

/// Whether what is on screen is the last pause rather than this one, which is
/// what draws it dimmed: nothing dimmed is mistaken for current.
pub fn stale(state: &State) -> bool {
    matches!(
        state.debug.as_ref().map(|session| &session.phase),
        Some(Phase::Running(Some(_)))
    )
}

/// What the Variables' title says: the mode the keyboard is in while Stepping
/// mode is on — it is four letters acting without their Space, and a mode
/// nobody can see they are in is a mode that swallows keys — then that the
/// program is running, and otherwise the pane's own name.
pub fn title(state: &State) -> &'static str {
    match (state.stepping, stale(state)) {
        (true, _) => "stepping",
        (false, true) => "running",
        (false, false) => "variables",
    }
}

/// The Frames the Corner lists, empty while no pause has anything to show.
pub fn frames(state: &State) -> &[Frame] {
    showing(state).map_or(&[], |pause| &pause.frames)
}

/// The Variables as they are drawn: the exception that paused the program if
/// one did, then the scopes, and under each open row the children that have
/// arrived — the tree flattened to what is open, and nothing that is not.
pub fn variables(state: &State) -> Vec<Row> {
    let Some(pause) = showing(state) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if let Some(text) = &pause.exception {
        rows.push(Row {
            name: "exception".to_string(),
            value: text.clone(),
            depth: 0,
            hint: Hint::Plain,
            open: false,
            opens: Opens::Nothing,
        });
    }
    for scope in &pause.scopes {
        draw(pause, scope, 0, &mut Vec::new(), &mut rows);
    }
    rows
}

/// One member and, while it is open, everything under it — then the row that
/// stands for the rest of a collection whose page has not been asked for.
///
/// `walked` is what stands above this member, and a reference already on it is
/// not opened again: the references are the adapter's, which is untrusted
/// input, and one that holds itself would otherwise be a structure the reader
/// could open into a stack overflow.
fn draw(pause: &Pause, member: &Member, depth: usize, walked: &mut Vec<i64>, rows: &mut Vec<Row>) {
    let open = member.reference != 0
        && pause.open.contains(&member.reference)
        && !walked.contains(&member.reference);
    rows.push(Row {
        name: member.name.clone(),
        value: member.value.clone(),
        depth,
        hint: member.hint,
        open,
        opens: match member.reference {
            0 => Opens::Nothing,
            reference => Opens::Children {
                reference,
                indexed: member.indexed,
            },
        },
    });
    if !open {
        return;
    }
    walked.push(member.reference);
    for child in pause.children.get(&member.reference).into_iter().flatten() {
        draw(pause, child, depth + 1, walked, rows);
    }
    walked.pop();
    let fetched = pause
        .fetched
        .get(&member.reference)
        .copied()
        .unwrap_or_default();
    if fetched < member.indexed {
        rows.push(Row {
            name: format!("{} more", member.indexed - fetched),
            value: String::new(),
            depth: depth + 1,
            hint: Hint::Plain,
            open: false,
            opens: Opens::NextPage {
                reference: member.reference,
                start: fetched,
            },
        });
    }
}

/// Enter on a Variables row, and a click on one: a member with children is
/// opened or closed, and the row that stands for a collection's next page asks
/// for it. Opening asks for one level — the children of that reference and
/// nothing under them — so walking a deep structure asks for what is opened
/// and nothing else.
pub fn open(next: &mut State, index: usize) -> Vec<Effect> {
    let Some(row) = variables(next).get(index).cloned() else {
        return Vec::new();
    };
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    let Phase::Paused(pause) = &mut session.phase else {
        return Vec::new();
    };
    match row.opens {
        Opens::Nothing => Vec::new(),
        Opens::Children { reference, indexed } => {
            if !pause.open.insert(reference) {
                pause.open.remove(&reference);
                return Vec::new();
            }
            // Asked for once: a row closed and opened again shows what already
            // arrived, since the values have not changed while the program is
            // stopped.
            match pause.children.contains_key(&reference) {
                true => Vec::new(),
                false => vec![fetch(session, reference, paged(indexed, 0))],
            }
        }
        // A next-page row exists only over a collection that is being read a
        // page at a time, so where it carries on from is the page to ask for.
        Opens::NextPage { reference, start } => vec![fetch(session, reference, Some(start))],
    }
}

/// Which page of a collection of `indexed` members to ask for, starting at
/// `start` — none at all for one small enough to come whole, since an adapter
/// answering a whole small scope is one round trip rather than two.
fn paged(indexed: usize, start: usize) -> Option<usize> {
    (indexed > PAGE).then_some(start)
}

/// One level of `reference`: the page named, or everything it holds.
fn fetch(session: &mut Session, reference: i64, page: Option<usize>) -> Effect {
    let arguments = match page {
        Some(start) => json!({ "variablesReference": reference, "start": start, "count": PAGE }),
        None => json!({ "variablesReference": reference }),
    };
    ask(session, "variables", arguments)
}

/// One member of the Variables, as the adapter worded it — with nothing in it
/// that could drive the terminal it is about to be drawn on, for the reason a
/// Frame's name is stripped.
fn member(value: &Value) -> Member {
    let attributes = value["presentationHint"]["attributes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<&str>>();
    let hint = if value["presentationHint"]["visibility"] == "private" {
        Hint::Private
    } else if attributes.contains(&"lazy") {
        Hint::Lazy
    } else if attributes.contains(&"readOnly") {
        Hint::ReadOnly
    } else {
        Hint::Plain
    };
    Member {
        name: printable(value["name"].as_str().unwrap_or_default()),
        value: printable(value["value"].as_str().unwrap_or_default()),
        reference: value["variablesReference"].as_i64().unwrap_or_default(),
        hint,
        indexed: value["indexedVariables"].as_u64().unwrap_or_default() as usize,
    }
}

/// Where the program is paused, as the chosen Frame names it, and why.
pub fn paused_line(state: &State) -> Option<(&Path, usize, Why)> {
    let Some(Phase::Paused(pause)) = state.debug.as_ref().map(|session| &session.phase) else {
        return None;
    };
    let frame = pause.frames.get(pause.chosen)?;
    Some((frame.file.as_deref()?, frame.line, pause.why))
}

/// The whole inspection moved to the chosen Frame: its file brought on screen
/// at its line — unless it is the Buffer already there, since the cursor of
/// the file being typed in is the reader's and a pause never moves it — and
/// its scopes asked for, which is what the Variables draw.
fn inspect(next: &mut State) -> Vec<Effect> {
    let mut effects = match paused_line(next) {
        Some((path, line, _)) if next.current_buffer.as_deref() != Some(path) => {
            vec![Effect::OpenAt {
                path: path.to_path_buf(),
                at: Place { line, column: 1 },
            }]
        }
        _ => Vec::new(),
    };
    // Asked for here rather than at the pause, because choosing another Frame
    // is the same question asked about another call.
    let Some(session) = next.debug.as_mut() else {
        return effects;
    };
    let Phase::Paused(pause) = &session.phase else {
        return effects;
    };
    if let Some(frame) = pause.frames.get(pause.chosen) {
        let id = frame.id;
        effects.push(ask(session, "scopes", json!({ "frameId": id })));
    }
    effects
}

/// What the Corner holds outside a session: what it held before one began,
/// which is what ending the session gives back and so what a restart, which
/// has no session, should show.
pub fn resting_corner(state: &State) -> layout::Corner {
    state
        .debug
        .as_ref()
        .map_or(state.corner, |session| session.corner)
}

/// The session ends: the Corner gets back what it held before, and the
/// keyboard leaves a pane that is going. Stepping mode goes with it — the
/// letters it claims do nothing without a session, and one that swallowed
/// them for nothing would be a mode nobody could see they were in.
fn end(next: &mut State) {
    next.corner = resting_corner(next);
    // The Strip the same way, which is the whole of what a session borrowed:
    // both slots go back to what they held before it began.
    next.strip = next
        .debug
        .as_ref()
        .map_or(next.strip, |session| session.strip);
    next.debug = None;
    next.stepping = false;
    // The mark is a claim that the Program output printed something nobody
    // has read; with the session gone there is nothing left to show, so it
    // goes too rather than standing over the next session's tab.
    next.output_unseen = false;
    // Both of the session's own panes go with it, so the keyboard is never
    // left in one that is no longer on screen.
    if matches!(next.focus, Pane::Frames | Pane::Variables) {
        next.focus = Pane::Editor;
    }
}

/// The adapter's words with nothing left in them that could drive the
/// terminal they are about to be drawn on.
fn printable(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

/// One request, numbered and remembered until its answer arrives.
fn ask(session: &mut Session, command: &str, arguments: Value) -> Effect {
    session.seq += 1;
    // Read off the request rather than passed beside it: the two could then
    // name different references, and the answer would be filed under the one
    // nobody asked.
    session.asked.insert(
        session.seq,
        Ask {
            command: command.to_string(),
            reference: arguments["variablesReference"].as_i64().unwrap_or_default(),
        },
    );
    Effect::DapSend {
        json: json!({
            "seq": session.seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        })
        .to_string(),
    }
}

/// The answer to a request the adapter made of Varde.
fn reply(session: &mut Session, request: &Value, mut answer: Value) -> Effect {
    session.seq += 1;
    answer["seq"] = json!(session.seq);
    answer["type"] = json!("response");
    answer["request_seq"] = request["seq"].clone();
    Effect::DapSend {
        json: answer.to_string(),
    }
}

/// The `seq` a request still waiting for its answer went out with, so a test
/// can answer it the way the adapter would.
#[cfg(test)]
pub(crate) fn outstanding(state: &State, command: &str) -> i64 {
    let session = state.debug.as_ref().expect("a session");
    *session
        .asked
        .iter()
        .find(|(_, asked)| asked.command == command)
        .unwrap_or_else(|| panic!("nothing is waiting on {command:?}"))
        .0
}

/// `state` with a session Paused on thread 1 in `main` at line 1 of
/// `/w/one.rs`, driven there through the protocol a real adapter speaks, for
/// the tests outside this module that need one.
#[cfg(test)]
pub(crate) fn paused(mut state: State) -> State {
    state.adapters.insert(
        "rust".to_string(),
        crate::startup::Adapter {
            command: "adapter".to_string(),
            args: Vec::new(),
            install: BTreeMap::new(),
        },
    );
    state.launches.insert(
        "app".to_string(),
        crate::startup::Launch {
            adapter: "rust".to_string(),
            request: "launch".to_string(),
            args: serde_json::Map::new(),
        },
    );
    start(&mut state, "app");
    started(&mut state);
    let seq = outstanding(&state, "initialize");
    received(
        &mut state,
        &json!({"type": "response", "request_seq": seq, "success": true, "command": "initialize"})
            .to_string(),
    );
    received(&mut state, r#"{"type":"event","event":"initialized"}"#);
    received(
        &mut state,
        r#"{"type":"event","event":"stopped","body":{"threadId":1,"reason":"breakpoint"}}"#,
    );
    let seq = outstanding(&state, "stackTrace");
    received(
        &mut state,
        &json!({"type": "response", "request_seq": seq, "success": true, "command": "stackTrace",
            "body": {"stackFrames": [{"id": 1, "name": "main", "line": 1, "source": {"path": "/w/one.rs"}}]}})
        .to_string(),
    );
    state
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

    /// The requests `effects` sent, as the adapter would read them.
    fn sent(effects: &[Effect]) -> Vec<Value> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::DapSend { json } => serde_json::from_str(json).ok(),
                _ => None,
            })
            .collect()
    }

    /// A reverse request nothing answers yet is refused out loud, naming the
    /// request it answers: an adapter left waiting is a session that hangs.
    #[test]
    fn a_request_from_the_adapter_is_refused_rather_than_ignored() {
        let mut state = paused(State::default());
        let effects = received(
            &mut state,
            r#"{"seq":40,"type":"request","command":"runInTerminal","arguments":{}}"#,
        );
        let reply = &sent(&effects)[0];
        assert_eq!(reply["type"], "response");
        assert_eq!(reply["request_seq"], 40);
        assert_eq!(reply["success"], false);
        assert_eq!(reply["command"], "runInTerminal");
    }

    /// An answer to nothing Varde asked is not taken for an answer to
    /// something it did.
    #[test]
    fn a_response_to_no_request_changes_nothing() {
        let mut state = paused(State::default());
        let before = state.clone();
        let effects = received(
            &mut state,
            r#"{"type":"response","request_seq":999,"success":false,"command":"launch"}"#,
        );
        assert_eq!(effects, vec![]);
        assert_eq!(state, before);
    }

    /// The adapter's own words reach the footer, and nothing in them can
    /// drive the terminal they are drawn on.
    #[test]
    fn a_refused_launch_says_why_without_the_escapes() {
        let mut state = State::default();
        state.adapters.insert(
            "rust".to_string(),
            crate::startup::Adapter {
                command: "adapter".to_string(),
                args: Vec::new(),
                install: BTreeMap::new(),
            },
        );
        state.launches.insert(
            "app".to_string(),
            crate::startup::Launch {
                adapter: "rust".to_string(),
                request: "launch".to_string(),
                args: serde_json::Map::new(),
            },
        );
        start(&mut state, "app");
        started(&mut state);
        received(
            &mut state,
            r#"{"type":"response","request_seq":1,"success":true,"command":"initialize"}"#,
        );
        let effects = received(
            &mut state,
            "{\"type\":\"response\",\"request_seq\":2,\"success\":false,\"command\":\"launch\",\"message\":\"no \\u001b[2Jprogram\"}",
        );
        assert_eq!(
            effects,
            vec![
                Effect::notify_about("launch-failed", "no [2Jprogram".to_string()),
                Effect::StopDap
            ]
        );
        assert_eq!(state.debug, None);
        assert_eq!(
            state.refusal,
            Some(Refusal::LaunchFailed("no [2Jprogram".to_string()))
        );
    }

    /// Pausing a program that has never paused needs a thread nobody has
    /// named yet, so the adapter is asked for them and the first is paused.
    /// A second F9 before the answer asks nothing twice.
    #[test]
    fn pausing_a_program_that_never_paused_asks_for_its_threads_first() {
        let mut state = paused(State::default());
        resume(&mut state);
        let asked = sent(&resume(&mut state));
        assert_eq!(asked[0]["command"], "threads");
        assert_eq!(resume(&mut state), vec![]);
        let seq = asked[0]["seq"].clone();
        let effects = received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": true, "command": "threads",
                "body": {"threads": [{"id": 7, "name": "worker"}]}})
            .to_string(),
        );
        let pause = &sent(&effects)[0];
        assert_eq!(pause["command"], "pause");
        assert_eq!(pause["arguments"]["threadId"], 7);
    }

    /// Stopping waits for `disconnect`'s answer, and a second stop does not:
    /// an adapter that never answers is not a session nobody can end.
    #[test]
    fn a_second_stop_lets_the_adapter_go_at_once() {
        let mut state = paused(State::default());
        let asked = sent(&stop(&mut state));
        assert_eq!(asked[0]["command"], "disconnect");
        assert!(state.debug.is_some());
        assert_eq!(stop(&mut state), vec![Effect::StopDap]);
        assert_eq!(state.debug, None);
    }

    /// An `initialized` ahead of `initialize`'s answer sends nothing: the
    /// Breakpoints wait for the program they are for to have been launched.
    #[test]
    fn nothing_is_configured_before_the_program_is_launched() {
        let mut state = State::default();
        state.adapters.insert(
            "rust".to_string(),
            crate::startup::Adapter {
                command: "adapter".to_string(),
                args: Vec::new(),
                install: BTreeMap::new(),
            },
        );
        state.launches.insert(
            "app".to_string(),
            crate::startup::Launch {
                adapter: "rust".to_string(),
                request: "launch".to_string(),
                args: serde_json::Map::new(),
            },
        );
        start(&mut state, "app");
        started(&mut state);
        let effects = received(&mut state, r#"{"type":"event","event":"initialized"}"#);
        assert_eq!(effects, vec![]);
    }

    /// A second thread stopping leaves the inspected one where it is, and a
    /// session being stopped is not paused back into life.
    #[test]
    fn only_the_inspected_thread_or_a_running_program_moves_the_pause() {
        let mut state = paused(State::default());
        let other =
            r#"{"type":"event","event":"stopped","body":{"threadId":2,"reason":"breakpoint"}}"#;
        assert_eq!(received(&mut state, other), vec![]);
        assert_eq!(paused_line(&state).map(|(_, line, _)| line), Some(1));
        stop(&mut state);
        assert_eq!(received(&mut state, other), vec![]);
        assert_eq!(
            state.debug.map(|session| session.phase),
            Some(Phase::Stopping)
        );
    }

    /// A `threads` that failed leaves F9 able to ask again.
    #[test]
    fn a_failed_threads_request_does_not_leave_pausing_stuck() {
        let mut state = paused(State::default());
        resume(&mut state);
        let seq = sent(&resume(&mut state))[0]["seq"].clone();
        received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": false, "command": "threads"})
                .to_string(),
        );
        assert_eq!(sent(&resume(&mut state))[0]["command"], "threads");
    }

    /// The program ending is told to the adapter with `disconnect`, and the
    /// adapter let go once it answers.
    #[test]
    fn a_terminated_program_disconnects_before_the_adapter_goes() {
        let mut state = paused(State::default());
        let asked = sent(&received(
            &mut state,
            r#"{"type":"event","event":"terminated"}"#,
        ));
        assert_eq!(asked[0]["command"], "disconnect");
        let effects = received(
            &mut state,
            &json!({"type": "response", "request_seq": asked[0]["seq"], "success": true, "command": "disconnect"})
                .to_string(),
        );
        assert_eq!(effects, vec![Effect::StopDap]);
        assert_eq!(state.debug, None);
    }

    /// The Corner a restart shows is the one the session will give back, not
    /// the Frames it borrowed.
    #[test]
    fn the_corner_at_rest_is_the_one_held_before_the_session() {
        let state = paused(State {
            corner: layout::Corner::Buffers,
            ..State::default()
        });
        assert_eq!(state.corner, layout::Corner::Frames);
        assert_eq!(resting_corner(&state), layout::Corner::Buffers);
    }

    /// A reference that holds itself is the adapter's word and the adapter is
    /// untrusted input: opened, it would be walked forever. It is drawn once
    /// and its second appearance is a closed row.
    #[test]
    fn a_reference_that_holds_itself_is_drawn_once() {
        let mut state = paused(State::default());
        let seq = outstanding(&state, "scopes");
        received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": true, "command": "scopes",
                "body": {"scopes": [{"name": "Locals", "variablesReference": 1}]}})
            .to_string(),
        );
        let seq = outstanding(&state, "variables");
        received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": true, "command": "variables",
                "body": {"variables": [{"name": "itself", "variablesReference": 1}]}})
            .to_string(),
        );
        let rows: Vec<String> = variables(&state).into_iter().map(|row| row.name).collect();
        assert_eq!(rows, ["Locals", "itself"]);
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
