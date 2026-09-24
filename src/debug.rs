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
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
pub const RESUME: &str = "debug-resume";
pub const STEP_OVER: &str = "debug-step-over";
pub const STEP_INTO: &str = "debug-step-into";
pub const STEP_OUT: &str = "debug-step-out";
pub const STOP: &str = "debug-stop";
pub const RESTART: &str = "debug-restart";
pub const ASK_AI: &str = "debug-ask-ai";
pub const SET_VALUE: &str = "debug-set-value";
pub const COPY_VALUE: &str = "debug-copy-value";
pub const COPY_EXPRESSION: &str = "debug-copy-expression";
pub const WATCH: &str = "debug-watch";
pub const REMOVE_WATCH: &str = "debug-remove-watch";
pub const EVALUATE: &str = "debug-evaluate";
/// The row's own, not the Transport's: one asks the AI about the whole pause
/// and the other about one value, so they are two actions wearing one name.
pub const ROW_ASK_AI: &str = "debug-ask-ai-value";
pub const NEXT_THREAD: &str = "debug-next-thread";

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

/// The Chips on the Variables' top border: every debug action, then the
/// Program output's. One control per action and never two — continue and
/// pause are one Chip named for what pressing it does, the way the Reading's
/// play is and for the same reason.
///
/// The Debug group's border carries the whole set while a session exists,
/// since that is when their keys are reserved. With the Shell group up — which
/// is where a session that ended leaves the Strip — one Chip is left: restart,
/// while there is a configuration to rerun. A control reachable only while
/// the thing it restarts is running is a control nobody can press, and the
/// keyboard's own `C-F5` has the same reach.
///
/// What the Transport holds, never where it is drawn: `crate::showing_transport`
/// is that, and `ui` and `mouse` read the one answer.
///
/// Glyphs are geometric and one cell wide in every font (ADR 0022): no emoji,
/// whose width terminals disagree about, and every column to the right of a
/// two-cell glyph is a click landing where nobody pointed.
pub fn strip_transport(state: &crate::State) -> Vec<crate::Chip> {
    use crate::{Chip, Hue, Tone};
    let chip = |action, name, glyph: &str, keys, hue, dimmed| Chip {
        action,
        name,
        glyph: glyph.to_string(),
        keys,
        hue,
        // Lit ahead of dimmed: a step taken leaves the program running, so the
        // Chip that was just pressed is dimmed the instant it acts and would
        // otherwise never be seen lit at all.
        tone: match (state.transport_lit == Some(action), dimmed) {
            (true, _) => Tone::Lit,
            (false, true) => Tone::Dimmed,
            (false, false) => Tone::Plain,
        },
    };
    let restart = |dimmed| {
        chip(
            RESTART,
            "restart",
            "\u{21bb}",
            "C-F5 \u{2423}r",
            Hue::Go,
            dimmed,
        )
    };
    let mut chips = Vec::new();
    if let Some(session) = state.debug.as_ref() {
        let running = matches!(session.phase, Phase::Running(_));
        let stepping = !matches!(session.phase, Phase::Paused(_));
        // Nothing to continue or pause until the program is one of the two:
        // a session still spawning or already stopping answers neither.
        let between = !matches!(session.phase, Phase::Paused(_) | Phase::Running(_));
        chips.extend([
            match running {
                true => chip(
                    RESUME,
                    "pause",
                    "\u{2016}",
                    "F9 \u{2423}c",
                    Hue::Hold,
                    between,
                ),
                false => chip(
                    RESUME,
                    "continue",
                    "\u{25ba}",
                    "F9 \u{2423}c",
                    Hue::Go,
                    between,
                ),
            },
            // Stepping is only ever asked of a stopped thread: an adapter sent
            // a step while the program runs answers with an error, so the
            // Chips say so rather than the reader finding out from the footer.
            chip(
                STEP_OVER,
                "step-over",
                "\u{293c}",
                "F8 \u{2423}n",
                Hue::Step,
                stepping,
            ),
            chip(
                STEP_INTO,
                "step-into",
                "\u{2913}",
                "F7 \u{2423}i",
                Hue::Step,
                stepping,
            ),
            chip(
                STEP_OUT,
                "step-out",
                "\u{2912}",
                "S-F8 \u{2423}o",
                Hue::Step,
                stepping,
            ),
            chip(STOP, "stop", "\u{25a0}", "C-F2 \u{2423}q", Hue::Halt, false),
            // Dimmed while a session exists: restarting a live one is stop
            // and start again, which nothing specifies yet, so today it would
            // refuse with `debug-session-running` — and a Chip that refuses is
            // a Chip that lies. What it is for is the session that ended,
            // below.
            restart(true),
            // The last two name no key and do nothing yet: `\u{2423}a` is issue
            // #70's to bind and the thread to jump to is #65's to count, and a
            // Chip teaching a key nobody bound is the cheatsheet contract
            // broken from the other end. Dimmed until then, because a dimmed
            // Chip does nothing and that is exactly what these do.
            chip(ASK_AI, "ask-ai", "\u{2736}", "", Hue::Plain, true),
            chip(NEXT_THREAD, "next-thread", "\u{21c9}", "", Hue::Plain, true),
        ]);
    }
    if state.debug.is_none() && state.last_launch.is_some() {
        chips.push(restart(false));
    }
    if state.output_running {
        chips.push(Chip {
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
            hue: Hue::Plain,
            tone: match state.output_unseen {
                true => Tone::Marked,
                false => Tone::Plain,
            },
        });
    }
    chips
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
    /// What the locals held at the pause before this one, by name, under the
    /// name of the Frame they belong to — and `None` until a pause has been
    /// left behind. This is what an Inline value is marked as changed against,
    /// and the Frame's name travels with them because only the *inspected*
    /// Frame's scopes are ever fetched: without it, choosing an outer Frame
    /// would diff one call's locals against another's and mark names that
    /// never moved. On the Session rather than on the Pause, because the
    /// question is about the pause that is gone: a Pause carrying it would
    /// have to be handed its predecessor's copy to build itself.
    previous: Option<(String, BTreeMap<String, String>)>,
    /// Whether the adapter said it can write a member back. Its word, never
    /// a try: a set-value Chip that looked enabled and failed teaches the
    /// reader nothing, so the capability dims it instead.
    can_set: bool,
    /// Whether it said a request can be taken back, which is the only way a
    /// Snippet that has not returned can be stopped. Its word for the reason
    /// `can_set` is its word: a cancel Chip that looked enabled and did
    /// nothing is worse than one that says it cannot.
    can_cancel: bool,
    /// What the Corner and the Strip held when the session began, given back
    /// when it ends.
    corner: layout::Corner,
    strip: layout::Group,
}

/// A request waiting for its answer: the command, which is how the response is
/// read, and the arguments it went out with. The whole arguments and not the
/// one field each arm wants, because a response carries almost nothing of its
/// question — a `variables` answer is a list of members belonging to nothing,
/// an `evaluate` answer is a string belonging to no expression, and a
/// `setVariable` answer names neither the member it wrote nor what held it.
/// One field per arm is three fields that travel together and a fourth on the
/// next command; the request already says all of it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ask {
    command: String,
    arguments: Value,
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
    /// The path to this row in the program's own language — `orders[0].id`
    /// rather than `id` — which is what copying it as an expression puts on
    /// the clipboard and what watching it adds. Built as the tree is
    /// flattened, because only the walk knows what stands above a row; empty
    /// for a scope, which is a heading and not an expression.
    pub expression: String,
    /// The reference of whatever holds this row, which is how `setVariable`
    /// names a member: the protocol asks for the container and the member's
    /// name, never for the member's own reference.
    pub parent: i64,
    pub of: Of,
}

/// What a row stands for, which is what its Chips act on: a member is
/// watched where a Watch is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Of {
    /// A member of the adapter's tree, or the exception that paused the
    /// program: both are the adapter's word about the program.
    Member,
    /// A Watch, by its place in the Watches — and whether its expression
    /// calls something, since a Watch runs that call again at every pause,
    /// and whether the last pause could not evaluate it.
    Watch {
        index: usize,
        calling: bool,
        failed: bool,
    },
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

/// One expression kept at the top of the Variables and re-evaluated at every
/// pause. Core state rather than the session's: a Watch is a question the
/// reader is asking of the program, and the next session is asked the same
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub expression: String,
    pub answer: Answer,
}

/// What the last pause's `evaluate` said about a Watch. The adapter's reason
/// is kept apart from a value because a reason drawn as a value reads as the
/// program's own answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Asked and not yet answered, which is also where a Watch starts.
    Waiting,
    Value(String),
    Failed(String),
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
    // Remembered before the session exists and kept after it ends: what
    // restart reruns is the configuration, not the session.
    next.last_launch = Some(name.to_string());
    next.debug = Some(Session {
        adapter: launch.adapter.clone(),
        command: adapter.command.clone(),
        phase: Phase::Spawning,
        request: launch.request.clone(),
        args: launch.args.clone(),
        seq: 0,
        asked: BTreeMap::new(),
        pausing: false,
        previous: None,
        can_set: false,
        can_cancel: false,
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
    let mut effects = end(next);
    effects.extend(notice);
    effects
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
    let answering = message["request_seq"].as_i64().unwrap_or_default();
    let Some(Ask { command, arguments }) = session.asked.remove(&answering) else {
        return Vec::new();
    };
    if message["success"] != Value::Bool(true) {
        return match command.as_str() {
            "initialize" | "launch" | "attach" => {
                let why = adapter_error(message, &command);
                let mut effects = end(next);
                next.refusal = Some(Refusal::LaunchFailed(why.clone()));
                // And in the status line, for the reason `gone` says it twice.
                effects.extend([Effect::notify_about("launch-failed", why), Effect::StopDap]);
                effects
            }
            // The answer that lets the adapter go, whatever it says.
            "disconnect" => {
                let mut effects = end(next);
                effects.push(Effect::StopDap);
                effects
            }
            // A pause that could not learn a thread is not still waiting on
            // one, or F9 would never pause again.
            "threads" => {
                session.pausing = false;
                Vec::new()
            }
            // The adapter's reason, kept against the Watch that asked: a
            // Watch that silently showed nothing is a Watch the reader reads
            // as false rather than as unanswerable.
            "evaluate" => {
                let why = adapter_error(message, &command);
                // The Evaluator first, and told apart by the context the
                // request went out in for the reason a Hover is: the reader
                // asked out loud, so the adapter's words are shown as they
                // came and never softened into a blank output.
                if let Some(ran) = ran_asked(next, &arguments, answering) {
                    ran.answer = Ran::Failed(why);
                    return Vec::new();
                }
                match hover_asked(next, &arguments) {
                    Some(hovered) => hovered.held = Held::Failed(why),
                    None => {
                        if let Some(watch) = watch_asked(next, &arguments) {
                            watch.answer = Answer::Failed(why);
                        }
                    }
                }
                Vec::new()
            }
            // Said out loud, for the reason a launch that failed is: a value
            // the program would not take and nothing on screen to say so is
            // a set that looks as though it worked.
            "setVariable" => {
                next.refusal = Some(Refusal::SetValueFailed(adapter_error(message, &command)));
                Vec::new()
            }
            _ => Vec::new(),
        };
    }
    match command.as_str() {
        "initialize" => {
            session.can_set = message["body"]["supportsSetVariable"] == Value::Bool(true);
            session.can_cancel = message["body"]["supportsCancelRequest"] == Value::Bool(true);
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
            let reference = arguments["variablesReference"].as_i64().unwrap_or_default();
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
        // A Watch's value, filed under the expression that asked for it — or
        // the Hover's, told apart by the context the request went out in and
        // never by the expression, since a Watch on what the pointer is
        // resting on is one expression with two places to be.
        "evaluate" => {
            let value = printable(message["body"]["result"].as_str().unwrap_or_default());
            let reference = message["body"]["variablesReference"]
                .as_i64()
                .unwrap_or_default();
            let indexed = message["body"]["indexedVariables"]
                .as_u64()
                .unwrap_or_default() as usize;
            if let Some(ran) = ran_asked(next, &arguments, answering) {
                ran.answer = Ran::Value {
                    value,
                    reference,
                    indexed,
                };
                return Vec::new();
            }
            match hover_asked(next, &arguments) {
                Some(hovered) => {
                    hovered.held = Held::Value {
                        value,
                        reference,
                        indexed,
                    }
                }
                None => {
                    if let Some(watch) = watch_asked(next, &arguments) {
                        watch.answer = Answer::Value(value);
                    }
                }
            }
            Vec::new()
        }
        // The adapter's new value replaces the row's, rather than Varde
        // assuming what it wrote: an adapter is free to coerce what it was
        // given, and the row has to show what the program now holds.
        // Filed against the member the request named, in the container it
        // named: matching on the name alone rewrites every `id` in the tree,
        // and reading the selection back would file the answer wherever the
        // keyboard has got to since.
        "setVariable" => {
            let value = printable(message["body"]["value"].as_str().unwrap_or_default());
            let reference = arguments["variablesReference"].as_i64().unwrap_or_default();
            let name = arguments["name"].as_str().unwrap_or_default();
            let Some(Phase::Paused(pause)) = next.debug.as_mut().map(|s| &mut s.phase) else {
                return Vec::new();
            };
            let held = match pause.children.get_mut(&reference) {
                Some(held) => held,
                // A member of a scope rather than of an opened row: the
                // scopes are the one list that is not under a reference.
                None => &mut pause.scopes,
            };
            if let Some(member) = held.iter_mut().find(|member| member.name == name) {
                member.value = value;
            }
            Vec::new()
        }
        "threads" if session.pausing => {
            session.pausing = false;
            match message["body"]["threads"][0]["id"].as_i64() {
                Some(thread) => vec![ask(session, "pause", json!({ "threadId": thread }))],
                None => Vec::new(),
            }
        }
        "disconnect" => {
            let mut effects = end(next);
            effects.push(Effect::StopDap);
            effects
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
            // What the pause now ending held, kept for the one pause that
            // follows it: taken here, where the old Pause is still whole, and
            // never from the new one, which has no values yet.
            let ending = match &session.phase {
                Phase::Paused(pause) => Some(pause),
                Phase::Running(last) => last.as_ref(),
                _ => None,
            };
            session.previous = ending.map(|pause| {
                let frame = pause
                    .frames
                    .get(pause.chosen)
                    .map(|frame| frame.name.clone())
                    .unwrap_or_default();
                (frame, locals(pause))
            });
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
        // An adapter may learn what it can do after `initialize` answered —
        // a language plugin loading, a program attached to — and says so with
        // this event. Read into the same field the initialize reply sets, so
        // the set-value Chip has one author.
        Some("capabilities") => {
            let session = next.debug.as_mut().expect("a session");
            if let Some(can_set) = body["capabilities"]["supportsSetVariable"].as_bool() {
                session.can_set = can_set;
            }
            if let Some(can_cancel) = body["capabilities"]["supportsCancelRequest"].as_bool() {
                session.can_cancel = can_cancel;
            }
            Vec::new()
        }
        // What the program printed, as the adapter repeats it. It goes to the
        // Evaluator while a Snippet is in flight, because output during a run
        // is that run's: the Program output is the pty's own and keeps its
        // copy either way, so this takes nothing away from it.
        Some("output") => {
            let text = body["output"].as_str().unwrap_or_default();
            if let Some(ran) = in_flight(next) {
                ran.printed.push(printable(text.trim_end_matches('\n')));
            }
            Vec::new()
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
    let effects = match &session.phase {
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
    };
    lit(next, RESUME, &effects);
    effects
}

/// The Chip the action just taken belongs to, lit until another is taken —
/// but only where it acted: a Chip lit for a request that never went out says
/// something happened.
fn lit(next: &mut State, action: &'static str, effects: &[Effect]) {
    if !effects.is_empty() {
        next.transport_lit = Some(action);
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
    let effects = vec![ask(session, request, json!({ "threadId": thread }))];
    lit(
        next,
        match step {
            Step::Over => STEP_OVER,
            Step::Into => STEP_INTO,
            Step::Out => STEP_OUT,
        },
        &effects,
    );
    effects
}

/// Ctrl+F2: a launched program is terminated and an attached one left
/// running. A session already stopping is let go at once.
pub fn stop(next: &mut State) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    let effects = match session.phase {
        Phase::Stopping | Phase::Spawning => {
            let mut effects = end(next);
            effects.push(Effect::StopDap);
            effects
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
    };
    lit(next, STOP, &effects);
    effects
}

/// Ctrl+F5, the `r` chord and the restart Chip: the last Launch configuration
/// started again. Refused by name with none — a key that quietly did nothing
/// would read as a key that failed.
pub fn restart(next: &mut State) -> Vec<Effect> {
    let Some(name) = next.last_launch.clone() else {
        next.refusal = Some(Refusal::NoLastSession);
        return Vec::new();
    };
    let effects = start(next, &name);
    lit(next, RESTART, &effects);
    effects
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
    let mut rows: Vec<Row> = state
        .watches
        .iter()
        .enumerate()
        .map(|(index, watch)| Row {
            name: watch.expression.clone(),
            value: match &watch.answer {
                Answer::Waiting => String::new(),
                Answer::Value(value) | Answer::Failed(value) => value.clone(),
            },
            depth: 0,
            hint: Hint::Plain,
            open: false,
            opens: Opens::Nothing,
            expression: watch.expression.clone(),
            parent: 0,
            of: Of::Watch {
                index,
                calling: calls(&paused_in(state), &watch.expression),
                failed: matches!(watch.answer, Answer::Failed(_)),
            },
        })
        .collect();
    let Some(pause) = showing(state) else {
        return rows;
    };
    if let Some(text) = &pause.exception {
        rows.push(Row {
            name: "exception".to_string(),
            value: text.clone(),
            depth: 0,
            hint: Hint::Plain,
            open: false,
            opens: Opens::Nothing,
            expression: String::new(),
            parent: 0,
            of: Of::Member,
        });
    }
    for scope in &pause.scopes {
        draw(pause, scope, 0, "", &mut Vec::new(), &mut rows);
    }
    rows
}

/// Whether an expression calls something, read off the same syntax `ui`
/// colours the editor with — never by handing it to the adapter to try, which
/// is the call the mark exists to warn about. `named` is the file whose
/// language the expression is written in, and `f(x)` is a call in some
/// languages and an index in others, so the wrong one answers no and the
/// caller runs the call it was asking about.
///
/// The two callers name two different files on purpose: a Watch is written in
/// the Paused Frame's language, while a Hover's expression was read out of
/// the Buffer under the pointer — which need not be the file the program
/// stopped in.
fn paused_in(state: &State) -> String {
    file_named(paused_line(state).map(|(file, _, _)| file))
}

/// A path's last component, which is all [`crate::highlight`] reads a
/// language off. Empty for no path at all, which is the one plain token per
/// line an unknown extension already gets.
fn file_named(path: Option<&Path>) -> String {
    path.and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_string()
}

fn calls(named: &str, expression: &str) -> bool {
    crate::highlight::highlight(named, expression)
        .into_iter()
        .flatten()
        .any(|token| token.kind == crate::highlight::Kind::Function)
}

/// One member and, while it is open, everything under it — then the row that
/// stands for the rest of a collection whose page has not been asked for.
///
/// `walked` is what stands above this member, and a reference already on it is
/// not opened again: the references are the adapter's, which is untrusted
/// input, and one that holds itself would otherwise be a structure the reader
/// could open into a stack overflow.
fn draw(
    pause: &Pause,
    member: &Member,
    depth: usize,
    path: &str,
    walked: &mut Vec<i64>,
    rows: &mut Vec<Row>,
) {
    let open = member.reference != 0
        && pause.open.contains(&member.reference)
        && !walked.contains(&member.reference);
    // A scope is a heading rather than a name the program knows, so it has no
    // expression of its own and its members start from theirs.
    let expression = match depth {
        0 => String::new(),
        _ => joined(path, &member.name),
    };
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
        expression: expression.clone(),
        parent: walked.last().copied().unwrap_or_default(),
        of: Of::Member,
    });
    if !open {
        return;
    }
    walked.push(member.reference);
    for child in pause.children.get(&member.reference).into_iter().flatten() {
        draw(pause, child, depth + 1, &expression, walked, rows);
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
            expression: String::new(),
            parent: member.reference,
            of: Of::Member,
        });
    }
}

/// The row the keyboard is on in the Variables, if it names one.
pub fn row(state: &State) -> Option<Row> {
    variables(state).into_iter().nth(state.variables_selection)
}

/// The Chips the row at `index` carries: its own actions, and only on the row
/// the keyboard is on — a pane drawing every row's actions is a pane of
/// icons with one row's worth of meaning.
///
/// Dimmed, never hidden, so the reader learns the action exists and why it
/// cannot run here: setting a value needs an adapter that said it can, and
/// the last two are issues #60 and #70 — a Chip teaching a key nobody bound
/// is the cheatsheet contract broken from the other end.
pub fn row_chips(state: &State, index: usize) -> Vec<crate::Chip> {
    use crate::{Chip, Hue, Tone};
    // The cheap question first: `ui` asks this of every row it draws, and
    // flattening the tree per row is the pane's whole cost squared.
    if index != state.variables_selection {
        return Vec::new();
    }
    let Some(row) = variables(state).into_iter().nth(index) else {
        return Vec::new();
    };
    let chip = |action, name, glyph: &str, keys, hue, dimmed| Chip {
        action,
        name,
        glyph: glyph.to_string(),
        keys,
        hue,
        tone: match dimmed {
            true => Tone::Dimmed,
            false => Tone::Plain,
        },
    };
    // A scope is a heading, and the row that stands for a collection's next
    // page is a place in the list — neither is a member the program could be
    // asked about, so what acts on a member is dimmed on both. `parent` is
    // the container `setVariable` writes into and `expression` is what a
    // Watch would carry: a row with neither is a row those two cannot act on.
    let nothing_to_write = row.parent == 0;
    let nothing_to_watch = row.expression.is_empty();
    let cannot_set =
        nothing_to_write || !state.debug.as_ref().is_some_and(|session| session.can_set);
    vec![
        chip(
            SET_VALUE,
            "set-value",
            "\u{270e}",
            "s",
            Hue::Step,
            cannot_set,
        ),
        chip(COPY_VALUE, "copy", "\u{29c9}", "y", Hue::Plain, false),
        match row.of {
            Of::Watch { .. } => chip(
                REMOVE_WATCH,
                "remove-watch",
                "\u{2715}",
                "d",
                Hue::Halt,
                false,
            ),
            Of::Member => chip(WATCH, "watch", "\u{25c9}", "w", Hue::Go, nothing_to_watch),
        },
        // A row with no expression is a heading or a place in a list, which
        // is nothing the Evaluator could be opened on.
        chip(
            EVALUATE,
            "evaluate",
            "\u{2261}",
            "",
            Hue::Plain,
            nothing_to_watch,
        ),
        chip(ROW_ASK_AI, "ask-ai", "\u{2736}", "", Hue::Plain, true),
    ]
}

/// The watch Chip on a member's row, and a Watch typed into the box the `a`
/// key opens: the expression joins the Watches and is evaluated at the next
/// pause — and at this one, if the program is stopped, since a Watch added
/// while Paused with nothing to show is a Watch that looks broken.
pub fn add_watch(next: &mut State, expression: String) -> Vec<Effect> {
    if expression.is_empty() || next.watches.iter().any(|w| w.expression == expression) {
        return Vec::new();
    }
    next.watches.push(Watch {
        expression: expression.clone(),
        answer: Answer::Waiting,
    });
    // Only the one just added: the Watches above it have been answered for
    // this pause already, and asking for all of them again would blank every
    // value on screen because somebody added a sixth.
    evaluate(next, &[expression], WATCH_CONTEXT)
}

/// The remove-watch Chip on a Watch's row.
pub fn remove_watch(next: &mut State, index: usize) {
    if index < next.watches.len() {
        next.watches.remove(index);
    }
}

/// One `evaluate` per Watch, in the protocol's `watch` context and against
/// the Frame being inspected — asked at every pause and again whenever
/// another Frame is chosen, since the same expression means something else
/// one call up. Nothing at all while the program runs: an adapter asked to
/// evaluate in a Frame that is no longer stopped answers with an error.
fn evaluate_watches(next: &mut State) -> Vec<Effect> {
    let watches: Vec<String> = next
        .watches
        .iter()
        .map(|watch| watch.expression.clone())
        .collect();
    // Every answer goes back to waiting first: they are about the pause that
    // has just ended, and a value left standing under a new pause is a value
    // the reader has no way to tell is stale.
    for watch in next.watches.iter_mut() {
        watch.answer = Answer::Waiting;
    }
    evaluate(next, &watches, WATCH_CONTEXT)
}

/// The `evaluate` requests for `watches`, or none at all while the program is
/// not stopped in a Frame to evaluate them in: an adapter asked to evaluate
/// in a Frame that is running answers with an error.
fn evaluate(next: &mut State, watches: &[String], context: &str) -> Vec<Effect> {
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    let Phase::Paused(pause) = &session.phase else {
        return Vec::new();
    };
    let Some(frame) = pause.frames.get(pause.chosen) else {
        return Vec::new();
    };
    let id = frame.id;
    watches
        .iter()
        .map(|expression| {
            ask(
                session,
                "evaluate",
                json!({ "expression": expression, "frameId": id, "context": context }),
            )
        })
        .collect()
}

/// The text typed into the row's box, sent to the adapter exactly as it was
/// written: it is an expression in the program's language, which Varde does
/// not parse and must not rewrite. Refused by the capability rather than by
/// trying it, which is what the dimmed Chip already says.
pub fn set_value(next: &mut State, value: String) -> Vec<Effect> {
    let Some(row) = row(next) else {
        return Vec::new();
    };
    let name = row.name.clone();
    let parent = row.parent;
    let Some(session) = next.debug.as_mut().filter(|session| session.can_set) else {
        return Vec::new();
    };
    vec![ask(
        session,
        "setVariable",
        json!({ "variablesReference": parent, "name": name, "value": value }),
    )]
}

/// Enter on a Variables row, and a click on one: a member with children is
/// opened or closed, and the row that stands for a collection's next page asks
/// for it. Opening asks for one level — the children of that reference and
/// nothing under them — so walking a deep structure asks for what is opened
/// and nothing else.
pub fn open(next: &mut State, index: usize) -> Vec<Effect> {
    match variables(next).get(index).cloned() {
        Some(row) => opened(next, row),
        None => Vec::new(),
    }
}

/// What opening a row asks for, whichever list the row came from: the
/// Variables' and the Hover's are rows of one tree, so a reference opened in
/// one is opened in the other.
fn opened(next: &mut State, row: Row) -> Vec<Effect> {
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

/// A member's expression under whatever holds it: an indexed member carries
/// on from its container's own spelling, so `orders` and `[0]` read as
/// `orders[0]`, and anything else is reached through a dot. A scope has no
/// expression at all, so its members start from their own names.
fn joined(path: &str, name: &str) -> String {
    match (path.is_empty(), name.starts_with('[')) {
        (true, _) => name.to_string(),
        (false, true) => format!("{path}{name}"),
        (false, false) => format!("{path}.{name}"),
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

/// What a Hover asked the Debug adapter while Paused: the expression the
/// syntax under the pointer named, where it starts so the editor can mark it,
/// and what came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hovered {
    pub expression: String,
    pub at: Place,
    pub held: Held,
}

/// What the Hover's expression holds, as far as the adapter has said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The expression calls something, so the adapter was never asked: a
    /// pointer crossing `delete_order()` on its way somewhere else must not
    /// delete an order. Only the Evaluator runs a call, because somebody
    /// pressed a key for it.
    NeedsEvaluate,
    /// Asked and not yet answered.
    Waiting,
    Failed(String),
    Value {
        value: String,
        reference: i64,
        indexed: usize,
    },
}

/// The Hover's value section while Paused, and the `evaluate` that asks for
/// it. Nothing at all outside a session and while the program runs, where a
/// Hover is the language server's answer and nothing more.
pub fn hovered(next: &mut State, at: Place) -> (Option<Hovered>, Vec<Effect>) {
    if !matches!(
        next.debug.as_ref().map(|session| &session.phase),
        Some(Phase::Paused(_))
    ) {
        return (None, Vec::new());
    }
    let Some((expression, column)) = expression_at(next, at) else {
        return (None, Vec::new());
    };
    // The Buffer the expression was read out of, not the file the program
    // stopped in: they need not be the same file, and a call parsed under
    // another language's grammar is a call this is about to run.
    let named = file_named(next.current_buffer.as_deref());
    let at = Place {
        line: at.line,
        column,
    };
    if calls(&named, &expression) {
        return (
            Some(Hovered {
                expression,
                at,
                held: Held::NeedsEvaluate,
            }),
            Vec::new(),
        );
    }
    let effects = evaluate(next, std::slice::from_ref(&expression), HOVER);
    (
        Some(Hovered {
            expression,
            at,
            held: Held::Waiting,
        }),
        effects,
    )
}

/// The protocol's contexts an `evaluate` goes out in. Both what tells the
/// adapter how its answer will be used — `hover` is the one that asks it to
/// answer cheaply and without side effects — and, since a reply names nothing
/// of its question, how [`hover_asked`] tells a Hover's answer from a Watch's.
/// Named rather than written at each call site: a literal mistyped at one of
/// the three still compiles and silently files the answer against the wrong
/// thing.
const HOVER: &str = "hover";
const WATCH_CONTEXT: &str = "watch";

/// The expression the syntax under a place names, and the column it starts
/// at: `order.total` where the pointer is on `total`, and the whole call where
/// it is on the name of one. Character by character over the line the Buffer
/// holds rather than over [`crate::highlight`]'s tokens, because a token is
/// grouped by scope and not by name — `mentioned` splits them again for the
/// same reason.
///
/// A place that is not in a name is no expression at all: the `7` of `load(7)`
/// is a literal, and the only expression around it is the call it is an
/// argument to — which is the one thing a Hover may not ask about.
fn expression_at(state: &State, at: Place) -> Option<(String, usize)> {
    let line = line_chars(state, at.line)?;
    let on = at.column.checked_sub(1)?;
    if !line.get(on).is_some_and(|character| named(*character)) {
        return None;
    }
    let mut from = on;
    while from > 0 && named(line[from - 1]) {
        from -= 1;
    }
    let mut to = on + 1;
    while to < line.len() && named(line[to]) {
        to += 1;
    }
    // A name no language lets start with a digit is a literal, and a literal
    // has nothing the program could be asked about.
    if line[from].is_ascii_digit() {
        return None;
    }
    // Back through the receivers a field is read off: `total` on its own is
    // not a name the program knows, and asking for it would either fail or —
    // worse — answer about some other `total` in scope. A receiver that is
    // itself a call or an index comes too, closer first, which is what makes
    // `get().total` an expression that calls something rather than a bare
    // `total` the adapter would happily evaluate.
    while from > 1 && line[from - 1] == '.' {
        let Some(receiver) = ends_at(&line, from - 2) else {
            break;
        };
        from = receiver;
    }
    // And on over the call the name opens, if it opens one: a call's name
    // alone evaluates to the function, which is not what is being pointed at.
    // Balanced, so a call taking a call is one expression.
    if line.get(to) == Some(&'(') {
        let mut depth = 0;
        for (index, character) in line.iter().enumerate().skip(to) {
            match character {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        to = index + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    Some((line[from..to].iter().collect(), from + 1))
}

/// One line of the buffer on screen, 1-based, as characters — what both
/// readings of an expression walk.
fn line_chars(state: &State, line: usize) -> Option<Vec<char>> {
    let path = state.current_buffer.as_ref()?;
    Some(
        state
            .buffers
            .get(path)?
            .shown()
            .split('\n')
            .nth(line.checked_sub(1)?)?
            .chars()
            .collect(),
    )
}

/// What `\u{2423}e` opens the Evaluator on: the Selection if there is one, and
/// otherwise the whole expression the cursor stands in — `orders.len()` with
/// the cursor on `orders`, where a Hover would name `orders` alone.
///
/// The difference is deliberate and is the same one that lets this run a call
/// at all: a Hover describes whatever the pointer happens to cross, so it
/// stops at the part it is resting on and refuses to call anything; this ran
/// because somebody pressed a key for it, so the call at the end of the chain
/// is the point rather than the danger.
pub fn cursor_expression(state: &State) -> String {
    if let Some(text) = state.selected_text().filter(|text| !text.is_empty()) {
        return text;
    }
    let Some(buffer) = crate::current_buffer(state) else {
        return String::new();
    };
    let at = Place {
        line: buffer.line,
        column: buffer.column,
    };
    let Some((expression, column)) = expression_at(state, at) else {
        return String::new();
    };
    let Some(line) = line_chars(state, at.line) else {
        return expression;
    };
    let from = column - 1;
    let to = chain_end(&line, from + expression.chars().count());
    line[from..to].iter().collect()
}

/// On through the chain the expression continues into: `.len()` after
/// `orders`, and the groups each name in it carries, balanced so a call
/// taking a call is one expression.
fn chain_end(line: &[char], mut to: usize) -> usize {
    loop {
        // The groups the name just passed carries — a call's arguments, an
        // index — balanced, so a call taking a call is one expression.
        while let Some(open @ ('(' | '[')) = line.get(to).copied() {
            let close = match open {
                '(' => ')',
                _ => ']',
            };
            let mut depth = 0usize;
            let mut at = to;
            loop {
                match line.get(at) {
                    Some(character) if *character == open => depth += 1,
                    Some(character) if *character == close => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Some(_) => {}
                    // Unbalanced, so there is no group to take: the expression
                    // ends where the name did.
                    None => return to,
                }
                at += 1;
            }
            to = at + 1;
        }
        if line.get(to) != Some(&'.') {
            return to;
        }
        let mut after = to + 1;
        while after < line.len() && named(line[after]) {
            after += 1;
        }
        // A dot with no name after it ends nothing — a decimal point, or a
        // chain the reader has not finished typing.
        if after == to + 1 {
            return to;
        }
        to = after;
    }
}

fn named(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Where the expression ending at `last` begins: a name, or a name followed by
/// as many bracketed groups as it carries — `a.b()[0]` read right to left.
/// `None` where what ends there is not an expression at all, which leaves the
/// chain broken and the walk above stopping where it stands.
fn ends_at(line: &[char], last: usize) -> Option<usize> {
    let mut at = last;
    loop {
        let opener = match line.get(at)? {
            ')' => '(',
            ']' => '[',
            character if named(*character) => break,
            _ => return None,
        };
        let closer = line[at];
        let mut depth = 0usize;
        loop {
            let character = *line.get(at)?;
            if character == closer {
                depth += 1;
            } else if character == opener {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            at = at.checked_sub(1)?;
        }
        // Past the group, onto whatever it hangs off: another group, or the
        // name the whole chain starts at.
        at = at.checked_sub(1)?;
    }
    while at > 0 && named(line[at - 1]) {
        at -= 1;
    }
    Some(at)
}

/// The expression the Hover describes, and how many characters of the line it
/// spans: the editor washes it while the box is up, so what was evaluated is
/// never left for the reader to guess from a box floating over the code.
pub fn hover_span(state: &State) -> Option<(Place, usize)> {
    let hovered = state.hover.as_ref()?.value.as_ref()?;
    Some((hovered.at, hovered.expression.chars().count()))
}

/// The Hover's value as rows, the tree flattened exactly as the Variables' is
/// — the same [`draw`], so a structure opens the same way in both places and
/// opening it in one is opening it in the other. Empty until the adapter has
/// answered: what a Hover says while it waits, and what it says about a call
/// it will not run, are [`crate::lsp::Said`]'s to draw.
pub fn hovered_rows(state: &State) -> Vec<Row> {
    let Some(hovered) = state.hover.as_ref().and_then(|hover| hover.value.as_ref()) else {
        return Vec::new();
    };
    let Some(pause) = showing(state) else {
        return Vec::new();
    };
    let member = match &hovered.held {
        Held::NeedsEvaluate => return Vec::new(),
        // The expression with nothing beside it yet, exactly as a Watch waits:
        // a box that showed nothing at all until the adapter answered would
        // open two columns wide and jump to its real size a moment later.
        Held::Waiting => Member {
            name: hovered.expression.clone(),
            value: String::new(),
            reference: 0,
            hint: Hint::Plain,
            indexed: 0,
        },
        Held::Failed(why) => Member {
            name: hovered.expression.clone(),
            value: why.clone(),
            reference: 0,
            hint: Hint::Plain,
            indexed: 0,
        },
        Held::Value {
            value,
            reference,
            indexed,
        } => Member {
            name: hovered.expression.clone(),
            value: value.clone(),
            reference: *reference,
            hint: Hint::Plain,
            indexed: *indexed,
        },
    };
    let mut rows = Vec::new();
    draw(pause, &member, 0, "", &mut Vec::new(), &mut rows);
    rows
}

/// The Chips on the Hover's top border: the Evaluator, which is the only way
/// to know what a call returns, and a Watch, which keeps the expression on the
/// Variables once the pointer has moved on. Their own Chips and not the
/// Variables row's, because the two act on different expressions.
pub fn hover_chips(state: &State) -> Vec<crate::Chip> {
    use crate::{Chip, Hue, Tone};
    if state
        .hover
        .as_ref()
        .and_then(|hover| hover.value.as_ref())
        .is_none()
    {
        return Vec::new();
    }
    let chip = |action, name, glyph: &str, hue, tone| Chip {
        action,
        name,
        glyph: glyph.to_string(),
        keys: "",
        hue,
        tone,
    };
    vec![
        chip(EVALUATE, "evaluate", "\u{2261}", Hue::Plain, Tone::Plain),
        chip(WATCH, "watch", "\u{25c9}", Hue::Go, Tone::Plain),
    ]
}

/// The Hover's Chips as they are drawn and hit-tested — the renderer, the
/// mouse and the box's own width read this one list, for the reason every
/// other strip of Chips has one. Nothing reserved for a title, unlike a pane's
/// border: the box has no name written on it.
pub fn hover_labels(state: &State, width: u16) -> Vec<String> {
    crate::layout::chip_labels(&hover_chips(state), width, 0)
}

/// The Watch Chip on the Hover: the expression the box describes joins the
/// Watches, so a value worth a second look outlives the pointer that found it.
pub fn watch_hovered(next: &mut State) -> Vec<Effect> {
    let Some(expression) = next
        .hover
        .as_ref()
        .and_then(|hover| hover.value.as_ref())
        .map(|hovered| hovered.expression.clone())
    else {
        return Vec::new();
    };
    add_watch(next, expression)
}

/// A click on a row of the Hover's value, which opens it exactly as the same
/// row of the Variables opens.
pub fn open_hovered(next: &mut State, index: usize) -> Vec<Effect> {
    match hovered_rows(next).get(index).cloned() {
        Some(row) => opened(next, row),
        None => Vec::new(),
    }
}

/// The floating window that runs a Snippet inside the Paused program, in the
/// chosen Frame. The Snippet is a [`crate::editor::Buffer`] so it inherits
/// the editor's own gestures rather than a second text editor written on a
/// string — the reason the comment box is one too.
///
/// Core state rather than the session's, though it closes with one: what a
/// reader is in the middle of writing is theirs, and the Snippet they ran is
/// remembered past the program it ran in.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluator {
    pub snippet: crate::editor::Buffer,
    /// The run on screen, and `None` until the Snippet has been run once.
    /// Replaced whole at every run: the output is about the run just made,
    /// and a line left over from the one before it reads as this one's.
    pub ran: Option<Run>,
    /// How many rows of the window the Snippet takes, and `None` until the
    /// rule under it is dragged — half, until somebody names a number, the
    /// way the Strip's height is a share until somebody drags it. With the
    /// window rather than with the project: the reader is dividing the room
    /// they have between what they are writing and what came back.
    pub snippet_rows: Option<u16>,
    /// How far back through the project's Snippets Up has walked, and `None`
    /// while the keyboard is in the Snippet rather than in its history.
    recalled: Option<usize>,
}

/// The modifier-free keyboard mode that arranges the Evaluator's window: `␣m`
/// moves it and `␣z` resizes it, and the same four letters do both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrange {
    Moving,
    Sizing,
}

/// One run of the Snippet: what the program printed while it ran and what it
/// came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// What went to the adapter — the Selection where there was one, and not
    /// the whole Snippet. Taken at the run rather than read back off the
    /// Selection, which the reader has moved on by the time an answer lands:
    /// the value is labelled with the code that produced it or with nothing.
    ran: String,
    /// In the order it arrived, and before the value below: a side effect
    /// happens while the expression that has it is still running.
    printed: Vec<String>,
    answer: Ran,
}

/// Where a run has got to. An enum for the reason [`Phase`] is one: running
/// and failed at once has no answer for the Run Chip.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ran {
    /// The `seq` the `evaluate` went out with, which is what a cancel names —
    /// the protocol takes a request back by its number and by nothing else,
    /// and it is also how a reply is told from a reply to the run before it.
    Running(i64),
    Value {
        value: String,
        reference: i64,
        indexed: usize,
    },
    Failed(String),
}

/// One row of the Evaluator output as it is drawn, hit-tested and asserted
/// on: the prints, then the value's tree or the adapter's reason. One list
/// for the reason the Variables are one list. Named as [`crate::lsp::Said`]
/// is, and for its reason — a row of a box is what the box says — which also
/// keeps it apart from the renderer's own `Line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    Printed(String),
    /// The value, and everything opened under it — the adapter's tree, drawn
    /// and opened exactly as the Variables are.
    Value(Row),
    /// The adapter's reason, kept apart from a value for the reason a Watch's
    /// is: a reason drawn as a value reads as the program's own answer.
    Failed(String),
    /// Gone out and nothing back yet.
    Running,
}

/// The protocol's context a Snippet goes out in. `repl` is the one that tells
/// the adapter the answer is for a person to read and that side effects are
/// wanted — a Snippet is run because somebody pressed a key for it, which is
/// the whole difference from the `hover` context beside it.
const REPL: &str = "repl";

pub const RUN: &str = "evaluator-run";
pub const CANCEL: &str = "evaluator-cancel";

/// `␣e`, the Hover's Evaluate Chip and a Variables row's: the Evaluator opens
/// on the expression the gesture named, prefilled and asking the adapter
/// nothing. What runs is what the reader presses Enter on, which is the whole
/// reason a Hover may refuse to evaluate a call and this may not.
pub fn open_evaluator(next: &mut State, expression: String) {
    if next.debug.is_none() {
        return;
    }
    next.evaluator = Some(Evaluator {
        snippet: crate::editor::Buffer::open(&expression, false, next.tab_width),
        ran: None,
        snippet_rows: None,
        recalled: None,
    });
    // Where the project last left it, and the middle of the screen the first
    // time — `settle` places it from there, so a rectangle recorded on a
    // bigger screen needs no arm of its own here.
    next.evaluator_at =
        Some(next.evaluator_at.unwrap_or_else(|| {
            crate::layout::centred_window(next.screen_width, next.screen_height)
        }));
    next.focus = Pane::Evaluator;
    // The Selection the expression was *read from* goes with it. It is a span
    // of the buffer behind the window, and the run below reads a Selection
    // against the Snippet: left standing, `\u{2423}e` on line 3 columns 17-28 would
    // be read against a one-line Snippet and send the adapter an empty
    // expression, silently. A Selection in the Snippet is one made in the
    // Snippet.
    next.selection = None;
}

/// Normal-mode Enter, the Run Chip and Ctrl+Enter: the Selection if there is
/// one and the whole Snippet otherwise, in the `repl` context and against the
/// Frame being inspected. Nothing at all while the program is not stopped in
/// a Frame to run it in — an adapter asked to evaluate then answers with an
/// error, which is why a Watch is not asked then either.
pub fn run(next: &mut State) -> Vec<Effect> {
    if next.evaluator.is_none() {
        return Vec::new();
    }
    let text = running_text(next);
    let whole = snippet_text(next);
    let Some(session) = next.debug.as_mut() else {
        return Vec::new();
    };
    let Phase::Paused(pause) = &session.phase else {
        return Vec::new();
    };
    let Some(frame) = pause.frames.get(pause.chosen) else {
        return Vec::new();
    };
    let id = frame.id;
    let effect = ask(
        session,
        "evaluate",
        json!({ "expression": &text, "frameId": id, "context": REPL }),
    );
    let seq = session.seq;
    if let Some(evaluator) = next.evaluator.as_mut() {
        evaluator.ran = Some(Run {
            ran: text,
            printed: Vec::new(),
            answer: Ran::Running(seq),
        });
    }
    let mut effects = vec![effect];
    effects.extend(remember(next, whole));
    effects
}

/// The cancel Chip: the run still in flight taken back by its number. Refused
/// by the capability rather than by trying it, which is what the dimmed Chip
/// already says — the reason a set value is refused that way.
pub fn cancel(next: &mut State) -> Vec<Effect> {
    let Some(Ran::Running(seq)) = next
        .evaluator
        .as_ref()
        .and_then(|evaluator| evaluator.ran.as_ref())
        .map(|ran| ran.answer.clone())
    else {
        return Vec::new();
    };
    let Some(session) = next.debug.as_mut().filter(|session| session.can_cancel) else {
        return Vec::new();
    };
    vec![ask(session, "cancel", json!({ "requestId": seq }))]
}

/// Where the window goes: the rectangle a gesture named, placed on the screen
/// and clear of the Paused line, and recorded for the project. Placed here as
/// well as in [`crate::settle`] — the one function, called twice — because the
/// number written to disk has to be the number on screen, and the effect
/// carrying it is built before the clamp every event goes through.
pub fn place(next: &mut State, at: crate::layout::Area) -> Vec<Effect> {
    if next.evaluator.is_none() {
        return Vec::new();
    }
    next.evaluator_at = Some(crate::layout::placed_window(
        at,
        next.screen_width,
        next.screen_height,
        paused_row(next),
    ));
    vec![Effect::SaveState(crate::state_json(next))]
}

/// The keyboard's arrange: one cell of the window per key, in the mode `␣m`
/// or `␣z` opened. The letters are the editor's own motions, so moving the
/// window and moving the caret are the same four keys — the modifier-free
/// gesture `AGENTS.md` requires, and the arrows reach it too.
pub fn arrange(next: &mut State, direction: crate::Direction, how: Arrange) -> Vec<Effect> {
    use crate::Direction::{Down, Left, Right, Up};
    let Some(at) = next.evaluator_at else {
        return Vec::new();
    };
    let moved = match (how, direction) {
        (Arrange::Moving, Left) => crate::layout::Area {
            x: at.x.saturating_sub(1),
            ..at
        },
        (Arrange::Moving, Right) => crate::layout::Area { x: at.x + 1, ..at },
        (Arrange::Moving, Up) => crate::layout::Area {
            y: at.y.saturating_sub(1),
            ..at
        },
        (Arrange::Moving, Down) => crate::layout::Area { y: at.y + 1, ..at },
        (Arrange::Sizing, Left) => crate::layout::Area {
            width: at.width.saturating_sub(1),
            ..at
        },
        (Arrange::Sizing, Right) => crate::layout::Area {
            width: at.width + 1,
            ..at
        },
        (Arrange::Sizing, Up) => crate::layout::Area {
            height: at.height.saturating_sub(1),
            ..at
        },
        (Arrange::Sizing, Down) => crate::layout::Area {
            height: at.height + 1,
            ..at
        },
    };
    place(next, moved)
}

/// Which screen row the Paused line is drawn on, and nothing where the file
/// it is in is not the one on screen or the wheel has taken it off the pane.
/// Read by the clamp that keeps the Evaluator off it, off the same
/// `editor_scroll` the line itself is drawn from — two derivations of where a
/// line is would be a window covering the very line it was moved to clear.
pub fn paused_row(state: &State) -> Option<u16> {
    let (file, line, _) = paused_line(state)?;
    if state.current_buffer.as_deref() != Some(file) {
        return None;
    }
    let editor = crate::panes_of(state).editor;
    let row = u16::try_from(line.checked_sub(1)?.checked_sub(state.editor_scroll)?).ok()?;
    let at = editor.y + 1 + row;
    (at < editor.bottom().saturating_sub(1)).then_some(at)
}

/// Up and Down on an empty Snippet walk the project's Snippets, newest first,
/// and Down comes forward again — the gesture every shell has, for its
/// reason: the code that worked last time is the code being reached for.
/// `true` where the arrow was the history's, which is what leaves it the
/// caret's motion on a Snippet somebody is writing.
///
/// Walking outlives the emptiness that started it: the Snippet a recall put
/// there is not empty, and a second Up that moved the caret instead would be
/// a history one step deep.
pub fn recall(next: &mut State, direction: crate::Direction) -> bool {
    use crate::Direction::{Down, Up};
    let newest = match next.snippets.len() {
        0 => return false,
        held => held - 1,
    };
    let tab_width = next.tab_width;
    let Some(evaluator) = next.evaluator.as_mut() else {
        return false;
    };
    if evaluator.recalled.is_none() && !evaluator.snippet.shown().is_empty() {
        return false;
    }
    let at = match (evaluator.recalled, direction) {
        (None, Up) => newest,
        (Some(at), Up) => at.saturating_sub(1),
        (Some(at), Down) => (at + 1).min(newest),
        // Down with nothing recalled is the caret's, on a Snippet with
        // nothing above it to come forward from.
        (None, Down) => return false,
        (_, crate::Direction::Left | crate::Direction::Right) => return false,
    };
    evaluator.recalled = Some(at);
    let recalled = next.snippets[at].clone();
    if let Some(evaluator) = next.evaluator.as_mut() {
        evaluator.snippet = crate::editor::Buffer::open(&recalled, false, tab_width);
    }
    true
}

/// What this run sends: the Selection where the keyboard is in the Snippet
/// and there is one, so a line of a longer block can be tried on its own.
fn running_text(state: &State) -> String {
    let Some(evaluator) = state.evaluator.as_ref() else {
        return String::new();
    };
    match state
        .selection
        .as_ref()
        .and_then(crate::Selection::buffer_span)
    {
        Some((from, to)) if state.focus == Pane::Evaluator => evaluator.snippet.text_in(from, to),
        _ => evaluator.snippet.shown().to_string(),
    }
}

/// The whole Snippet, which is what is remembered whatever was run: the
/// reader wrote the block, and recalling the one line they tried of it would
/// hand back something they never typed.
fn snippet_text(state: &State) -> String {
    state
        .evaluator
        .as_ref()
        .map(|evaluator| evaluator.snippet.shown().to_string())
        .unwrap_or_default()
}

/// A Snippet kept for the project, newest last and never twice. Written where
/// the Snippet leaves the window — at a run and at the close that ends the
/// session — rather than on every keystroke, which would remember every
/// half-typed line on the way to the one that worked.
fn remember(next: &mut State, snippet: String) -> Vec<Effect> {
    if snippet.is_empty() {
        return Vec::new();
    }
    next.snippets.retain(|held| held != &snippet);
    next.snippets.push(snippet);
    vec![Effect::SaveState(crate::state_json(next))]
}

/// The Chips on the Evaluator's top border: run, and the cancel that takes a
/// run back. Run is dimmed while the program is not stopped, because there is
/// no Frame to run a Snippet in; cancel while there is nothing in flight or
/// the adapter cannot take one back.
pub fn evaluator_chips(state: &State) -> Vec<crate::Chip> {
    use crate::{Chip, Hue, Tone};
    let Some(evaluator) = state.evaluator.as_ref() else {
        return Vec::new();
    };
    let running = matches!(
        evaluator.ran.as_ref().map(|ran| &ran.answer),
        Some(Ran::Running(_))
    );
    let stopped = matches!(
        state.debug.as_ref().map(|session| &session.phase),
        Some(Phase::Paused(_))
    );
    let can_cancel = state
        .debug
        .as_ref()
        .is_some_and(|session| session.can_cancel);
    vec![
        Chip {
            action: RUN,
            name: "run",
            glyph: "\u{25b6}".to_string(),
            keys: "\u{21b5}",
            hue: Hue::Go,
            tone: match stopped {
                true => Tone::Plain,
                false => Tone::Dimmed,
            },
        },
        Chip {
            action: CANCEL,
            name: "cancel",
            glyph: "\u{2715}".to_string(),
            keys: "",
            hue: Hue::Halt,
            tone: match running && can_cancel {
                true => Tone::Plain,
                false => Tone::Dimmed,
            },
        },
    ]
}

/// The Evaluator's Chips as they are drawn and hit-tested, for the reason the
/// Hover's are one list.
pub fn evaluator_labels(state: &State, width: u16) -> Vec<String> {
    crate::layout::chip_labels(&evaluator_chips(state), width, 0)
}

/// The Evaluator output: the prints in the order they arrived, then the value
/// as a tree opened exactly as the Variables are, or the adapter's reason.
pub fn evaluator_output(state: &State) -> Vec<Said> {
    let Some(ran) = state
        .evaluator
        .as_ref()
        .and_then(|evaluator| evaluator.ran.as_ref())
    else {
        return Vec::new();
    };
    let mut lines: Vec<Said> = ran.printed.iter().cloned().map(Said::Printed).collect();
    match &ran.answer {
        Ran::Running(_) => lines.push(Said::Running),
        Ran::Failed(why) => lines.push(Said::Failed(why.clone())),
        Ran::Value {
            value,
            reference,
            indexed,
        } => {
            let Some(pause) = showing(state) else {
                return lines;
            };
            let member = Member {
                name: ran.ran.clone(),
                value: value.clone(),
                reference: *reference,
                hint: Hint::Plain,
                indexed: *indexed,
            };
            let mut rows = Vec::new();
            draw(pause, &member, 0, "", &mut Vec::new(), &mut rows);
            lines.extend(rows.into_iter().map(Said::Value));
        }
    }
    lines
}

/// A click on a row of the Evaluator's value, which opens it exactly as the
/// same row of the Variables opens.
pub fn open_evaluated(next: &mut State, index: usize) -> Vec<Effect> {
    match evaluator_output(next).into_iter().nth(index) {
        // A print has nothing under it to ask for, and neither has a reason.
        Some(Said::Value(row)) => opened(next, row),
        _ => Vec::new(),
    }
}

/// The run still in flight, which is what the program's prints belong to.
fn in_flight(next: &mut State) -> Option<&mut Run> {
    next.evaluator
        .as_mut()?
        .ran
        .as_mut()
        .filter(|ran| matches!(ran.answer, Ran::Running(_)))
}

/// The run an `evaluate` reply belongs to: the `repl` context is what names
/// it, and the `seq` has to still be the one in flight — a reply to the run
/// before this one belongs to output that has already been replaced.
fn ran_asked<'a>(next: &'a mut State, arguments: &Value, seq: i64) -> Option<&'a mut Run> {
    if arguments["context"] != json!(REPL) {
        return None;
    }
    in_flight(next).filter(|ran| ran.answer == Ran::Running(seq))
}

/// Where the program is paused, as the chosen Frame names it, and why.
pub fn paused_line(state: &State) -> Option<(&Path, usize, Why)> {
    let Some(Phase::Paused(pause)) = state.debug.as_ref().map(|session| &session.phase) else {
        return None;
    };
    let frame = pause.frames.get(pause.chosen)?;
    Some((frame.file.as_deref()?, frame.line, pause.why))
}

/// One value drawn at the end of a line the Paused call has already run.
/// `text` is exactly what the renderer draws, its gap included and its value
/// already trimmed to the columns the line leaves — the trimming is the
/// library's because *does it fit* is a question about the pane, and a
/// renderer that cut it itself would be a second author for the width. `name`
/// is what the line mentioned, which is the only thing the value belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inline {
    pub name: String,
    pub text: String,
    /// Whether this pause is the one where the value moved. One pause only:
    /// a mark that stayed would say the same thing at every stop, and what
    /// makes stepping readable is seeing what the line just did.
    pub changed: bool,
}

/// The gap before an Inline value, and between two of them.
const GAP: usize = 2;

/// The Inline values of the Buffer on screen, by line: every local the chosen
/// Frame holds, drawn at the end of the line that mentions it, from the line
/// the call opens on down to the line before the Paused one. Never on the
/// Paused line or past it — the call has not run those, so a value shown there
/// would be the one from before it was assigned.
///
/// `tokens` are the current Buffer's, parsed once per edit by the edge and
/// handed in for the reason [`crate::minimap::cells`] is handed them: nothing
/// parses per frame. `columns` is what
/// [`crate::fits_in`] counted off the rectangles the renderer drew, handed in
/// for the reason `mouse` is handed them: one derivation of where the text
/// ends, or a value is trimmed against a pane of another size.
pub fn inline(
    state: &State,
    tokens: &[Vec<crate::highlight::Token>],
    columns: usize,
) -> BTreeMap<usize, Vec<Inline>> {
    let mut drawn = BTreeMap::new();
    let (Some(session), Some(pause)) = (state.debug.as_ref(), showing(state)) else {
        return drawn;
    };
    let Some(frame) = pause.frames.get(pause.chosen) else {
        return drawn;
    };
    // The Buffer on screen and the call being inspected have to be the same
    // file: a value drawn against another file's line numbers names a line
    // nobody is paused in.
    let Some(buffer) = frame
        .file
        .as_ref()
        .filter(|file| state.current_buffer.as_ref() == Some(*file))
        .and_then(|file| state.buffers.get(file))
    else {
        return drawn;
    };
    let held = locals(pause);
    // Nothing is marked while the program runs: a highlight says *this pause*
    // moved it, and between pauses there is no such pause. What is on screen
    // then is the last one's values, whole and faint, which is what dims them.
    let was = session
        .previous
        .as_ref()
        .filter(|_| matches!(session.phase, Phase::Paused(_)))
        .filter(|(called, _)| *called == frame.name)
        .map(|(_, held)| held);
    let source = buffer.shown();
    let opens = call_start(source, frame.line);
    for (index, line) in source
        .split('\n')
        .enumerate()
        .skip(opens - 1)
        .take(frame.line.saturating_sub(opens))
    {
        let number = index + 1;
        let mut room = columns.saturating_sub(line.width());
        let mut values: Vec<Inline> = Vec::new();
        for name in mentioned(tokens.get(number - 1).map_or(&[], Vec::as_slice)) {
            let Some(value) = held.get(name) else {
                continue;
            };
            if values.iter().any(|shown| shown.name == name) {
                continue;
            }
            let head = format!("{}{name} = ", " ".repeat(GAP));
            // A name with no room left for even one column of its value is
            // not drawn at all, and neither is anything after it: code pushed
            // off screen is the one thing an Inline value must never do.
            let spare = room.saturating_sub(head.width());
            if spare == 0 {
                break;
            }
            let value = clipped(value, spare);
            room -= head.width() + value.width();
            values.push(Inline {
                name: name.to_string(),
                text: head + &value,
                changed: was.is_some_and(|was| was.get(name) != held.get(name)),
            });
        }
        if !values.is_empty() {
            drawn.insert(number, values);
        }
    }
    drawn
}

/// `text` cut to `columns` **display** columns, for the reason `ui`'s own
/// truncation is by width: a wide glyph cut on a character boundary still
/// overruns the column it was cut to fit, and the column it overruns is the
/// editor's last one.
fn clipped(text: &str, columns: usize) -> String {
    let mut left = columns;
    let mut kept = String::new();
    for character in text.chars() {
        let width = character.width().unwrap_or(0);
        if width > left {
            break;
        }
        left -= width;
        kept.push(character);
    }
    kept
}

/// What the chosen Frame's scopes hold, by name, in the adapter's own words.
/// The top level of the tree and nothing under it: a member called `id` inside
/// an order is not a name the code on screen mentions.
fn locals(pause: &Pause) -> BTreeMap<String, String> {
    pause
        .scopes
        .iter()
        .filter_map(|scope| pause.children.get(&scope.reference))
        .flatten()
        .map(|member| (member.name.clone(), member.value.clone()))
        .collect()
}

/// The names a line mentions, in the order it mentions them: the words of the
/// tokens the highlighter left plain. Which names are variables is the
/// grammar's answer and never a rule of Varde's — a name inside a comment, a
/// string, a call or a type is some other kind and never reaches here, in
/// every language the grammar set knows.
fn mentioned(tokens: &[crate::highlight::Token]) -> Vec<&str> {
    tokens
        .iter()
        .filter(|token| token.kind == crate::highlight::Kind::Plain)
        .flat_map(|token| token.text.split(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// The line the Paused call opens on: the innermost block holding the Paused
/// line, read off the indentation [`crate::fold`] already derives rather than
/// from a brace matcher — the same answer in Python as in Rust, and no
/// per-language rule about what a call looks like. A Paused line in no block
/// at all is a script's top level, where everything above it has run.
fn call_start(source: &str, line: usize) -> usize {
    crate::fold::blocks(source)
        .into_iter()
        .filter(|block| block.from < line && line <= block.to)
        .map(|block| block.from)
        .max()
        .unwrap_or(1)
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
    // The Watches with them: the same question asked of the same Frame, so
    // they go where the scopes go rather than at the `stopped` event, which
    // has no Frame to evaluate in yet.
    effects.extend(evaluate_watches(next));
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
fn end(next: &mut State) -> Vec<Effect> {
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
    // left in one that is no longer on screen. The Evaluator is a third: it
    // runs code inside a program, and a window offering to run one with
    // nothing to run it in is a window that can only refuse.
    if matches!(next.focus, Pane::Frames | Pane::Variables | Pane::Evaluator) {
        next.focus = Pane::Editor;
    }
    // The Snippet outlives the window it was written in, whether or not it
    // was ever run: a session ending under a half-written block must not be
    // what loses it.
    let snippet = snippet_text(next);
    next.evaluator = None;
    remember(next, snippet)
}

/// The Hover an `evaluate` was sent for, if this one was: the `hover` context
/// is what names it, and the expression has to still be the one the box is a
/// claim about — a reply for an expression the pointer has moved off belongs
/// to nothing on screen.
fn hover_asked<'a>(next: &'a mut State, arguments: &Value) -> Option<&'a mut Hovered> {
    if arguments["context"] != json!(HOVER) {
        return None;
    }
    let expression = arguments["expression"].as_str()?;
    next.hover
        .as_mut()?
        .value
        .as_mut()
        .filter(|hovered| hovered.expression == expression)
}

/// The Watch an `evaluate` was sent for, found by the expression the request
/// carried rather than by whichever row the keyboard has reached since.
fn watch_asked<'a>(next: &'a mut State, arguments: &Value) -> Option<&'a mut Watch> {
    let expression = arguments["expression"].as_str()?;
    next.watches
        .iter_mut()
        .find(|watch| watch.expression == expression)
}

/// Why the adapter refused, in its own words and stripped of anything that
/// could drive the terminal they are about to be drawn on. The readable text
/// is the error's `format` where the adapter sent one; `message` is often
/// only a short code, and the command it answered is the last resort.
fn adapter_error(message: &Value, command: &str) -> String {
    printable(
        message["body"]["error"]["format"]
            .as_str()
            .or(message["message"].as_str())
            .unwrap_or(command),
    )
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
    // The request itself rather than a copy of the parts of it somebody
    // expected to need: the two could then say different things, and the
    // answer would be filed under the question nobody asked.
    session.asked.insert(
        session.seq,
        Ask {
            command: command.to_string(),
            arguments: arguments.clone(),
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

    /// What `\u{2423}e` names where a Hover would name less: the chain the
    /// cursor stands in, the groups each name in it carries, and the two
    /// shapes that end it early. The scenarios drive one line of Rust; these
    /// are the shapes a real line has that no scenario would be readable
    /// enumerating.
    #[test]
    fn a_cursor_names_the_whole_chain_it_stands_in() {
        let named = |line: &str, column: usize| {
            let mut state = State::default();
            let path = PathBuf::from("/w/one.rs");
            state
                .buffers
                .insert(path.clone(), crate::editor::Buffer::open(line, false, 4));
            let buffer = state.buffers.get_mut(&path).expect("just inserted");
            buffer.line = 1;
            buffer.column = column;
            state.current_buffer = Some(path);
            cursor_expression(&state)
        };
        // On the receiver, and the call at the end of the chain comes too.
        assert_eq!(named("    let n = orders.len();", 17), "orders.len()");
        // Every link of a longer one, from anywhere along it.
        assert_eq!(named("a.b().c[0].d", 1), "a.b().c[0].d");
        assert_eq!(named("a.b().c[0].d", 7), "a.b().c[0].d");
        // A call taking a call is one expression, brackets balanced.
        assert_eq!(named("x.f(g(1)).y", 1), "x.f(g(1)).y");
        // A dot with no name after it ends the chain: a decimal point, and a
        // chain the reader has not finished typing.
        assert_eq!(named("total.", 1), "total");
        assert_eq!(named("n + 1.5", 1), "n");
        // A group nobody closed is a group there is nothing to take.
        assert_eq!(named("a.b(1", 1), "a.b");
        // A place in no name at all names nothing.
        assert_eq!(named("    let n = 7;", 13), "");
    }

    /// What the pointer names, branch by branch. The scenarios drive four
    /// places in one file; these are the shapes a real line has that no
    /// scenario would be readable enumerating.
    #[test]
    fn a_place_names_the_expression_the_syntax_around_it_makes() {
        let named = |line: &str, column: usize| {
            let mut state = State::default();
            let path = PathBuf::from("/w/one.rs");
            state
                .buffers
                .insert(path.clone(), crate::editor::Buffer::open(line, false, 4));
            state.current_buffer = Some(path);
            expression_at(&state, Place { line: 1, column })
        };
        // A name, and the receivers a field is read off — however many.
        assert_eq!(
            named("    let order = load(7);", 9),
            Some(("order".to_string(), 9))
        );
        assert_eq!(
            named("    one.two.three = 1", 13),
            Some(("one.two.three".to_string(), 5))
        );
        // The call a name opens, balanced, so a call taking a call is one
        // expression — and the receiver in front of it comes too.
        assert_eq!(
            named("    delete_order(order.id);", 5),
            Some(("delete_order(order.id)".to_string(), 5))
        );
        assert_eq!(
            named("    let n = a.count(b(c));", 15),
            Some(("a.count(b(c))".to_string(), 13))
        );
        // A receiver that is itself a call or an index comes too, closer
        // first — which is what makes the first of these an expression that
        // calls something rather than a bare `total` the adapter would
        // happily evaluate.
        assert_eq!(
            named("    let n = get().total;", 19),
            Some(("get().total".to_string(), 13))
        );
        assert_eq!(
            named("    let n = v[i].total;", 18),
            Some(("v[i].total".to_string(), 13))
        );
        assert_eq!(
            named("    let n = a.b()[0].c;", 22),
            Some(("a.b()[0].c".to_string(), 13))
        );
        // A chain broken by something that is not an expression stops where
        // it stands rather than reaching past it.
        assert_eq!(
            named("    let n = 1 + .total;", 18),
            Some(("total".to_string(), 18))
        );
        // A literal is no expression: the only one around the `7` below is
        // the call it is an argument to, which is what a Hover may not ask.
        assert_eq!(named("    let order = load(7);", 22), None);
        // And neither is whitespace, punctuation, or a column past the line.
        assert_eq!(named("    let order = load(7);", 15), None);
        assert_eq!(named("    let order = load(7);", 90), None);
    }

    /// The Selection `\u{2423}e` read the expression off is a span of the buffer
    /// *behind* the window, and a run reads a Selection against the Snippet.
    /// Left standing it named columns 17-28 of a one-line Snippet, so the
    /// adapter was sent an empty expression and nothing on screen said so.
    /// Driven here rather than in a scenario because the defect is the absence
    /// of a Selection, and no `Then` can see one that is gone.
    #[test]
    fn opening_on_a_selection_does_not_leave_it_to_be_read_against_the_snippet() {
        let mut state = paused(State::default());
        let path = PathBuf::from("/w/one.rs");
        state.buffers.insert(
            path.clone(),
            crate::editor::Buffer::open("    let count = orders.len();\n", false, 4),
        );
        state.current_buffer = Some(path);
        state.selection = Some(crate::Selection::Buffer {
            anchor: Place {
                line: 1,
                column: 17,
            },
            cursor: Place {
                line: 1,
                column: 28,
            },
        });
        open_evaluator(&mut state, "orders.len()".to_string());
        assert_eq!(running_text(&state), "orders.len()");
    }

    /// An adapter that refuses an `evaluate` the Hover sent: its reason
    /// stands in the box, for the reason a Watch's does. Left waiting, the
    /// box would show the expression with nothing beside it forever, which
    /// reads as a debugger that lost the question.
    #[test]
    fn a_refused_hover_evaluate_says_why_in_the_box() {
        let mut state = paused(State::default());
        let path = PathBuf::from("/w/one.rs");
        state.buffers.insert(
            path.clone(),
            crate::editor::Buffer::open("    let order = load(7);\n", false, 4),
        );
        state.current_buffer = Some(path.clone());
        let at = Place { line: 1, column: 9 };
        crate::lsp::value_hover(&mut state, at);
        let seq = outstanding(&state, "evaluate");
        received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": false,
                "command": "evaluate", "message": "not available"})
            .to_string(),
        );
        assert_eq!(
            state
                .hover
                .as_ref()
                .and_then(|hover| hover.value.as_ref())
                .map(|hovered| hovered.held.clone()),
            Some(Held::Failed("not available".to_string()))
        );
    }

    /// A Hover over a call is the absence this feature exists for, and the
    /// branch that decides it is [`calls`] — driven here over the shapes the
    /// scenarios' one file does not have.
    #[test]
    fn a_hover_asks_for_everything_but_a_call() {
        let mut state = paused(State::default());
        let path = PathBuf::from("/w/one.rs");
        state.buffers.insert(
            path.clone(),
            crate::editor::Buffer::open("    delete_order(order.id);\n", false, 4),
        );
        state.current_buffer = Some(path);
        let (over_call, effects) = hovered(&mut state, Place { line: 1, column: 5 });
        assert_eq!(over_call.map(|box_| box_.held), Some(Held::NeedsEvaluate));
        assert_eq!(effects, Vec::new(), "the adapter was asked to run a call");
        // The argument inside it is asked for, and for itself alone.
        let (over_argument, effects) = hovered(
            &mut state,
            Place {
                line: 1,
                column: 24,
            },
        );
        assert_eq!(
            over_argument.map(|box_| box_.expression),
            Some("order.id".to_string())
        );
        assert_eq!(effects.len(), 1);
    }

    /// The route a real adapter takes to the set-value capability: its
    /// `initialize` reply, which no scenario drives because every scenario
    /// reaches the same field through the `capabilities` event. Both write
    /// the one field, so the Chip cannot be dimmed by one and lit by the
    /// other.
    #[test]
    fn the_initialize_reply_is_where_the_set_value_capability_comes_from() {
        let plain = paused(State::default());
        assert!(!plain.debug.as_ref().expect("a session").can_set);
        let mut can = State::default();
        can.adapters.insert(
            "rust".to_string(),
            crate::startup::Adapter {
                command: "adapter".to_string(),
                args: Vec::new(),
                install: BTreeMap::new(),
            },
        );
        can.launches.insert(
            "app".to_string(),
            crate::startup::Launch {
                adapter: "rust".to_string(),
                request: "launch".to_string(),
                args: serde_json::Map::new(),
            },
        );
        start(&mut can, "app");
        started(&mut can);
        let seq = outstanding(&can, "initialize");
        received(
            &mut can,
            &json!({"type": "response", "request_seq": seq, "success": true,
                "command": "initialize", "body": {"supportsSetVariable": true}})
            .to_string(),
        );
        assert!(can.debug.as_ref().expect("a session").can_set);
        // And the event may take it away again, which is the same field.
        received(
            &mut can,
            r#"{"type":"event","event":"capabilities","body":{"capabilities":{"supportsSetVariable":false}}}"#,
        );
        assert!(!can.debug.as_ref().expect("a session").can_set);
    }

    /// A Watch typed rather than taken off a row — the `a` key's box, which
    /// is the half of "added from a row or typed" no scenario drives. The
    /// same `add_watch` either way, so a Watch typed twice is still one.
    #[test]
    fn a_typed_watch_joins_the_watches_once() {
        let mut state = State::default();
        add_watch(&mut state, "orders.len()".to_string());
        add_watch(&mut state, "orders.len()".to_string());
        add_watch(&mut state, String::new());
        assert_eq!(
            state
                .watches
                .iter()
                .map(|watch| watch.expression.as_str())
                .collect::<Vec<&str>>(),
            ["orders.len()"]
        );
        remove_watch(&mut state, 5);
        assert_eq!(state.watches.len(), 1);
        remove_watch(&mut state, 0);
        assert!(state.watches.is_empty());
    }

    /// The bug this closes: the answer used to be filed against whichever row
    /// the keyboard had reached by the time it arrived, and then against
    /// every member of that name anywhere in the tree — so setting
    /// `first.count` rewrote `second.count` too. No scenario reaches it,
    /// because `variables.feature` sets a member of the one flat scope.
    #[test]
    fn a_set_value_is_filed_against_the_member_that_was_written() {
        let mut state = paused(State::default());
        state.debug.as_mut().expect("a session").can_set = true;
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
                "body": {"variables": [
                    {"name": "first", "value": "…", "variablesReference": 2},
                    {"name": "second", "value": "…", "variablesReference": 3},
                    {"name": "other", "value": "7", "variablesReference": 0}]}})
            .to_string(),
        );
        // Both structs opened, each holding a member called `count`.
        for (row, reference) in [(1, 2), (3, 3)] {
            open(&mut state, row);
            let seq = outstanding(&state, "variables");
            received(
                &mut state,
                &json!({"type": "response", "request_seq": seq, "success": true,
                    "command": "variables", "body": {"variables":
                        [{"name": "count", "value": "0", "variablesReference": 0}]}})
                .to_string(),
            );
            assert_eq!(
                variables(&state)[row].opens,
                Opens::Children {
                    reference,
                    indexed: 0
                }
            );
        }
        // The first struct's `count` is written, and then the selection moves
        // away before the answer lands — which is what used to decide it.
        state.variables_selection = 2;
        let effects = set_value(&mut state, "9".to_string());
        assert_eq!(effects.len(), 1, "one setVariable");
        // Onto a row of another name entirely, which is what the selection
        // read at reply time would have written instead.
        state.variables_selection = 5;
        let seq = outstanding(&state, "setVariable");
        received(
            &mut state,
            &json!({"type": "response", "request_seq": seq, "success": true,
                "command": "setVariable", "body": {"value": "9"}})
            .to_string(),
        );
        let written: Vec<(String, String)> = variables(&state)
            .into_iter()
            .map(|row| (row.expression, row.value))
            .collect();
        assert_eq!(
            written,
            [
                // The scope heading, which has a name and no value.
                (String::new(), String::new()),
                ("first".to_string(), "…".to_string()),
                ("first.count".to_string(), "9".to_string()),
                ("second".to_string(), "…".to_string()),
                ("second.count".to_string(), "0".to_string()),
                ("other".to_string(), "7".to_string()),
            ]
        );
    }

    /// A member's expression is the path a reader could type: an index
    /// carries on from its container, a field is reached through a dot, and a
    /// scope is neither — it is a heading the program does not know.
    #[test]
    fn an_expression_is_the_path_to_the_member() {
        assert_eq!(joined("", "orders"), "orders");
        assert_eq!(joined("orders", "[0]"), "orders[0]");
        assert_eq!(joined("orders[0]", "id"), "orders[0].id");
    }

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

    /// A name the grammar coloured as something else is not a variable, which
    /// is what keeps a value off a line that only talks about one. No rule
    /// here says which languages have comments or strings.
    #[test]
    fn only_the_names_the_grammar_left_plain_are_variables() {
        let tokens = crate::highlight::highlight("main.rs", "    let total = 0; // count");
        assert_eq!(mentioned(&tokens[0]), ["total"]);
        let tokens = crate::highlight::highlight("main.rs", "    let total = \"count\";");
        assert_eq!(mentioned(&tokens[0]), ["total"]);
    }

    /// The call the Paused line is in, and not the one above it: a value drawn
    /// on a line of the function before this one is a value from a call that
    /// is not on the stack.
    #[test]
    fn the_call_opens_where_the_block_holding_the_paused_line_opens() {
        let source = "fn a() {\n    let x = 1;\n}\nfn b() {\n    let y = 2;\n}";
        assert_eq!(call_start(source, 5), 4);
    }

    /// A value the line has half the room for is cut where the screen runs
    /// out, which is not where its characters do: two of these glyphs fill
    /// four columns and the third would overrun the one column left.
    #[test]
    fn a_value_is_cut_by_display_width_and_never_by_character_count() {
        assert_eq!(clipped("東京タワー", 5), "東京");
        assert_eq!(clipped("abc", 2), "ab");
    }

    /// Indentation and nothing else, so a language with no braces at all
    /// answers the same question the same way.
    #[test]
    fn a_call_in_a_language_without_braces_opens_the_same_way() {
        let source = "def main():\n    total = 0\n    print(total)";
        assert_eq!(call_start(source, 3), 1);
        // A script's top level is in no block at all, and everything above the
        // Paused line has run.
        assert_eq!(call_start("total = 0\nprint(total)", 2), 1);
    }
}
