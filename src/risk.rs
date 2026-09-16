//! F27 — Risk: how hard the workspace will be to change safely.
//!
//! Every decision about the figure lives here. The edge runs the analyser and
//! converts its tree into [`Space`] field for field, with no decisions in it;
//! which of those spaces is a Function, what the threshold does to them and
//! what the border says are all this module's, under unit test, because the
//! Function rule is what makes the count un-gameable by nesting.
//!
//! Reads nothing, and knows nothing about the analyser: it is pinned pre-1.0,
//! so its types must not reach here or the file format.

use crate::{Effect, Entry, State, View};
use serde::{Deserialize, Serialize};

/// What the figure is called on screen. `CX` rather than `CRAP` because CRAP
/// claims complexity weighted by test coverage and no coverage was read; the
/// concept keeps its name — Risk — so reading coverage one day renames the
/// label and nothing else.
pub const METRIC: &str = "CX";

/// The documented default threshold: above 15 branches in one Function is
/// where a project that has not said otherwise wants to be told. Configurable,
/// because a codebase with different norms sets its own bar — `startup`'s
/// built-in defaults carry this same number, and its tests hold them level.
pub const DEFAULT_THRESHOLD: u32 = 15;

/// Every space kind the analyser distinguishes, so the edge's conversion is a
/// mapping and never a judgement. Only [`Kind::Function`] can become a
/// Function; the rest are containers, read for their children and never
/// counted, which is what stops one `impl`'s branching being counted twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Unknown,
    Function,
    Class,
    Struct,
    Trait,
    Impl,
    Unit,
    Namespace,
    Interface,
}

/// What was measured about one space. Whole numbers: three of the four are
/// counts, and the maintainability index is rounded because `State` is compared
/// for equality and a float has no `Eq` — a floor of one index point is well
/// below anything the Gate should act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub maintainability: u32,
    pub lines: u32,
}

/// One space in a file, as the analyser found it: a file, a class, an `impl`, a
/// function, a closure. Recursive, because that nesting is the whole input to
/// the Function rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// `None` when the analyser parsed the space but could not name it.
    pub name: Option<String>,
    pub line: usize,
    pub kind: Kind,
    pub metrics: Metrics,
    pub children: Vec<Space>,
}

/// A Function: a function whose enclosing space is not itself a function. A
/// closure is counted toward the Function holding it and never separately, so
/// moving code into a lambda cannot make the figure look better.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub file: String,
    pub name: String,
    pub line: usize,
    pub metrics: Metrics,
}

/// The whole answer for a Scope: the Functions found, and how much of the
/// workspace the figure does not describe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Figures {
    pub functions: Vec<Function>,
    /// Files in a supported language the analyser could not parse, plus the
    /// spaces it parsed and could not name. Carried rather than dropped, so
    /// the figure never claims to describe more of the workspace than it did.
    pub unparsed: usize,
}

impl Figures {
    /// Nothing the analyser handles, and nothing it choked on: a folder in no
    /// language it knows. Distinct from a workspace measured and found clean,
    /// which has Functions in it.
    pub fn nothing_analysed(&self) -> bool {
        self.functions.is_empty() && self.unparsed == 0
    }
}

/// What was measured, and whether it still describes the workspace. A Stale
/// figure keeps the figure it has and says so: an old answer, labelled, beats
/// no answer, and beating it into shape by recomputing on every save is what
/// makes measuring compete with typing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Figure {
    /// Nothing measured yet. In a project CRIME asks for an analysis the moment
    /// it opens, so a workspace with no figure is one being measured rather than
    /// one nobody asked about; a Bare workspace measures only when asked, and
    /// stays here until somebody does.
    #[default]
    None,
    Current(Figures),
    Stale(Figures),
}

/// The workspace's figure, and the analyses behind it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Risk {
    pub figure: Figure,
    /// How many analyses have been asked for, and how many answers have been
    /// taken. They differ exactly while a job is in flight — there is no
    /// separate flag to disagree with them.
    ///
    /// A second request bumps `asked`, so the superseded job answers under a
    /// generation nobody is waiting for and its figures are dropped rather than
    /// overwriting a fresher answer. That is what makes a recompute *supersede*
    /// rather than queue: two analyses of two different workspace states cannot
    /// both still be true, and the one that finishes last is not the one that
    /// describes the workspace.
    pub asked: u64,
    pub answered: u64,
    /// What the latest request covers, so the spinner can say what is being
    /// worked on. Only meaningful while a job is in flight — `analyse` is the
    /// only writer, and `job` the only reader.
    pub scope: Scope,
    /// The reviewed files as the base revision had them. Present exactly while
    /// the figure is a review-scoped one, because a review answer measures both
    /// sides in one job and arrives as a pair — so this is also what says which
    /// Scope the figure describes, and the border cannot read a delta as a count
    /// or the other way round.
    pub before: Option<Figures>,
}

impl Risk {
    /// At most one, by construction: superseding is what the generation is for.
    pub fn in_flight(&self) -> bool {
        self.answered < self.asked
    }

    /// The figure there is, current or stale — the count is the same number
    /// either way, and only the label differs.
    pub fn figures(&self) -> Option<&Figures> {
        match &self.figure {
            Figure::None => None,
            Figure::Current(figures) | Figure::Stale(figures) => Some(figures),
        }
    }
}

/// Asks for a fresh analysis, superseding whatever is in flight. Every request
/// goes through here so that no arm can start a second job by forgetting to
/// bump the generation.
pub fn analyse(state: &mut State, scope: Scope) -> Effect {
    // The Scope's files, and the revision to measure them at as well. `None`
    // for the workspace on both counts: only the edge can enumerate a workspace
    // — the walk is what knows what the project ignores — and the workspace's
    // figure is a count, with nothing it would be a delta from.
    let (files, base) = match scope {
        Scope::Workspace => (None, None),
        Scope::Review => (
            Some(crate::review::list(state)),
            crate::review::base(state).map(str::to_string),
        ),
    };
    state.risk.asked += 1;
    state.risk.scope = scope;
    Effect::AnalyseRisk {
        scope,
        generation: state.risk.asked,
        files,
        base,
    }
}

/// Takes an analysis's answer, unless a later request has superseded it, and
/// says what to cache when there is a commit to cache it against. Both in one
/// function because they are one decision: a figure the core would not show is
/// a figure it must not write.
///
/// No commit means no cache — a folder that is no repository has nothing that
/// could ever say its figure had gone out of date, so a file written there
/// would only ever be believed wrongly. The figure itself still stands; it is
/// only never written.
fn arrived(
    risk: &mut Risk,
    generation: u64,
    figures: Figures,
    commit: Option<&str>,
) -> Option<String> {
    // The latest request, and only while it is still waiting: an answer taken
    // twice would rewrite the cache for a figure already shown, and `answered`
    // would stop meaning what it says.
    if generation != risk.asked || !risk.in_flight() {
        return None;
    }
    risk.answered = generation;
    let cache = commit.map(|commit| persist(&figures, commit));
    risk.figure = Figure::Current(figures);
    cache
}

/// The workspace has moved, so the figure describes code that is no longer
/// there. It keeps the figure it has and is labelled stale; nothing is
/// recomputed, because a save must never start a job.
pub fn went_stale(risk: &mut Risk) {
    if let Figure::Current(figures) = &risk.figure {
        risk.figure = Figure::Stale(figures.clone());
    }
}

/// Where the figure is cached, in [`crate::crime_dir`]. Derived per-user data
/// that changes on every commit, so it is ignored rather than committed — a
/// committed cache is a merge conflict in a file nobody reads by hand.
pub const FILE: &str = "risk.json";

/// The figure as it is written: CRIME's own shape, a flat list of Functions
/// plus the commit it was measured at and which metric produced it. The
/// analyser is pinned pre-1.0, so its types may not reach the file format —
/// its next release must not silently change what CRIME wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persisted {
    pub commit: String,
    pub metric: String,
    pub functions: Vec<Function>,
    pub unparsed: usize,
}

/// The bytes to write, so that reopening on unchanged code shows the figure
/// instead of measuring the workspace again.
pub fn persist(figures: &Figures, commit: &str) -> String {
    // Numbers and strings all the way down, so there is no value here serde can
    // refuse. An empty file written silently is the alternative, and `cached`
    // would reject it on every reopen while nothing ever said why.
    serde_json::to_string_pretty(&Persisted {
        commit: commit.to_string(),
        metric: METRIC.to_string(),
        functions: figures.functions.clone(),
        unparsed: figures.unparsed,
    })
    .expect("CRIME's own shape holds nothing serde can refuse")
}

/// The cached figure, if it still describes the workspace. A file recording
/// another commit is not stale-but-usable, it is an answer about code that is
/// not checked out — so it is discarded and the analysis runs.
pub fn cached(json: &str, head: &str) -> Option<Figures> {
    let saved: Persisted = serde_json::from_str(json).ok()?;
    // The metric is checked, not merely recorded: a figure measured under a
    // metric this CRIME does not compute — coverage-weighted CRAP, one day —
    // read as a CX figure is a number labelled with the wrong promise.
    (saved.commit == head && saved.metric == METRIC).then_some(Figures {
        functions: saved.functions,
        unparsed: saved.unparsed,
    })
}

/// Which Scope an analysis covers: the whole workspace, or the files under
/// review. Never a mix — one figure describing two different sets of files is a
/// figure nobody can act on — and the review-scoped loop shares this machinery
/// rather than having its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Workspace,
    Review,
}

impl Scope {
    /// The name a scenario knows the Scope by, and the word the prompt hands
    /// the session. One spelling, so the two cannot drift.
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Workspace => "workspace",
            Scope::Review => "review",
        }
    }
}

/// The Scope a gesture in the view on screen acts on. The view, not the last
/// analysis that ran: a recompute is always the workspace's, so reading the
/// analysis would offer a workspace loop from Review view — over files nobody is
/// reviewing, while the border beside the action showed the change's delta. The
/// Scope the number on screen describes is the Scope the action beside it takes.
pub fn on_screen(view: View) -> Scope {
    match view {
        View::Review => Scope::Review,
        View::Edit | View::Story => Scope::Workspace,
    }
}

/// The whole answer for a Scope, from what the edge could read: one entry per
/// file in a language the analyser handles, carrying its space tree, or `None`
/// where the file would not read or would not parse. Counting those here rather
/// than at the edge is what keeps "a supported file CRIME could not read is
/// Unparsed" a rule with a test rather than an `if` in `main.rs`.
pub fn figures(analysed: Vec<(String, Option<Space>)>) -> Figures {
    let mut all = Vec::new();
    let mut unparsed = 0;
    for (file, space) in analysed {
        match space {
            Some(space) => {
                let (found, unnamed) = functions(&file, &space);
                all.extend(found);
                unparsed += unnamed;
            }
            None => unparsed += 1,
        }
    }
    Figures {
        functions: all,
        unparsed,
    }
}

/// Every Function in one file's space tree, and how many spaces the analyser
/// could not name. A space with no name is not a Function — nothing could be
/// opened at it or asked about — so it is Unparsed instead of a row nobody can
/// act on.
pub fn functions(file: &str, root: &Space) -> (Vec<Function>, usize) {
    let mut found = Vec::new();
    let mut unnamed = 0;
    walk(file, root, &mut found, &mut unnamed);
    (found, unnamed)
}

/// The descent stops at the first function it meets, which is the Function
/// rule: everything under a function — a closure, a nested function, an `impl`
/// declared in a body — is already in that function's figure, because the
/// analyser's parent metrics include every child's contribution.
///
/// Only an unnamed *function* is Unparsed. An unnamed container is walked for
/// its children like any other: nothing was going to be counted for it, so
/// there is nothing the missing name cost.
fn walk(file: &str, space: &Space, found: &mut Vec<Function>, unnamed: &mut usize) {
    if space.kind == Kind::Function {
        match &space.name {
            Some(name) => found.push(Function {
                file: file.to_string(),
                name: name.clone(),
                line: space.line,
                metrics: space.metrics,
            }),
            None => *unnamed += 1,
        }
        return;
    }
    for child in &space.children {
        walk(file, child, found, unnamed);
    }
}

/// How many Functions sit *above* the threshold — strictly above, so a
/// threshold names the highest figure a project is content with rather than the
/// lowest it objects to.
pub fn count(functions: &[Function], threshold: u32) -> usize {
    functions
        .iter()
        .filter(|function| function.metrics.cyclomatic > threshold)
        .count()
}

/// What the figure amounts to, as one value. A folder with nothing the analyser
/// handles is not the same situation as one measured and found clean, so they
/// are different answers: a fabricated zero would read as a clean bill of
/// health nobody was given.
///
/// An enum rather than the name alone because two surfaces read this — the
/// scenarios by name, the Risk pane's border in its own words — and a `&str`
/// they each match on is a `_` arm away from a fifth situation drawing as one
/// of the four. Both matches below are exhaustive over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Computing,
    NothingAnalysed,
    Stale,
    Computed,
}

/// The figure measured for a Scope, where the one figure CRIME holds is that
/// Scope's. Which Scope it describes is `Risk::before`'s presence: a review
/// answer measures both sides of the change and arrives as a pair, and a
/// workspace answer has no other side. Anything else is the wrong figure —
/// reviewed files counted as a workspace, or a workspace's own read as a delta.
///
/// Keyed on the Scope and never on the view, because the Scope is a value the
/// loop carries: a Gate reading the figure the *screen* shows would judge a
/// review-scoped pass against the workspace's counts the moment the user left
/// the view, and accept it.
fn for_scope(risk: &Risk, scope: Scope) -> Option<&Figures> {
    let describes = match scope {
        Scope::Workspace => risk.before.is_none(),
        Scope::Review => risk.before.is_some(),
    };
    describes.then(|| risk.figures()).flatten()
}

/// The figure for the Scope on screen, which is the only figure the pane, the
/// border and the list may be read off: the workspace's own describes files
/// nobody is reviewing, and showing it in Review view — as a count, as a list,
/// or as a stale delta — is the wrong number in the wrong place. There is always
/// an analysis behind it, because entering the view asks for one.
fn scoped(state: &State) -> Option<&Figures> {
    for_scope(&state.risk, on_screen(state.view))
}

pub fn standing(state: &State) -> Standing {
    // Nothing measured for this Scope yet: no figure at all, or a figure
    // measured for another one — which is not the same as being measured, and
    // only a job in flight is that. A Bare workspace analyses nothing unasked,
    // so `Computing` there would name a job that will never answer. It lands in
    // `NothingAnalysed` instead: no figure, and a recompute is how one arrives,
    // which is the standing's whole meaning either way in (R27.1, R27.4).
    let Some(figures) = scoped(state) else {
        return if state.risk.in_flight() {
            Standing::Computing
        } else {
            Standing::NothingAnalysed
        };
    };
    // Ahead of staleness: a workspace in no language the analyser handles does
    // not become a stale figure by being edited, it stays a workspace nothing
    // was measured in.
    if figures.nothing_analysed() {
        return Standing::NothingAnalysed;
    }
    match state.risk.figure {
        Figure::Stale(_) => Standing::Stale,
        Figure::Current(_) | Figure::None => Standing::Computed,
    }
}

/// The name the scenarios know each standing by.
pub fn view_state(state: &State) -> &'static str {
    match standing(state) {
        Standing::Computing => "computing",
        Standing::NothingAnalysed => "nothing-analysed",
        Standing::Stale => "stale",
        Standing::Computed => "computed",
    }
}

/// The Risk count, once there is one to show. `None` while the job runs, and
/// for a workspace where nothing was analysed — the same absence, because in
/// both cases there is no figure to act on.
pub fn risk_count(state: &State) -> Option<usize> {
    scoped(state)
        .filter(|figures| !figures.nothing_analysed())
        .map(|figures| count(&figures.functions, state.risk_threshold))
}

/// The Risk list, worst first: a worklist, so only the Functions above the
/// threshold — unless everything was asked for, which is how a figure that is
/// not yet a problem is read. Sorted on the figure alone, across the whole
/// Scope: one ordering rule is worth more than rows grouped under file
/// headings, and the pane is narrower than a path anyway.
///
/// Borrowed rather than cloned: the pane draws it and nothing edits it.
pub fn list(state: &State) -> Vec<&Function> {
    // Every Function in Review view, whatever its figure: the rows are there to
    // say which part of the change moved, and a Function the change made worse
    // while staying under the threshold is exactly the row a reviewer wants.
    let all = state.risk_all || state.view == View::Review;
    worst_first(scoped(state))
        .into_iter()
        .filter(|function| all || function.metrics.cyclomatic > state.risk_threshold)
        .collect()
}

/// A figure's Functions, worst first — the pane's order, and the order the
/// prompt names the worst few in, because the two must agree about which
/// Functions those are.
///
/// Stable, so two Functions on the same figure keep the order the analyser found
/// them in rather than swapping places between frames.
fn worst_first(figures: Option<&Figures>) -> Vec<&Function> {
    let mut rows: Vec<&Function> = figures
        .map(|figures| figures.functions.iter().collect())
        .unwrap_or_default();
    rows.sort_by_key(|function| std::cmp::Reverse(function.metrics.cyclomatic));
    rows
}

/// The row the keyboard is on, if the list has one there. `get` rather than an
/// index into a list assumed long enough: the figure is replaced wholesale when
/// an analysis answers, so the selection can outlive the row it was on, and a
/// row that is not there is nothing to open and nothing to name on the border.
///
/// The pane is narrower than a path, so the file it names is what the border
/// shows for this row rather than the row itself — one ordering rule across the
/// whole Scope, instead of rows grouped under file headings.
pub fn selected(state: &State) -> Option<&Function> {
    list(state).get(state.risk_selection).copied()
}

/// The one action a Risk row offers: ask an AI session for a refactor of that
/// Function. Named once, because the core routes it, the mouse hit-tests it and
/// the pane draws it, and three spellings of one string is three places for it
/// to drift.
pub const REFACTOR: &str = "refactor-function";

/// What the row the keyboard is on offers. Only that row, the way a tree row
/// does, so the pane draws one icon rather than a column of them — and nothing
/// at all when the selection has outlived the row it was on.
pub fn row_actions(state: &State) -> Vec<&'static str> {
    match selected(state) {
        Some(_) => vec![REFACTOR],
        None => Vec::new(),
    }
}

/// Whether the keyboard is past the last row and on the pane's own actions —
/// the recompute and the loop, which the mouse reaches by clicking their icons
/// on the border. One slot past the end, the way the Story spine's Remainder is,
/// rather than a second field beside `risk_selection`: two fields for one
/// position is two fields that can disagree about where the keyboard is.
///
/// True with an empty list as well, and deliberately: a Scope with no rows is
/// exactly the one where the recompute is the only thing left to reach, and a
/// pane whose actions only appear once it has rows is a pane you cannot
/// recompute your way out of.
pub fn on_actions(state: &State) -> bool {
    state.focus == crate::Pane::Risk && state.risk_selection >= list(state).len()
}

/// The ask handed to an AI session for one Function. It carries what CRIME
/// already measured — the Function, its file and its figure — so the session
/// does not spend a turn rediscovering it, and it says what is *not* wanted:
/// this is one prompt with no Gate behind it, so a commit would be a change
/// nobody reviewed — and, for the same reason, so would a Function shredded
/// into helpers nobody can name. The Gate's third condition would revert that
/// in the loop; here the wording is the whole of the defence.
///
/// Generic, and it names no provider: whatever CLI the user already starts is
/// the one that reads this.
pub fn refactor_prompt(function: &Function) -> String {
    format!(
        "Refactor the function `{name}` in {file}, at line {line}, to lower its complexity \
without changing its behaviour. Its {METRIC} figure is {figure} (cyclomatic {figure}, \
cognitive {cognitive}), measured over this working tree.\n\n\
The figure is the symptom, not the goal: extract only functions whose one responsibility \
their own name states, and where it cannot be split that way leave it as it is and tell me \
why, rather than trading nested complexity for structural scattering into helpers called \
from a single place and named after where they were cut from.\n\n\
Change nothing else, follow the convention files this repository holds, and do not commit: \
the change is for me to review in the working tree.",
        name = function.name,
        file = function.file,
        line = function.line,
        figure = function.metrics.cyclomatic,
        cognitive = function.metrics.cognitive,
    )
}

// ---- F29: the Refactor loop ----

/// Where the session says a pass is finished. A file rather than a line of
/// output, because CRIME may not read what a hosted pane prints
/// (`docs/adr/0004-hosted-panes-are-transparent.md`): completion is made a
/// filesystem fact the watcher already sees, and any CLI that can edit files
/// can write one. Its contents are ignored — CRIME recomputes the real figures
/// itself (`docs/adr/0010-crime-owns-the-test-gate.md`).
pub const SENTINEL: &str = "refactor-done";

/// The documented default cap: ten Iterations, so a loop cannot run all
/// afternoon while nobody watches. Configurable as `risk.max_iterations`;
/// `startup`'s built-in defaults carry this same number and a test there holds
/// the two level.
pub const DEFAULT_MAX_ITERATIONS: u32 = 10;

/// The project shapes a test command is read off, first match winning. A table
/// rather than a probe of what is installed: the file the project holds is what
/// says which command its tests are behind, and a Rust project that also has a
/// `Makefile` still wants the first line. Configuration overrides all of it,
/// which is the answer for a project this table gets wrong.
const SHAPES: [(&str, &str); 8] = [
    ("Cargo.toml", "cargo test"),
    ("package.json", "npm test"),
    ("pyproject.toml", "pytest"),
    ("go.mod", "go test ./..."),
    ("pom.xml", "mvn test"),
    ("build.gradle", "gradle test"),
    ("build.gradle.kts", "gradle test"),
    ("Makefile", "make test"),
];

/// What an Iteration is waiting for. The wait for the session has no timeout,
/// deliberately — a session thinking for forty seconds looks exactly like one
/// that finished — and that is only survivable because the pane can say which
/// of these it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    Session,
    Tests,
    /// Waiting for one particular analysis: the generation the Iteration asked
    /// for. Carried, because a recompute the *user* fires mid-Iteration
    /// supersedes that request and answers under a generation of its own — and
    /// gating a pass on a measurement nobody in the loop asked for would judge
    /// it against a tree the user may have edited, and then restore over it.
    Figures(u64),
}

impl Wait {
    pub fn as_str(self) -> &'static str {
        match self {
            Wait::Session => "waiting-for-session",
            Wait::Tests => "waiting-for-tests",
            Wait::Figures(_) => "waiting-for-figures",
        }
    }

    /// What the pane says it is waiting for. A caption beside the Iteration
    /// rather than a spinner alone: the wait for the session has no timeout on
    /// purpose, and an unlabelled wait is indistinguishable from a hang
    /// (`docs/adr/0009-a-spinner-is-bounded-by-its-job.md`).
    ///
    /// One word each, and a noun rather than a sentence, because the pane is
    /// thirty columns wide and its border also carries its name and its two
    /// action icons. A caption that does not fit is a caption nobody reads,
    /// which is the hang all over again; the sentences live in `main.rs`'s
    /// notice table, on a footer as wide as the screen.
    fn caption(self) -> &'static str {
        match self {
            Wait::Session => "session",
            Wait::Tests => "tests",
            Wait::Figures(_) => "measuring",
        }
    }
}

/// The Iteration in flight: which pass it is, what will be run to judge it, and
/// what it is waiting for. The test command is resolved once, when the loop
/// starts, so an edit to the config mid-run cannot change what the Gate
/// measures with half way through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iteration {
    pub scope: Scope,
    pub number: u32,
    pub test_command: String,
    pub wait: Wait,
    /// The figures this Iteration is judged against: the ones measured before
    /// its session was let at the files. Carried by the Iteration rather than
    /// read off `Risk` when the Gate runs, because by then `Risk` holds the
    /// *new* figures and the old ones are gone.
    ///
    /// `None` until that baseline exists. A review-scoped loop is started from
    /// a view that asked for its analysis on the way in, so the figure the Gate
    /// judges against is still being measured when the loop begins — the answer
    /// that view is waiting for *is* the baseline, and it is taken as one the
    /// moment it lands. A Gate reached without one judges nothing: it reverts
    /// and says so, rather than reading an honest pass as no improvement.
    pub before: Option<Figures>,
}

/// The loop, running or stopped. One struct rather than fields on `State`,
/// because a stopped loop still has to answer for itself: why it stopped and
/// what the last test run said are what the user reads after it is gone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Refactor {
    pub running: Option<Iteration>,
    /// Why a start was refused. A refusal rather than a silent no-op: a Gate
    /// that reports a pass having run nothing is the one failure mode worse
    /// than no Gate at all.
    pub refusal: Option<&'static str>,
    /// Which Gate condition stopped the loop. "It gave up" is a diagnosis, not
    /// a mystery.
    pub stopped: Option<&'static str>,
    /// The last test run: whether it passed, and what it printed. The output is
    /// kept because a bare "tests failed" is a report nobody can act on.
    pub tests: Option<(bool, String)>,
}

impl Refactor {
    /// The last test run, as the pane says it. `None` before anything ran —
    /// distinct from a run that failed, which is a result.
    pub fn last_test(&self) -> Option<&'static str> {
        self.tests
            .as_ref()
            .map(|(passed, _)| if *passed { "passed" } else { "failed" })
    }
}

/// What the Gate will run: configuration if the project set one, else the
/// project's shape. `None` refuses the loop — CRIME never reports a passing
/// Gate having run nothing.
pub fn test_command(configured: Option<&str>, entries: &[Entry]) -> Option<String> {
    if let Some(command) = configured
        .map(str::trim)
        .filter(|command| !command.is_empty())
    {
        return Some(command.to_string());
    }
    SHAPES
        .iter()
        .find(|(marker, _)| {
            entries
                .iter()
                .any(|entry| !entry.is_dir && entry.name == *marker)
        })
        .map(|(_, command)| command.to_string())
}

/// Why this start cannot become an Iteration, if it cannot. Ahead of the test
/// command, because both of these are true before anything about the project
/// has been read, and a refusal is never a silent no-op: a Gate that reports a
/// pass having measured nothing is the one failure mode worse than no Gate.
fn refused(state: &State, scope: Scope) -> Option<&'static str> {
    if state.refactor.running.is_some() {
        return Some(LOOP_ALREADY_RUNNING);
    }
    // A measured figure of zero is a baseline, and a Scope the analyser handled
    // nothing in is one the Gate can still judge. What cannot be judged is a
    // baseline nobody has taken and nobody asked for — so this Scope's own
    // analysis, still in flight, is enough: the answer it is waiting for is the
    // baseline, and it is taken as one the moment it lands. One rule for both
    // Scopes, which is the point; entering Review view happens to satisfy it
    // every time, because entering asks for that analysis.
    let coming = state.risk.in_flight() && state.risk.scope == scope;
    (for_scope(&state.risk, scope).is_none() && !coming).then_some(NO_FIGURE)
}

/// Starts the loop over a Scope: the session is handed the Scope and the goal,
/// and the judgement stays CRIME's. The sentinel goes first, so a leftover from
/// a previous pass cannot complete this one instantly, and the files are
/// snapshotted before the session is let at them, so a failed Gate has
/// somewhere to go back to.
pub fn start(state: &mut State, scope: Scope) -> Vec<Effect> {
    if let Some(refusal) = refused(state, scope) {
        state.refactor.refusal = Some(refusal);
        return vec![Effect::Notify(refusal)];
    }
    state.refactor.refusal = None;
    let entries = state.contents.get(&state.root).cloned().unwrap_or_default();
    let Some(test_command) = test_command(state.test_command.as_deref(), &entries) else {
        state.refactor.refusal = Some("no-test-command");
        return vec![Effect::Notify("no-test-command")];
    };
    let prompt = loop_prompt(state, scope);
    state.refactor = Refactor {
        running: Some(Iteration {
            scope,
            number: 1,
            test_command,
            wait: Wait::Session,
            before: for_scope(&state.risk, scope).cloned(),
        }),
        ..Refactor::default()
    };
    let mut effects = vec![
        Effect::DeleteFile(crate::crime_dir(&state.root, state.sidecar.as_deref()).join(SENTINEL)),
        Effect::Snapshot { iteration: 1 },
    ];
    effects.extend(crate::queue_for_ai(state, prompt));
    effects
}

/// The sentinel appeared, so the pass is reported finished and the tests run.
/// Nothing else ends that wait: silence is not completion, and a timeout would
/// mean measuring a half-written edit.
pub fn pass_reported(state: &mut State) -> Vec<Effect> {
    let Some(iteration) = state.refactor.running.as_mut() else {
        return vec![];
    };
    if iteration.wait != Wait::Session {
        return vec![];
    }
    iteration.wait = Wait::Tests;
    vec![Effect::RunTests {
        command: iteration.test_command.clone(),
    }]
}

/// The Gate's first condition, and the only one that can be applied without
/// measuring again: the tests still pass. Failing puts the Iteration back and
/// stops the loop; passing asks for the figures the other two conditions are
/// over.
pub fn tests_finished(state: &mut State, passed: bool, output: String) -> Vec<Effect> {
    let Some(iteration) = state.refactor.running.clone() else {
        return vec![];
    };
    state.refactor.tests = Some((passed, output.clone()));
    if passed {
        let asked = analyse(state, iteration.scope);
        state.refactor.running = Some(Iteration {
            wait: Wait::Figures(state.risk.asked),
            ..iteration
        });
        return vec![asked];
    }
    revert(state, TESTS_FAILED, &iteration, &output)
}

/// The stop gesture. The Iteration in flight is put back, so the workspace is
/// where the last accepted Iteration left it rather than holding half a pass an
/// unfinished session was still writing — the same restore a failed Gate does,
/// and to the same place, so the edits that were the user's before the loop ran
/// survive it. Nothing is committed, here as everywhere in the loop.
///
/// Not the escape key: it is heavily overloaded, and a stray press midway
/// through a pass would throw the pass away. It is the start action's own
/// place, flipped.
pub fn stop(state: &mut State) -> Vec<Effect> {
    let Some(iteration) = state.refactor.running.clone() else {
        return Vec::new();
    };
    state.refactor.running = None;
    state.refactor.stopped = Some(STOPPED);
    vec![
        Effect::RestoreSnapshot {
            iteration: iteration.number,
        },
        Effect::Notify(STOPPED),
    ]
}

/// The actions the Risk *pane* offers, as opposed to a row's: the recompute,
/// and the loop — started or stopped from the one place, because a stop
/// somewhere else is a stop nobody finds while the thing they want to stop is
/// editing their files.
pub const RECOMPUTE: &str = "recompute-risk";
pub const START_LOOP: &str = "start-refactor-loop";
pub const STOP_LOOP: &str = "stop-refactor-loop";

/// What the pane offers, in order. The same slot holds the start and the stop,
/// so the icon flips rather than the row growing a second one — which is what
/// makes "stopping is exactly where starting was" a property of this list and
/// not of a renderer.
pub fn pane_actions(state: &State) -> Vec<&'static str> {
    vec![RECOMPUTE, loop_action(state)]
}

/// Which of the two the loop's one slot is holding. Named rather than reached
/// by index, so the key that runs it and the icon that draws it agree by
/// construction: a key that started a loop while the icon beside it said stop
/// is the drift one shared list exists to prevent.
pub fn loop_action(state: &State) -> &'static str {
    match state.refactor.running {
        Some(_) => STOP_LOOP,
        None => START_LOOP,
    }
}

/// What the pane says about the loop: the Iteration out of the cap, what the
/// wait is for, and what the last test run said — and, once it is over, the
/// condition that stopped it. `None` when no loop has run, which is the border
/// saying nothing rather than saying "idle".
pub fn status(state: &State) -> Option<String> {
    let Some(iteration) = state.refactor.running.as_ref() else {
        // The condition, read as words rather than as its slug: the slug is the
        // vocabulary the scenarios and the footer's table share, and hyphens on
        // a border are a slug leaking onto the screen.
        return state
            .refactor
            .stopped
            .map(|condition| condition.replace('-', " "));
    };
    // The cap beside the number, because "iteration 7" alone says nothing about
    // how much of the run is left and the loop's other exit is the stop action.
    // A ratio rather than words for the same reason the captions are one word.
    let mut said = format!(
        "{number}/{cap} {wait}",
        number = iteration.number,
        cap = state.max_iterations,
        wait = iteration.wait.caption(),
    );
    // A tick, not a sentence: what the run said is in the pane's own report
    // (`Refactor::tests`) and in the footer, and this border has four columns
    // to spare.
    if let Some((passed, _)) = state.refactor.tests {
        said.push(' ');
        said.push(if passed { '✓' } else { '✗' });
    }
    Some(said)
}

/// The Gate's conditions, by name: what stopped the loop, and — since each one
/// is also the notice the footer draws — the word the edge looks up to say so.
/// Spelled once, like `REFACTOR`: `main.rs`'s notice table falls back to an
/// empty footer for a slug it does not know, so a fourth drifting spelling is a
/// loop that stops and says nothing.
pub const TESTS_FAILED: &str = "tests-failed";
pub const NO_IMPROVEMENT: &str = "no-improvement";
pub const OTHER_METRIC_WORSENED: &str = "other-metric-worsened";
pub const CAP_REACHED: &str = "cap-reached";

/// Why a start was refused, and why a run ended without the Gate closing.
/// Spelled beside the Gate's conditions because the pane and the footer read
/// all six through one table.
///
/// A second loop is refused rather than queued: two sessions editing the same
/// files is two agents fighting, and the pass that lands is whichever wrote
/// last. A loop with no figure and none on the way is refused for the reason
/// `no-test-command` is — a Gate judging a pass against a baseline of zero can
/// only ever read as `no-improvement`, so it would revert an honest first pass
/// and stop. The same slug says a run *ended* that way, which is the same
/// missing baseline reached one step later: the answer the loop started on was
/// superseded before it landed.
pub const LOOP_ALREADY_RUNNING: &str = "loop-already-running";
pub const NO_FIGURE: &str = "no-figure";
/// The Gate had nothing to judge the pass against: the answer the loop started
/// on was superseded before it landed. Its own condition rather than the
/// refusal's, because this one put a pass back — a refusal never ran anything,
/// and a report that does not say the work was reverted is a report nobody can
/// act on.
pub const NO_BASELINE: &str = "no-baseline";
pub const STOPPED: &str = "stopped";

/// What the Gate made of an Iteration, once its tests have passed. Two ways to
/// fail rather than one, because "it gave up" is a diagnosis and not a mystery:
/// the loop reverted a plateau and reverted a gamed pass for different reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Passed,
    NoImprovement,
    OtherMetricWorsened,
}

/// Every recorded metric, summed over the Scope. Every field named, with no
/// `..Default::default()` to fall back on, so a fifth metric does not compile
/// until somebody has said what the Gate does with it — a Gate blind to a
/// metric it records is the blind spot the third condition exists to close.
fn totals(figures: &Figures) -> Metrics {
    figures
        .functions
        .iter()
        .fold(Metrics::default(), |sum, function| Metrics {
            cyclomatic: sum.cyclomatic + function.metrics.cyclomatic,
            cognitive: sum.cognitive + function.metrics.cognitive,
            maintainability: sum.maintainability + function.metrics.maintainability,
            lines: sum.lines + function.metrics.lines,
        })
}

/// The Gate's other two conditions, over the figures before an Iteration and
/// the figures after it: the primary figure moved down, and no other recorded
/// metric moved up.
///
/// Improvement is the Risk count **or** the primary total, because chipping a
/// Function from 31 to 26 under a threshold of 20 is progress that has not
/// crossed the line yet — counting the count alone reads a genuine first pass
/// as zero and gives up on it (R29.10).
///
/// The other metrics are compared as totals, each in the direction that means
/// worse for that metric. Cognitive complexity and lines worsen upward. The
/// maintainability index worsens *downward* — `main.rs` fills it from
/// `mi_visual_studio`, where a higher number is easier to maintain — so reading
/// it as "up is worse" would revert the honest pass this loop exists to
/// commission: lowering a Function's complexity raises its index.
///
/// Summing the index is scale-dependent, so a pass that merges two Functions
/// lowers the total and is put back. That is the conservative direction: the
/// loop stops, naming the condition, rather than accepting a pass it cannot
/// measure. Shredding is caught by cognitive complexity, which is what that
/// metric is in the Gate for (`docs/adr/0010-crime-owns-the-test-gate.md`).
fn gate(before: &Figures, after: &Figures, threshold: u32) -> Verdict {
    let improved = count(&after.functions, threshold) < count(&before.functions, threshold)
        || totals(after).cyclomatic < totals(before).cyclomatic;
    if !improved {
        return Verdict::NoImprovement;
    }
    // Destructured by name, so the exhaustiveness `totals` buys is not spent
    // one line later by an `if` that names three of four fields.
    let Metrics {
        cyclomatic: _,
        cognitive,
        maintainability,
        lines,
    } = totals(before);
    let after = totals(after);
    match after.cognitive > cognitive
        || after.maintainability < maintainability
        || after.lines > lines
    {
        true => Verdict::OtherMetricWorsened,
        false => Verdict::Passed,
    }
}

/// Figures arrived: they are taken unless a later request superseded them,
/// cached where there is a commit to cache against, and — when an Iteration was
/// waiting for them — put through the Gate. One function because they are one
/// decision made in one order: the Gate is evaluated over the figures the core
/// took, never over an answer it dropped.
pub fn figures_arrived(
    state: &mut State,
    generation: u64,
    figures: Figures,
    before: Option<Figures>,
) -> Vec<Effect> {
    // A review-scoped answer is never cached: the file records the workspace's
    // figure at a commit, and read back at the next open a delta over two files
    // would claim the whole workspace was those two files.
    let commit = state.head.clone().filter(|_| before.is_none());
    let mut effects = Vec::new();
    if let Some(contents) = arrived(&mut state.risk, generation, figures, commit.as_deref()) {
        effects.push(Effect::WriteFile {
            path: crate::crime_dir(&state.root, state.sidecar.as_deref()).join(FILE),
            contents,
        });
    }
    // A superseded answer describes a workspace state nobody is waiting on, and
    // gating an Iteration on it would judge the pass by the wrong measurement.
    if state.risk.answered != generation {
        return effects;
    }
    // Both sides of the pair, or neither: the workspace's answer carries no
    // before, and taking it is what puts the border back to a count.
    state.risk.before = before;
    // The baseline a loop started without: an Iteration still waiting for its
    // session has not been judged against anything yet, so this answer is what
    // it will be judged against — if it is an answer about the Iteration's own
    // Scope. This is the review-scoped loop's whole baseline: the analysis the
    // view asked for, answering after the loop began. A recompute the user fired
    // meanwhile is always the workspace's, and taking that would judge a review
    // pass against the workspace's counts and accept it.
    let waiting = state
        .refactor
        .running
        .as_ref()
        .filter(|iteration| iteration.wait == Wait::Session && iteration.before.is_none())
        .map(|iteration| iteration.scope);
    if let Some(scope) = waiting {
        let measured = for_scope(&state.risk, scope).cloned();
        if let Some(iteration) = state.refactor.running.as_mut() {
            iteration.before = measured;
        }
    }
    effects.extend(judge(state, generation));
    effects
}

/// The Gate's last two conditions, applied to the figures that just arrived.
/// Nothing happens unless an Iteration is waiting for *this* analysis: a
/// recompute the user fired mid-Iteration answers under a generation of its
/// own, and the Iteration is still waiting for the one it asked for.
fn judge(state: &mut State, generation: u64) -> Vec<Effect> {
    let Some(iteration) = state.refactor.running.clone() else {
        return Vec::new();
    };
    if iteration.wait != Wait::Figures(generation) {
        return Vec::new();
    }
    // No baseline to judge against: the answer the loop started on was
    // superseded before it landed. The pass goes back and the loop stops saying
    // why, because the alternative is judging it against zero — which reads
    // every honest pass as no improvement and reverts it under the wrong name.
    let Some(before) = iteration.before.clone() else {
        return revert(
            state,
            NO_BASELINE,
            &iteration,
            "the figures the pass would have been judged against were never measured",
        );
    };
    let after = state.risk.figures().cloned().unwrap_or_default();
    let explanation = verdict_report(&before, &after, state.risk_threshold);
    match gate(&before, &after, state.risk_threshold) {
        Verdict::Passed => accept(state, &iteration, after),
        Verdict::NoImprovement => revert(state, NO_IMPROVEMENT, &iteration, &explanation),
        Verdict::OtherMetricWorsened => {
            revert(state, OTHER_METRIC_WORSENED, &iteration, &explanation)
        }
    }
}

/// What the figures did, in words, so a reverted pass is explained rather than
/// reported bare — to the session as much as to the user, which is the same
/// "never silent" rule the failing-tests path follows.
fn verdict_report(before: &Figures, after: &Figures, threshold: u32) -> String {
    let (was, now) = (totals(before), totals(after));
    format!(
        "{METRIC} above {threshold}: {} functions before, {} after. Totals — cyclomatic {} to {}, cognitive {} to {}, maintainability {} to {}, lines {} to {}.",
        count(&before.functions, threshold),
        count(&after.functions, threshold),
        was.cyclomatic,
        now.cyclomatic,
        was.cognitive,
        now.cognitive,
        was.maintainability,
        now.maintainability,
        was.lines,
        now.lines,
    )
}

/// An Iteration that passed the Gate: its changes stand, uncommitted — always,
/// because the loop's whole output is a working tree to review — and the next
/// Iteration begins from the figures this one reached.
///
/// The cap stops the loop *here*, with the work in place: reaching it is not a
/// failed Gate, so nothing is reverted, and the loop says the cap is why it
/// stopped rather than looking like it plateaued.
fn accept(state: &mut State, iteration: &Iteration, after: Figures) -> Vec<Effect> {
    if iteration.number >= state.max_iterations {
        state.refactor.running = None;
        state.refactor.stopped = Some(CAP_REACHED);
        return vec![Effect::Notify(CAP_REACHED)];
    }
    let number = iteration.number + 1;
    state.refactor.running = Some(Iteration {
        number,
        wait: Wait::Session,
        before: Some(after),
        ..iteration.clone()
    });
    let prompt = loop_prompt(state, iteration.scope);
    let mut effects = vec![
        // The same two the loop opened with: a leftover sentinel would complete
        // this Iteration before its session had started on it.
        Effect::DeleteFile(crate::crime_dir(&state.root, state.sidecar.as_deref()).join(SENTINEL)),
        Effect::Snapshot { iteration: number },
    ];
    effects.extend(crate::queue_for_ai(state, prompt));
    effects
}

/// A failed Gate: the Iteration goes back to the snapshot it started from — not
/// to the last commit, which would destroy work that was the user's before the
/// loop ever ran — the loop stops naming the condition, and the session is told
/// as well as the user. A session that believes its reverted edit landed builds
/// whatever is asked next on a false premise.
///
/// Nothing is committed here or anywhere else in the loop: the whole output is
/// one working tree, and the Review view is where it is judged.
fn revert(
    state: &mut State,
    condition: &'static str,
    iteration: &Iteration,
    output: &str,
) -> Vec<Effect> {
    state.refactor.running = None;
    state.refactor.stopped = Some(condition);
    let mut effects = vec![
        Effect::RestoreSnapshot {
            iteration: iteration.number,
        },
        // The condition itself is the notice, so the footer says which of
        // the three closed the Gate rather than naming the commonest one.
        Effect::Notify(condition),
    ];
    effects.extend(crate::queue_for_ai(
        state,
        reverted_prompt(condition, output),
    ));
    effects
}

/// The ask that starts an Iteration. One generic constant carrying no
/// project-specific fact except the data interpolated here: the Scope, where
/// CRIME wrote the figures, the worst few Functions, the target, an
/// instruction to obey whatever convention file the repository holds, and what
/// a good split is. The last of those is not the Gate going soft: the third
/// condition still reverts a shredded pass, but it does so an Iteration late,
/// and an Iteration is the expensive unit here
/// (`docs/adr/0010-crime-owns-the-test-gate.md`).
///
/// The test command is never in it, because CRIME runs the tests — which keeps
/// the most project-specific string in the system out of a prompt that has to
/// work on any workspace. No provider is named: whatever CLI the user already
/// starts is the one that reads this.
fn loop_prompt(state: &State, scope: Scope) -> String {
    // The Scope's own worst, not the screen's: a loop started over one Scope
    // while the other is on the border would otherwise hand the session the
    // wrong Functions, or — where the two disagree about which figure exists —
    // none at all.
    let worst: String = worst_first(for_scope(&state.risk, scope))
        .iter()
        .take(3)
        .map(|function| {
            format!(
                "  - `{name}` in {file}, line {line} — cyclomatic {cyclomatic}, cognitive {cognitive}\n",
                name = function.name,
                file = function.file,
                line = function.line,
                cyclomatic = function.metrics.cyclomatic,
                cognitive = function.metrics.cognitive,
            )
        })
        .collect();
    // The Scope's files by name, where the Scope is a file set: the session
    // cannot read the git status CRIME read, so a Scope named only by its word
    // is a Scope the session has to guess at — and a guess is an edit outside
    // it. Nothing but the prompt keeps the loop inside its Scope; CRIME cannot
    // stop a session touching a file, it can only measure and put the pass
    // back.
    let files = match scope {
        Scope::Workspace => String::new(),
        Scope::Review => format!(
            "Only these files are under review, and they are the whole of what you may change:\n\n{}\n",
            crate::review::list(state)
                .iter()
                .map(|file| format!("  - {file}\n"))
                .collect::<String>(),
        ),
    };
    format!(
        "Lower the Risk in this workspace, over the scope {scope}.\n\n\
{files}\
The target is no function above a {METRIC} figure of {threshold}. I measured the figures \
myself and wrote every one of them to {CRIME_DIR}/{FILE}; the worst are:\n\n\
{worst}\n\
Refactor those to lower their complexity without changing behaviour. Follow the convention \
files this repository holds, change nothing else, and do not commit: the working tree is what \
I review.\n\n\
The figure is the symptom, not the goal. Moving branches somewhere else lowers it without \
making anything clearer, so a split only counts when the pieces stand on their own: every \
function you extract must have one responsibility, and its name must say what that \
responsibility is. A helper called from exactly one place, or named after the part of the \
original it was cut out of rather than after what it does, has traded nested complexity for \
structural scattering and left the code harder to read than it found it. Fewer well-named \
extractions beat many small ones. Where a function cannot be split that way, leave it as it \
is and tell me why rather than shredding it.\n\n\
When the pass is finished, create the file {CRIME_DIR}/{SENTINEL}. Its contents are ignored. I then run \
this project's tests and measure the figures again myself, and put the whole pass back if \
either got worse — so finish and write that file rather than judging the pass yourself.",
        scope = scope.as_str(),
        threshold = state.risk_threshold,
        CRIME_DIR = crate::CRIME_DIR,
    )
}

/// What the session is told about a pass that was put back, carrying the
/// condition and what ran — "never silent" applied to the agent as a consumer
/// of errors.
fn reverted_prompt(condition: &str, output: &str) -> String {
    format!(
        "I put your last pass back: the working tree is as it was before it, and the loop has \
stopped.\n\nThe gate condition that failed is `{condition}`, and what ran said:\n\n{output}\n\n\
Do not redo the pass, and do not try to fix this on your own — wait for what I ask next.",
    )
}

/// How much of the workspace the figure does not describe, for the pane to say
/// so. Zero while there is no figure at all: nothing was measured, so nothing
/// was missed either.
pub fn unparsed(state: &State) -> usize {
    scoped(state).map_or(0, |figures| figures.unparsed)
}

/// The job in flight, if there is one: the name the scenarios know it by, and
/// the caption the border draws beside the spinner. One function because they
/// are one fact — a spinner with no caption is indistinguishable from a hang,
/// which is why the spinner beat static text at all
/// (`docs/adr/0009-a-spinner-is-bounded-by-its-job.md`). Matched exhaustively,
/// so a second Scope cannot acquire a spinner that says nothing.
pub fn job(state: &State) -> Option<(&'static str, &'static str)> {
    state.risk.in_flight().then_some(match state.risk.scope {
        Scope::Workspace => ("workspace-analysis", "measuring Risk"),
        Scope::Review => ("review-analysis", "measuring the change"),
    })
}

/// The spinner's frames, one cell wide each so the border does not change width
/// as it turns.
const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// What the number on the border describes: the workspace's Risk count, or how
/// far the change moved the files under review. Never both, and never the
/// workspace's figure over a diff — the number always describes the Scope on
/// screen, which is the whole reason it is one value and not two fields a
/// renderer chooses between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Count(usize),
    Delta {
        /// How far the reviewed files' primary total moved: negative where the
        /// change lowered it.
        delta: i64,
        worse: bool,
    },
}

/// Which of the two the border is carrying. `None` while nothing has been
/// measured for the Scope on screen, or where nothing in it was in a language
/// the analyser handles — a fabricated zero would read as a clean bill of health
/// nobody was given.
///
/// The delta is over the totals rather than over the rows, so a Function the
/// change deleted is credited and one it added is charged: both are part of what
/// the change did to these files.
pub fn shown(state: &State) -> Option<Shown> {
    let Some(before) = state.risk.before.as_ref() else {
        return risk_count(state).map(Shown::Count);
    };
    let after = scoped(state)?;
    let delta = i64::from(totals(after).cyclomatic) - i64::from(totals(before).cyclomatic);
    Some(Shown::Delta {
        delta,
        worse: delta > 0,
    })
}

/// How far one row's Function moved, for the Risk list to say which part of the
/// change added the risk. `None` outside a review — the workspace's figure is a
/// figure and not a delta.
///
/// A Function the base revision did not have counts its whole figure: the change
/// added it, so it added all of it. Matched on the name within its file rather
/// than on the line, because a Function that only moved down the file did not
/// change.
pub fn row_delta(state: &State, function: &Function) -> Option<i64> {
    let before = state.risk.before.as_ref()?;
    let was = before
        .functions
        .iter()
        .find(|other| other.file == function.file && other.name == function.name)
        .map_or(0, |other| other.metrics.cyclomatic);
    Some(i64::from(function.metrics.cyclomatic) - i64::from(was))
}

/// What the tree pane's border says about Risk, after the pane's own name. One
/// place on screen always answers the same question: the label naming the
/// metric and the count of Functions to fix, and a turning spinner saying what
/// is being measured while a job runs.
///
/// Both at once when there are both. A recompute over a Stale figure must not
/// take the figure off the border while it runs: R27.8 is that a Stale figure
/// still shows the figure it has, and a count that vanishes for the length of
/// an analysis is the dead end the recompute gesture exists to end.
///
/// The frame comes from the edge's tick rather than from the frames the screen
/// happens to draw. Advancing it on incidental frames stalls precisely when the
/// user stops touching anything, which is when they are watching it, and a
/// stalled spinner claims the job died.
pub fn border(state: &State) -> Option<String> {
    let figure = shown(state).map(|shown| {
        let said = match shown {
            Shown::Count(count) => format!("{METRIC} {count}"),
            // Signed always, so a delta of 5 and a count of 5 cannot read alike
            // on the same border, and said in words when the change made things
            // worse: that is the signal a reviewer least reliably gets from
            // reading a diff, so it must be impossible to miss.
            Shown::Delta {
                delta,
                worse: false,
            } => format!("{METRIC} {delta:+}"),
            Shown::Delta { delta, worse: true } => format!("{METRIC} {delta:+} worse"),
        };
        match state.risk.figure {
            // Never drawn as though it were current: the figure describes code
            // the workspace has already moved past, and a number nobody can tell
            // from a fresh one is a number acted on by mistake. Matched
            // exhaustively, so a fourth thing a figure can be cannot quietly
            // draw as a current one.
            Figure::Stale(_) => format!("{said} stale"),
            Figure::Current(_) | Figure::None => said,
        }
    });
    let spinning = job(state).map(|(_, caption)| {
        format!(
            "{} {caption}",
            FRAMES[(state.tick % FRAMES.len() as u64) as usize]
        )
    });
    match (figure, spinning) {
        (Some(figure), Some(spinning)) => Some(format!("{figure} · {spinning}")),
        (figure, spinning) => figure.or(spinning),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(name: Option<&str>, kind: Kind, cyclomatic: u32, children: Vec<Space>) -> Space {
        Space {
            name: name.map(str::to_string),
            line: 1,
            kind,
            metrics: Metrics {
                cyclomatic,
                ..Metrics::default()
            },
            children,
        }
    }

    /// The rule that makes the count un-gameable: a function inside an `impl`
    /// inside a file is one Function, and the containers are read and never
    /// counted, so no branching is counted twice.
    #[test]
    fn a_function_inside_a_container_is_the_only_function_counted() {
        let tree = space(
            Some("src/keys.rs"),
            Kind::Unit,
            40,
            vec![space(
                Some("Router"),
                Kind::Impl,
                31,
                vec![space(Some("route"), Kind::Function, 31, Vec::new())],
            )],
        );
        let (functions, unparsed) = functions("src/keys.rs", &tree);
        assert_eq!(
            functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["route"]
        );
        assert_eq!(unparsed, 0);
    }

    /// A closure counts toward the Function holding it, never separately:
    /// shredding a Function's branching into lambdas moves the figure nowhere.
    #[test]
    fn a_closure_is_counted_toward_the_function_holding_it() {
        let tree = space(
            Some("src/ui.rs"),
            Kind::Unit,
            22,
            vec![space(
                Some("draw"),
                Kind::Function,
                22,
                vec![space(Some("<anonymous>"), Kind::Function, 14, Vec::new())],
            )],
        );
        let (functions, _) = functions("src/ui.rs", &tree);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].metrics.cyclomatic, 22);
    }

    /// A space nothing can be opened at is not a row anybody could act on, so
    /// it is Unparsed rather than a Function with no name.
    #[test]
    fn a_space_the_analyser_could_not_name_is_unparsed() {
        let tree = space(
            Some("src/keys.rs"),
            Kind::Unit,
            57,
            vec![
                space(Some("route"), Kind::Function, 31, Vec::new()),
                space(None, Kind::Function, 26, Vec::new()),
            ],
        );
        let (functions, unparsed) = functions("src/keys.rs", &tree);
        assert_eq!(functions.len(), 1);
        assert_eq!(unparsed, 1);
    }

    /// A file in a supported language that would not read or would not parse
    /// is carried as Unparsed, so the figure can say it describes less than the
    /// whole workspace. Dropping it would be a clean bill of health nobody gave.
    #[test]
    fn a_file_the_analyser_could_not_read_is_unparsed() {
        let assembled = figures(vec![
            (
                "src/keys.rs".to_string(),
                Some(space(
                    Some("src/keys.rs"),
                    Kind::Unit,
                    31,
                    vec![space(Some("route"), Kind::Function, 31, Vec::new())],
                )),
            ),
            ("src/ui.rs".to_string(), None),
        ]);
        assert_eq!(assembled.functions.len(), 1);
        assert_eq!(assembled.unparsed, 1);
        assert!(!assembled.nothing_analysed());
        assert!(figures(Vec::new()).nothing_analysed());
    }

    #[test]
    fn the_count_is_the_functions_above_the_threshold() {
        let (functions, _) = functions(
            "src/keys.rs",
            &space(
                Some("src/keys.rs"),
                Kind::Unit,
                65,
                vec![
                    space(Some("route"), Kind::Function, 31, Vec::new()),
                    space(Some("spell"), Kind::Function, 12, Vec::new()),
                    space(Some("draw"), Kind::Function, 22, Vec::new()),
                ],
            ),
        );
        assert_eq!(count(&functions, 10), 3);
        assert_eq!(count(&functions, 20), 2);
        assert_eq!(count(&functions, 30), 1);
        assert_eq!(count(&functions, 40), 0);
    }

    /// The one place on screen that answers the Risk question, in each of the
    /// three things it can say. Pinned, because the label is a promise: `CX` is
    /// complexity alone, and drawing `CRAP` here would claim test coverage
    /// nobody read.
    #[test]
    fn the_border_names_the_metric_and_the_count() {
        let mut state = State {
            risk_threshold: 20,
            ..State::default()
        };
        assert_eq!(border(&state), None, "no figure and no job: nothing to say");
        state.risk.figure = Figure::Current(Figures::default());
        assert_eq!(border(&state), None);
        state.risk.figure = Figure::Current(one_function());
        assert_eq!(border(&state).as_deref(), Some("CX 1"));
    }

    /// The two rows no scenario reaches, and the reason the delta is over the
    /// totals rather than over the rows: a Function the change added has no
    /// earlier figure, so it is charged all of its own, and one the change
    /// deleted is credited rather than vanishing unremarked.
    #[test]
    fn a_function_the_change_added_is_charged_and_one_it_deleted_is_credited() {
        let function = |name: &str, cyclomatic: u32| Function {
            file: "src/keys.rs".to_string(),
            name: name.to_string(),
            line: 1,
            metrics: Metrics {
                cyclomatic,
                ..Metrics::default()
            },
        };
        let figures = |functions: Vec<Function>| Figures {
            functions,
            unparsed: 0,
        };
        let state = State {
            view: View::Review,
            risk_threshold: 20,
            risk: Risk {
                figure: Figure::Current(figures(vec![function("route", 26), function("new", 7)])),
                before: Some(figures(vec![function("route", 31), function("gone", 10)])),
                ..Risk::default()
            },
            ..State::default()
        };
        assert_eq!(
            shown(&state),
            Some(Shown::Delta {
                delta: -8,
                worse: false
            })
        );
        assert_eq!(border(&state).as_deref(), Some("CX -8"));
        assert_eq!(row_delta(&state, &function("route", 26)), Some(-5));
        assert_eq!(
            row_delta(&state, &function("new", 7)),
            Some(7),
            "a Function the base revision did not have was credited for existing"
        );
        // Every Function in the Scope, whatever its figure: the row is there to
        // say which part of the change moved, and both of these are under the
        // threshold the workspace list filters on.
        let names: Vec<&str> = list(&state)
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert_eq!(names, ["route", "new"]);
        // The worse half, said in words: the signal that must not be missable.
        let worse = State {
            risk: Risk {
                figure: Figure::Current(figures(vec![function("route", 38)])),
                before: Some(figures(vec![function("route", 31)])),
                ..Risk::default()
            },
            ..state.clone()
        };
        assert_eq!(border(&worse).as_deref(), Some("CX +7 worse"));
    }

    /// The border while a job runs: a frame that moves with the tick, and a
    /// caption beside it. Both halves are the point — a spinner with nothing
    /// next to it is the hang `docs/adr/0009-a-spinner-is-bounded-by-its-job.md`
    /// rejected static text for looking like.
    #[test]
    fn the_border_turns_a_spinner_with_a_caption_while_a_job_runs() {
        let mut state = State {
            risk_threshold: 20,
            ..State::default()
        };
        let _ = analyse(&mut state, Scope::Workspace);
        assert_eq!(job(&state), Some(("workspace-analysis", "measuring Risk")));
        let frames: Vec<String> = (0..FRAMES.len() as u64 + 1)
            .map(|tick| {
                state.tick = tick;
                border(&state).expect("a border while the job runs")
            })
            .collect();
        assert!(
            frames.iter().all(|frame| frame.ends_with("measuring Risk")),
            "a spinner with no caption: {frames:?}"
        );
        assert_ne!(frames[0], frames[1], "the spinner stood still");
        assert_eq!(frames[0], frames[FRAMES.len()], "the frames do not cycle");
    }

    /// A recompute over a Stale figure keeps the figure on the border while it
    /// runs. R27.8 is that a Stale figure still shows the figure it has, and a
    /// count that disappears for the length of an analysis is the dead end the
    /// recompute gesture exists to end.
    #[test]
    fn a_recompute_spins_beside_the_figure_it_is_replacing() {
        let mut state = State {
            risk_threshold: 20,
            ..State::default()
        };
        state.risk.figure = Figure::Current(one_function());
        went_stale(&mut state.risk);
        let _ = analyse(&mut state, Scope::Workspace);
        let drawn = border(&state).expect("a border while the job runs");
        assert!(drawn.starts_with("CX 1 stale"), "{drawn}");
        assert!(drawn.ends_with("measuring Risk"), "{drawn}");
        // And what the figure is, said by the one function that says it: the
        // spinner is about the job, never about the figure's own state.
        assert_eq!(view_state(&state), "stale");
    }

    /// The tick is the only thing that turns it, and the tick only exists while
    /// the job does: an answered job leaves a count, and no state spins with
    /// nothing to spin for.
    #[test]
    fn the_count_replaces_the_spinner_when_the_figures_arrive() {
        let mut state = State {
            risk_threshold: 20,
            tick: 7,
            ..State::default()
        };
        let _ = analyse(&mut state, Scope::Workspace);
        assert!(arrived(&mut state.risk, 1, one_function(), None).is_none());
        assert_eq!(job(&state), None);
        assert_eq!(border(&state).as_deref(), Some("CX 1"));
    }

    /// A Stale figure is drawn as the figure it is, marked: the same count, and
    /// never a number a reader could mistake for one measured just now.
    #[test]
    fn a_stale_figure_is_drawn_as_stale() {
        let mut state = State {
            risk_threshold: 20,
            ..State::default()
        };
        state.risk.figure = Figure::Current(one_function());
        went_stale(&mut state.risk);
        assert_eq!(view_state(&state), "stale");
        // Nothing measured stays nothing measured: editing a workspace in no
        // language the analyser handles does not give it a stale figure.
        let mut empty = State::default();
        empty.risk.figure = Figure::Current(Figures::default());
        went_stale(&mut empty.risk);
        assert_eq!(view_state(&empty), "nothing-analysed");
        assert_eq!(risk_count(&state), Some(1));
        assert_eq!(border(&state).as_deref(), Some("CX 1 stale"));
        // Already stale: nothing to demote, and the figure it has must survive.
        went_stale(&mut state.risk);
        assert_eq!(risk_count(&state), Some(1));
    }

    /// The rule that makes a recompute supersede rather than queue. Two jobs
    /// exist and the first one answers last: a core that took the answer it was
    /// handed would show a figure for a workspace state nobody asked about.
    #[test]
    fn a_superseded_analysis_answers_into_nothing() {
        let mut state = State::default();
        let first = analyse(&mut state, Scope::Workspace);
        let second = analyse(&mut state, Scope::Workspace);
        assert_ne!(first, second);
        let risk = &mut state.risk;
        assert!(risk.in_flight());
        assert_eq!(arrived(risk, 1, one_function(), Some("aaaa")), None);
        assert_eq!(risk.figure, Figure::None);
        assert!(risk.in_flight());
        assert!(arrived(risk, 2, one_function(), Some("aaaa")).is_some());
        assert!(!risk.in_flight());
        assert_eq!(
            arrived(risk, 2, Figures::default(), Some("aaaa")),
            None,
            "an answer already taken is not taken again"
        );
        assert_eq!(risk.figure, Figure::Current(one_function()));
    }

    /// A folder that is no repository still gets its figure — it just gets no
    /// file, because nothing there could ever say the figure had gone stale.
    #[test]
    fn a_workspace_with_no_commit_is_measured_and_never_cached() {
        let mut state = State::default();
        let _ = analyse(&mut state, Scope::Workspace);
        let risk = &mut state.risk;
        assert_eq!(arrived(risk, 1, one_function(), None), None);
        assert_eq!(risk.figure, Figure::Current(one_function()));
    }

    /// The cache is only believed for the commit it names: a figure measured at
    /// another commit describes code that is not checked out, which is not a
    /// stale-but-usable answer.
    #[test]
    fn the_cache_is_read_back_only_for_the_commit_it_names() {
        let written = persist(&one_function(), "aaaaaaaaaaaa");
        assert_eq!(cached(&written, "aaaaaaaaaaaa"), Some(one_function()));
        assert_eq!(cached(&written, "bbbbbbbbbbbb"), None);
        assert_eq!(cached("not json", "aaaaaaaaaaaa"), None);
        assert_eq!(
            cached(&written.replace("\"CX\"", "\"CRAP\""), "aaaaaaaaaaaa"),
            None,
            "another metric's figure is not this one's"
        );
    }

    fn one_function() -> Figures {
        Figures {
            functions: vec![Function {
                file: "src/keys.rs".to_string(),
                name: "route".to_string(),
                line: 88,
                metrics: Metrics {
                    cyclomatic: 31,
                    cognitive: 24,
                    maintainability: 41,
                    lines: 96,
                },
            }],
            unparsed: 0,
        }
    }

    /// The border's count and the pane's rows are one number and one list of
    /// the same Functions: a count pointing at rows that are not there is a
    /// worklist nobody can work.
    #[test]
    fn the_list_holds_exactly_the_functions_the_count_counts() {
        let mut state = State {
            risk_threshold: 20,
            ..State::default()
        };
        state.risk.figure = Figure::Current(three_functions());
        assert_eq!(list(&state).len(), risk_count(&state).expect("a figure"));
        assert_eq!(
            list(&state)
                .iter()
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            ["route", "draw"],
            "worst first, and only above the threshold"
        );
        // Everything, for a figure that is not yet a problem — the same
        // Functions in the same order, plus the ones the worklist leaves out.
        state.risk_all = true;
        assert_eq!(
            list(&state)
                .iter()
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            ["route", "draw", "cheatsheet"]
        );
        assert_eq!(unparsed(&state), 3);
        // No figure at all is an empty list rather than a list of nothing
        // measured: the pane says which it is on its border.
        state.risk.figure = Figure::None;
        assert!(list(&state).is_empty());
        assert_eq!(unparsed(&state), 0);
    }

    fn three_functions() -> Figures {
        let function = |file: &str, name: &str, cyclomatic| Function {
            file: file.to_string(),
            name: name.to_string(),
            line: 1,
            metrics: Metrics {
                cyclomatic,
                ..Metrics::default()
            },
        };
        Figures {
            functions: vec![
                function("src/ui.rs", "draw", 22),
                function("src/keys.rs", "cheatsheet", 4),
                function("src/keys.rs", "route", 31),
            ],
            unparsed: 3,
        }
    }

    /// Strictly above: the threshold is the highest figure the project is
    /// content with, so a Function exactly on it is not on the worklist.
    #[test]
    fn a_function_exactly_on_the_threshold_is_not_counted() {
        let (functions, _) = functions(
            "src/keys.rs",
            &space(
                Some("src/keys.rs"),
                Kind::Unit,
                20,
                vec![space(Some("route"), Kind::Function, 20, Vec::new())],
            ),
        );
        assert_eq!(count(&functions, 20), 0);
        assert_eq!(count(&functions, 19), 1);
    }

    /// The Gate's own conditions, each one alone: a plateau, a genuine
    /// improvement that has not crossed the threshold yet, and the shred — one
    /// Function split into five trivial ones, which lowers the count and the
    /// primary total while cognitive complexity rises. That last one is the
    /// attack the third condition exists for, and it is the case a Gate on the
    /// count alone would wave through.
    #[test]
    fn the_gate_takes_a_lower_count_or_a_lower_total_and_no_other_metric_rising() {
        let one = |cyclomatic, cognitive| Figures {
            functions: vec![Function {
                file: "src/keys.rs".to_string(),
                name: "route".to_string(),
                line: 88,
                metrics: Metrics {
                    cyclomatic,
                    cognitive,
                    ..Metrics::default()
                },
            }],
            unparsed: 0,
        };
        let before = one(31, 24);
        assert_eq!(gate(&before, &before, 20), Verdict::NoImprovement);
        // The count fell.
        assert_eq!(gate(&before, &one(14, 11), 20), Verdict::Passed);
        // The count held at one and the total fell: progress, not a plateau.
        assert_eq!(gate(&before, &one(26, 20), 20), Verdict::Passed);
        // The primary figure fell and cognitive complexity rose.
        assert_eq!(
            gate(&before, &one(14, 29), 20),
            Verdict::OtherMetricWorsened
        );
        let shredded = |lines| Figures {
            functions: (0..5)
                .map(|which| Function {
                    file: "src/keys.rs".to_string(),
                    name: format!("route_{which}"),
                    line: 88 + which * 8,
                    metrics: Metrics {
                        cyclomatic: 3,
                        cognitive: 5,
                        lines,
                        ..Metrics::default()
                    },
                })
                .collect(),
            unparsed: 0,
        };
        assert_eq!(
            gate(&before, &shredded(0), 20),
            Verdict::OtherMetricWorsened
        );
        // Any other recorded metric, not only cognitive complexity: the same
        // shred with its cognitive total held under the old one is still
        // reverted, because the lines it costs went up.
        let quiet = Figures {
            functions: shredded(20)
                .functions
                .into_iter()
                .map(|function| Function {
                    metrics: Metrics {
                        cognitive: 4,
                        ..function.metrics
                    },
                    ..function
                })
                .collect(),
            unparsed: 0,
        };
        assert_eq!(gate(&before, &quiet, 20), Verdict::OtherMetricWorsened);
    }

    /// The maintainability index is the one metric that worsens downward:
    /// `main.rs` fills it from `mi_visual_studio`, where higher is easier to
    /// maintain. A pass that lowers complexity raises it, so reading it as "up
    /// is worse" would revert exactly the pass the loop asks for.
    #[test]
    fn the_maintainability_index_worsens_downward_not_upward() {
        let one = |cyclomatic, maintainability| Figures {
            functions: vec![Function {
                file: "src/keys.rs".to_string(),
                name: "route".to_string(),
                line: 88,
                metrics: Metrics {
                    cyclomatic,
                    cognitive: 24,
                    maintainability,
                    lines: 96,
                },
            }],
            unparsed: 0,
        };
        let before = one(31, 40);
        assert_eq!(gate(&before, &one(14, 60), 20), Verdict::Passed);
        assert_eq!(
            gate(&before, &one(14, 21), 20),
            Verdict::OtherMetricWorsened
        );
    }

    /// A recompute the user fires while an Iteration waits supersedes the
    /// analysis the Gate asked for, and answers under a generation of its own.
    /// The Gate must not close on it: those figures describe a tree the user may
    /// have edited, and the revert that follows would restore over their work.
    #[test]
    fn a_recompute_mid_iteration_is_not_the_iterations_measurement() {
        let mut state = State {
            root: std::path::PathBuf::from("/w"),
            risk_threshold: 20,
            test_command: Some("cargo test".to_string()),
            risk: Risk {
                figure: Figure::Current(three_functions()),
                ..Risk::default()
            },
            ..State::default()
        };
        start(&mut state, Scope::Workspace);
        pass_reported(&mut state);
        tests_finished(&mut state, true, "ok".to_string());
        // The user asks for a figure of their own; the loop's request is now
        // one generation behind.
        analyse(&mut state, Scope::Workspace);
        let generation = state.risk.asked;
        let effects = figures_arrived(&mut state, generation, Figures::default(), None);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::RestoreSnapshot { .. })),
            "the Gate closed on a measurement the Iteration never asked for: {effects:?}"
        );
        assert_eq!(state.refactor.stopped, None);
        assert_eq!(
            state.refactor.running.map(|iteration| iteration.wait),
            Some(Wait::Figures(generation - 1))
        );
    }

    /// A review-scoped loop starts while the baseline is still being measured,
    /// so the Gate can be reached with nothing to judge against: the answer the
    /// loop was counting on is superseded before it lands. The pass goes back
    /// and the loop stops naming that, rather than being judged against zero —
    /// which reads an honest pass as no improvement and reverts it under a name
    /// that is a lie. Unreachable from a scenario: it takes two analyses racing.
    #[test]
    fn a_gate_with_no_baseline_reverts_the_pass_and_says_why() {
        let mut state = State {
            root: std::path::PathBuf::from("/w"),
            view: crate::View::Review,
            risk_threshold: 20,
            max_iterations: 3,
            test_command: Some("cargo test".to_string()),
            ..State::default()
        };
        // The analysis Review view asks for on the way in, which is what lets
        // the loop start at all.
        analyse(&mut state, Scope::Review);
        start(&mut state, Scope::Review);
        assert_eq!(state.refactor.refusal, None, "a measured Scope was refused");
        pass_reported(&mut state);
        // The Gate's own analysis supersedes the baseline's, so the baseline
        // answer is dropped and never becomes one.
        tests_finished(&mut state, true, "ok".to_string());
        let generation = state.risk.asked;
        let effects = figures_arrived(&mut state, generation, Figures::default(), None);
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::RestoreSnapshot { iteration: 1 })),
            "the pass was kept without ever being judged: {effects:?}"
        );
        assert_eq!(state.refactor.stopped, Some(NO_BASELINE));
        assert_eq!(state.refactor.running, None);
    }

    /// What a review-scoped session is handed: the Scope's files by name, since
    /// it cannot read the git status CRIME read, and the Scope's own worst
    /// Functions — never a file nobody is reviewing, in either half. The prompt
    /// is the whole of what keeps the loop inside its Scope, so what it names is
    /// the Scope's boundary; a scenario can only hold that a file is absent from
    /// it, which is why the presences are pinned here.
    #[test]
    fn a_review_scoped_prompt_names_the_reviewed_files_and_their_own_worst() {
        let reviewed = |path: &str, cyclomatic| Function {
            file: path.to_string(),
            name: format!("in_{}", path.replace(['/', '.'], "_")),
            line: 1,
            metrics: Metrics {
                cyclomatic,
                ..Metrics::default()
            },
        };
        let state = State {
            risk_threshold: 20,
            view: crate::View::Review,
            repo: Some(vec![
                crate::review::GitFile {
                    path: "src/keys.rs".to_string(),
                    status: crate::review::GitStatus::Modified,
                },
                crate::review::GitFile {
                    path: "src/tree.rs".to_string(),
                    status: crate::review::GitStatus::Committed,
                },
            ]),
            risk: Risk {
                figure: Figure::Current(Figures {
                    functions: vec![reviewed("src/keys.rs", 31)],
                    unparsed: 0,
                }),
                before: Some(Figures::default()),
                ..Risk::default()
            },
            ..State::default()
        };
        let prompt = loop_prompt(&state, Scope::Review);
        assert!(prompt.contains("src/keys.rs"), "{prompt}");
        assert!(prompt.contains("in_src_keys_rs"), "{prompt}");
        assert!(
            !prompt.contains("src/tree.rs"),
            "a file nobody is reviewing was handed to the session: {prompt}"
        );
    }

    /// Every ask CRIME sends about complexity says what a good split is. The
    /// Gate's third condition already reverts a shredded pass, but it does so
    /// an Iteration late, and the row action's one-shot ask has no Gate behind
    /// it at all (R28.14) — so the wording is the only thing standing between a
    /// figure that fell and a file scattered into helpers nobody can name. A
    /// scenario can only reach one prompt at a time; all three are pinned here.
    #[test]
    fn every_refactor_ask_says_what_a_good_split_is() {
        let function = Function {
            file: "src/keys.rs".to_string(),
            name: "route".to_string(),
            line: 88,
            metrics: Metrics {
                cyclomatic: 31,
                cognitive: 24,
                ..Metrics::default()
            },
        };
        let state = State {
            risk_threshold: 20,
            view: crate::View::Review,
            repo: Some(vec![crate::review::GitFile {
                path: "src/keys.rs".to_string(),
                status: crate::review::GitStatus::Modified,
            }]),
            risk: Risk {
                figure: Figure::Current(Figures {
                    functions: vec![function.clone()],
                    unparsed: 0,
                }),
                before: Some(Figures::default()),
                ..Risk::default()
            },
            ..State::default()
        };
        for prompt in [
            loop_prompt(&state, Scope::Workspace),
            loop_prompt(&state, Scope::Review),
            refactor_prompt(&function),
        ] {
            assert!(prompt.contains("the symptom, not the goal"), "{prompt}");
            assert!(prompt.contains("structural scattering"), "{prompt}");
        }
    }

    /// A recompute is always the workspace's, so an answer landing while a
    /// review-scoped Iteration waits for its session is an answer about other
    /// files. Taking it would judge the pass against the workspace's counts —
    /// which a two-file pass beats trivially, so the false verdict is *accept*,
    /// and a Gate that accepts what it did not measure is worse than no Gate.
    #[test]
    fn an_answer_about_another_scope_is_not_the_iterations_baseline() {
        let mut state = State {
            root: std::path::PathBuf::from("/w"),
            view: crate::View::Review,
            risk_threshold: 20,
            max_iterations: 3,
            test_command: Some("cargo test".to_string()),
            ..State::default()
        };
        analyse(&mut state, Scope::Review);
        start(&mut state, Scope::Review);
        // The user's own recompute, answering first.
        analyse(&mut state, Scope::Workspace);
        let generation = state.risk.asked;
        figures_arrived(&mut state, generation, three_functions(), None);
        assert_eq!(
            state
                .refactor
                .running
                .and_then(|iteration| iteration.before),
            None,
            "the review Iteration will be judged against the workspace"
        );
    }

    /// The border's answer while a loop runs, and after it stops. Every part is
    /// load-bearing: the cap, because "Iteration 7" alone says nothing about
    /// how much is left; the caption, because the wait for the session has no
    /// timeout and a wait nobody labelled is indistinguishable from a hang; and
    /// the condition once it is over, because "it gave up" is not a diagnosis.
    #[test]
    fn the_border_names_the_iteration_the_cap_and_what_is_being_waited_for() {
        let mut state = State {
            root: std::path::PathBuf::from("/w"),
            risk_threshold: 20,
            max_iterations: 3,
            test_command: Some("cargo test".to_string()),
            risk: Risk {
                figure: Figure::Current(three_functions()),
                ..Risk::default()
            },
            ..State::default()
        };
        assert_eq!(status(&state), None, "an idle pane says nothing");
        start(&mut state, Scope::Workspace);
        assert_eq!(status(&state).as_deref(), Some("1/3 session"));
        pass_reported(&mut state);
        assert_eq!(status(&state).as_deref(), Some("1/3 tests"));
        tests_finished(&mut state, true, "ok".to_string());
        assert_eq!(status(&state).as_deref(), Some("1/3 measuring ✓"));
        stop(&mut state);
        assert_eq!(status(&state).as_deref(), Some("stopped"));
        state.refactor.stopped = Some(TESTS_FAILED);
        assert_eq!(status(&state).as_deref(), Some("tests failed"));
    }

    fn file(name: &str) -> Entry {
        Entry {
            name: name.to_string(),
            is_dir: false,
        }
    }

    /// Configuration first, then the shape, then a refusal. The middle one is
    /// ordered rather than a set: a Rust project that also holds a `Makefile`
    /// wants `cargo test`, and a folder that says nothing about tests must
    /// yield nothing at all, because a Gate reporting a pass having run
    /// nothing is worse than no Gate.
    #[test]
    fn the_test_command_is_configuration_then_the_projects_shape() {
        let rust = [file("Cargo.toml"), file("Makefile")];
        assert_eq!(
            test_command(Some("cargo nextest run"), &rust).as_deref(),
            Some("cargo nextest run")
        );
        assert_eq!(test_command(None, &rust).as_deref(), Some("cargo test"));
        assert_eq!(
            test_command(None, &[file("Makefile")]).as_deref(),
            Some("make test")
        );
        assert_eq!(test_command(None, &[file("README.md")]), None);
        // A folder of the marker's name is a folder, not a project's shape —
        // and a configured empty string says nothing, so the shape is asked.
        assert_eq!(
            test_command(
                Some("   "),
                &[Entry {
                    name: "Cargo.toml".to_string(),
                    is_dir: true,
                }]
            ),
            None
        );
    }
}
