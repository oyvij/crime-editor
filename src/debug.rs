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
use std::collections::BTreeMap;
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
    /// The `seq` the last request went out with, and the command of every one
    /// still unanswered, by `seq`: a response names its request only by number.
    seq: i64,
    asked: BTreeMap<i64, String>,
    /// A pause asked for before any thread was known, so it waits on the
    /// `threads` answer that names one.
    pausing: bool,
    /// What the Corner held when the session began, given back when it ends.
    corner: layout::Corner,
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
    Running,
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
    /// Empty until the adapter answers `stackTrace`, which the pause asks for.
    pub frames: Vec<Frame>,
    pub chosen: usize,
}

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
        // A reverse request nothing here answers yet is refused out loud: an
        // adapter left waiting on a reply is a session that hangs.
        Some("request") => {
            let session = next.debug.as_mut().expect("a session");
            let command = message["command"].as_str().unwrap_or_default();
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
    let Some(command) = message["request_seq"]
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
            shown(next)
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
                Phase::Running => {}
                Phase::Paused(pause) if pause.thread == thread => {}
                _ => return Vec::new(),
            }
            session.phase = Phase::Paused(Pause {
                thread,
                why: match body["reason"].as_str() {
                    Some("exception") => Why::Exception,
                    _ => Why::Paused,
                },
                frames: Vec::new(),
                chosen: 0,
            });
            let effect = ask(session, "stackTrace", json!({ "threadId": thread }));
            // The Frames come forward; the keyboard stays where it was, since
            // a pause is something the program did, not the reader.
            next.corner = layout::Corner::Frames;
            vec![effect]
        }
        Some("continued") => {
            let session = next.debug.as_mut().expect("a session");
            if matches!(session.phase, Phase::Paused(_)) {
                session.phase = Phase::Running;
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
    session.phase = Phase::Running;
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
            let thread = pause.thread;
            session.phase = Phase::Running;
            vec![ask(session, "continue", json!({ "threadId": thread }))]
        }
        // Pausing needs a thread, and a program that has never paused has
        // named none, so the adapter is asked for them first.
        Phase::Running if !session.pausing => {
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
    let thread = pause.thread;
    let request = match step {
        Step::Over => "next",
        Step::Into => "stepIn",
        Step::Out => "stepOut",
    };
    session.phase = Phase::Running;
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
    shown(next)
}

/// The Frames the Corner lists, empty while nothing is Paused.
pub fn frames(state: &State) -> &[Frame] {
    match state.debug.as_ref().map(|session| &session.phase) {
        Some(Phase::Paused(pause)) => &pause.frames,
        _ => &[],
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

/// The chosen Frame's file brought on screen at its line, unless it is the
/// Buffer already there: the cursor of the file being typed in is the
/// reader's, and a pause never moves it.
fn shown(next: &State) -> Vec<Effect> {
    match paused_line(next) {
        Some((path, line, _)) if next.current_buffer.as_deref() != Some(path) => {
            vec![Effect::OpenAt {
                path: path.to_path_buf(),
                at: Place { line, column: 1 },
            }]
        }
        _ => Vec::new(),
    }
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
    next.debug = None;
    next.stepping = false;
    if next.focus == Pane::Frames {
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
    session.asked.insert(session.seq, command.to_string());
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
    let asked = |state: &State, command: &str| {
        let session = state.debug.as_ref().expect("a session");
        *session
            .asked
            .iter()
            .find(|(_, asked)| *asked == command)
            .expect("asked")
            .0
    };
    let seq = asked(&state, "initialize");
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
    let seq = asked(&state, "stackTrace");
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
