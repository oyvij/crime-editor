//! F22 — the story artifact, and the spine it becomes.
//!
//! Reads nothing and asks git nothing: the edge hands over the artifact's
//! bytes and git's answer about the range, and this decides what they mean.
//! The schema is the validator — a derived [`serde::Deserialize`] refuses a
//! missing field, a wrong type or a `side`/`kind` the schema does not name,
//! with a message carrying the line and column. Only the exactly-three-choices
//! rule needs code of its own, because a array of one length reports as a
//! trailing-token parse error, which sends the reviewer hunting for a syntax
//! mistake that is not there.
//!
//! Deliberately lenient about fields the schema does not name: real authoring
//! runs write a `choices[].id` no scenario reads, and denying it would refuse
//! every real artifact while every scenario stayed green.

use crate::{Direction, Modal, State};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Artifact {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    /// What the whole range accomplishes at large — a feature added, a bug
    /// fixed, an improvement made — so a reviewer with zero context knows
    /// what the set is for before reading a single Story.
    pub title: String,
    pub range: Range,
    pub stories: Vec<Story>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Range {
    pub base: String,
    pub head: String,
    pub spelling: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Story {
    pub id: String,
    pub name: String,
    pub premise: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Step {
    pub id: String,
    /// Short enough to sit in the step-menu; the claim is the full sentence
    /// the band narrates.
    pub name: String,
    pub claim: String,
    pub why: String,
    pub site: Site,
    #[serde(default)]
    pub flow: Option<Flow>,
    #[serde(default)]
    pub values: Vec<Value>,
    #[serde(default)]
    pub nudge: Option<String>,
    #[serde(default)]
    pub prediction: Option<Prediction>,
}

/// What flows into and out of a Step's Site — not sourced from anywhere the
/// way a [`Value`] is, so it carries no citation of its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Flow {
    #[serde(rename = "in")]
    pub flow_in: String,
    #[serde(rename = "out")]
    pub flow_out: String,
}

/// One value a Step's claim depends on, and where it came from. `cite` is
/// present for `literal` and `fixture` provenance and absent for `invented` —
/// Varde never checks that a value is really a literal (that would need a
/// parser per language), so this is the whole of what it can promise: a place
/// to jump to, or an honest admission that there is none.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Value {
    pub name: String,
    pub value: String,
    pub provenance: Provenance,
    #[serde(default)]
    pub cite: Option<Cite>,
}

impl Value {
    /// What is actually shown: a value with nowhere to point is displayed as
    /// invented regardless of what its own `provenance` field claims. Varde
    /// never checks that a value is really a literal, so an authored
    /// `literal` with no `cite` is exactly the unsupported claim it refuses
    /// to repeat as though it were supported.
    pub fn displayed_provenance(&self) -> &'static str {
        if self.cite.is_none() {
            Provenance::Invented.as_str()
        } else {
            self.provenance.as_str()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Literal,
    Fixture,
    Invented,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::Literal => "literal",
            Provenance::Fixture => "fixture",
            Provenance::Invented => "invented",
        }
    }
}

/// Where a cited [`Value`] was copied from — `g` jumps here.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cite {
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Site {
    pub file: String,
    pub side: Side,
    pub kind: Kind,
    pub from: u32,
    pub to: u32,
    /// The lines the Site names, as they read when the Story arrived — the
    /// baseline the stale check compares against. Not authored: the AI names
    /// the file, the side, the kind and the range, and Varde reads the text
    /// off its own copy once, on arrival ([`fill`]). Defaulted rather than
    /// dropped, because the sets already in `.varde/stories/` carry a
    /// transcription and a set on disk has to keep loading.
    #[serde(default)]
    pub text: String,
}

/// Orthogonal to [`Kind`] and both required: a deletion is always `Old` +
/// `Changed`, never `New` + `Context`, because filing a removal under context
/// drops it out of coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Changed,
    Context,
}

impl Kind {
    /// The word a scenario names this by.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Changed => "changed",
            Kind::Context => "context",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Prediction {
    pub question: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Choice {
    pub text: String,
    pub correct: bool,
    pub feedback: String,
}

/// What the workspace knows about the story artifact on disk. Never a fourth
/// case folded into one of these three: a parse failure and an unresolvable
/// range are different reasons a Story is not walkable, and the spine has to
/// say which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Set {
    None,
    Loaded(Artifact),
    /// Parsed, and waiting for the edge to say what each Site's lines
    /// currently hold — the round trip [`fill`] documents. Never walkable: a
    /// Step shown here would be shown with no baseline behind its stale
    /// check, which is a Step Varde cannot yet say anything true about.
    ///
    /// `attempt` is which authoring round wrote this artifact — `Some(1)` for
    /// the first, `Some(2)` for one written in answer to a fix request, and
    /// `None` for one nobody asked for: a set read off disk when Story view
    /// opened. It rides here because the checks that decide whether a fix
    /// round is allowed do not run until the edge has answered, and by then
    /// the round the artifact belongs to is otherwise lost. `None` matters as
    /// much as the count: handing a disk read back would start a CLI the
    /// reviewer never asked for and ask it to rewrite a Story nobody was
    /// waiting on.
    Filling {
        artifact: Artifact,
        attempt: Option<u8>,
    },
    /// Refused whole, with what the parse objected to — a reviewer whose CLI
    /// wrote a broken file has to be able to find the break.
    Refused {
        because: String,
    },
    RangeGone,
    /// A bare `:story` on a clean tree: nothing in the offline
    /// default-branch ladder resolved.
    NoDefaultBranch,
    /// An explicit range git could not resolve.
    BadRange,
    /// `:story?` in a folder git does not know. Refused out loud rather than
    /// opening an empty picker: a list of no branches reads as a repository
    /// with no branches.
    NotARepository,
    /// `:story?` with work nobody has committed. Refused before the picker is
    /// drawn, because picking a row checks that branch out and Varde never
    /// checks out over an uncommitted change.
    WorkingTreeDirty,
    /// `:story? <url>` in a project workspace. A project workspace is locked
    /// to its project and a foreign repository must never appear inside it,
    /// so the Guest repo is a Bare workspace's alone.
    GuestNeedsBareWorkspace,
    /// `:story? <url>` on a machine with no `git` to run. The clone is the
    /// user's own git (ADR 0015), which makes the binary a runtime dependency
    /// for this one feature — and an absence Varde says out loud.
    NoGit,
    /// The download is running in the shell pane, where its progress and any
    /// passphrase prompt are the reviewer's to see and answer. Carries the URL
    /// because the Guest repo's directory is named from it, and the sentinel
    /// that ends this wait arrives as a path the core has to recognise; and
    /// [`Download`], because the wait is the same wait but the sentence on
    /// screen is not.
    Downloading {
        url: String,
        how: Download,
    },
    /// The download's exit status, which the sentinel carried. Never silence:
    /// a bad URL is the ordinary case and it has to read as a refusal.
    DownloadFailed {
        how: Download,
        status: Option<String>,
    },
    /// The reviewer confirmed the range and the prompt is in the AI's pane.
    /// There is no timeout (ADR 0006): a guessed one is wrong on a slow run
    /// and slow on a fast one, so this only ever leaves by an artifact
    /// arriving, the CLI exiting, or the reviewer cancelling.
    Authoring {
        spelling: String,
    },
    /// The CLI exited before the artifact arrived — told by `AiExited`, not
    /// by a stopwatch. Distinct from cancelling, which drops back to `None`
    /// outright: this one is reported because it was not the reviewer's
    /// choice.
    AuthoringAbandoned,
    /// The artifact failed [`problems`] and has been handed back: the fix
    /// request naming these Steps is in the AI's pane, and Varde is waiting
    /// again. Nothing here is walkable — a half-checked Story shown as
    /// walkable is a Step Varde already knows is wrong. Told apart from
    /// [`Set::Authoring`] on purpose: a reviewer who cannot tell a second
    /// round from a slow first one has no way to know whether waiting longer
    /// is reasonable.
    Fixing {
        /// The artifact that failed, kept only to tell the AI's answer from a
        /// re-read of the same file: leaving Story view and coming back
        /// re-reads the folder, and the failing set arriving a second time is
        /// this round still open, not a second attempt at it. Never walkable
        /// and never filled from — [`same_set`] is the whole of what reads it.
        artifact: Artifact,
        /// The round now being waited on — the same thing [`Set::Filling`]'s
        /// `attempt` means, so the word reads one way everywhere.
        attempt: u8,
        problems: Vec<Problem>,
    },
}

/// How many artifacts a confirmed range gets before its set is refused. The
/// bound is on rounds, never on a clock (ADR 0006): the failure that needs
/// catching is a dead CLI, and that arrives as `AiExited`. Two — a fix round
/// costs seconds against a re-author's minutes, and an AI that got the same
/// four line numbers wrong twice is not converging.
pub const ATTEMPTS: u8 = 2;

/// What `:story` (or `:story!`) resolved to, once the edge has walked the
/// offline ladder or checked an explicit range. Only the edge can ask git, so
/// this is told rather than derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The resolved range is already authored — load it directly, touching
    /// neither the AI pane nor a session.
    Authored,
    /// Nothing on disk describes the resolved range yet, or `force` demanded
    /// re-authoring one that already does. `out` is the exact path the AI is
    /// told to write to (ADR 0005) — only the edge can turn `spelling` into
    /// real oids, so it is handed over already computed rather than left for
    /// the CLI to derive on its own and maybe get wrong.
    ToAuthor { spelling: String, out: String },
    /// A bare `:story` on a clean tree: nothing in the offline ladder
    /// resolved.
    NoDefaultBranch,
    /// An explicit range git could not resolve.
    BadRange,
}

/// Whether an already-authored range loads or is offered up for
/// re-authoring. `force` (`:story!`) always re-authors, even over a range
/// already on disk — loading is instant and offline, and re-authoring costs
/// minutes and the AI's prompt, which is why it takes the bang.
pub fn decide(force: bool, authored: bool, spelling: String, out: String) -> Resolution {
    match (force, authored) {
        (false, true) => Resolution::Authored,
        _ => Resolution::ToAuthor { spelling, out },
    }
}

/// The authoring prompt (ADR 0006), pasted into the AI pane exactly as a
/// submitted review's is: inline text, so it is actionable by any CLI.
/// `{RANGE}` and `{OUT}` are the only substitutions Varde makes — `{OUT}` is
/// the exact path ADR 0005 names for this range, computed by the edge from
/// git's own oids rather than left for the CLI to derive (and maybe
/// abbreviate, or resolve against a different `HEAD`) on its own.
const AUTHORING_PROMPT: &str = r#"Write a guided walkthrough of the changes in `{RANGE}` as JSON (protocolVersion 2), to the file `{OUT}`.

You are writing for a developer who did not make this change and will walk it step by step, one
screen at a time, to build a mental model of it. Follow how the code runs, not how the files are
arranged: start where control enters, and go where it goes — into unchanged code when the flow
goes there. A file-by-file summary is not a walkthrough.

`{CONTEXT}` already holds everything Varde knows about this change: the two oids the range resolved
to, its diff, and every changed file's contents with line numbers down the side. Read it once rather
than running `git diff` or `git log`, and rather than reopening a changed file to count lines. It
carries the changes only — a step whose `site.kind` is `context` points deliberately at code this
change did not touch, so open those files yourself; they are not in there.

Give the whole set a `title` naming what the range accomplishes at large — a feature added, a bug
fixed, an improvement made — so a reviewer with zero context knows what it is for before reading a
single story.

Split it into one or more stories, each with a name and a one-line premise. A story's `name` must
state its concrete subject — what changed — never stand alone as a metaphor or narrative flourish;
the `premise` is where the why goes. Aim for three to six steps per story — a reader gets lost past
five, so prefer more short stories over one long one. Give each step a short `name` too, next to its
claim: the name is what sits in a menu, the claim is the full sentence the narration reads aloud.
Keep a step's `name` to about 18 characters — the menu it sits in is a fixed width, and a longer
name is truncated with an ellipsis rather than shown in full.

## The shape

A complete set, with one story and one step filled in:

```json
{
  "protocolVersion": 2,
  "title": "The wheel scrolls the tree without waiting for a redraw",
  "range": {
    "base": "1a2b3c4d5e6f7890abcdef1234567890abcdef12",
    "head": "0fedcba9876543210fedcba9876543210fedcba9",
    "spelling": "{RANGE}"
  },
  "stories": [
    {
      "id": "wheel-scroll",
      "name": "A wheel event moves the tree",
      "premise": "Where a scroll wheel becomes a new first visible row, and why the clamp is skipped.",
      "steps": [
        {
          "id": "wheel-scroll.1",
          "name": "Wheel sets scroll",
          "claim": "A wheel event moves `tree_scroll` by three rows instead of moving the selection.",
          "why": "Scrolling and selecting are different gestures; moving the selection under the wheel would drag the editor along with it.",
          "site": {
            "file": "src/lib.rs",
            "side": "new",
            "kind": "changed",
            "from": 412,
            "to": 415
          },
          "flow": {
            "in": "an `Event::Wheel` carrying a direction and the pane the pointer is over",
            "out": "a new `State` whose `tree_scroll` has moved, and no effects"
          },
          "values": [
            {
              "name": "rows per wheel notch",
              "value": "3",
              "provenance": "literal",
              "cite": { "file": "src/lib.rs", "line": 44 }
            },
            {
              "name": "the tree's first visible row while scrolling",
              "value": "17",
              "provenance": "invented"
            }
          ],
          "nudge": "The arm returns early, so the clamp every other event falls through to is skipped here on purpose.",
          "prediction": {
            "question": "Why does this arm return before the clamp the other arms fall through to?",
            "choices": [
              {
                "text": "The clamp would pull the scroll back to the selection, undoing the wheel.",
                "correct": true,
                "feedback": "Right: the clamp keeps the selection visible, which is the opposite of what a wheel asked for."
              },
              {
                "text": "The clamp is slow, and a wheel arrives hundreds of times a second.",
                "correct": false,
                "feedback": "The clamp is arithmetic on two integers; a wheel batch costs its time in drawing, not here."
              },
              {
                "text": "The renderer clamps `tree_scroll` already, so a second clamp is redundant.",
                "correct": false,
                "feedback": "The renderer reads `tree_scroll` and never writes it — all clamping lives in `update`."
              }
            ]
          }
        }
      ]
    }
  ]
}
```

## Rules, field by field

**`title`** names what the whole range accomplishes — a feature added, a bug fixed, an improvement
made — never a list of the stories under it. **`name`** on a story and on a step states its concrete
subject; a step's is about 18 characters, because it sits in a fixed-width menu.

**`range`** carries the two full oids the range resolved to and the `spelling` exactly as given
above. Do not abbreviate an oid and do not resolve the range yourself against a different `HEAD`.

**`id`** is a short slug on a story and `"<story-id>.<n>"` on a step. **`premise`** is one line: what
the story is about, so a reader can choose it from a list.

**`claim`** is one sentence — what this code does. **`why`** is why it exists, or why it is done this
way and not the obvious other way.

**`site`** is where the step points.
- `file` is a path relative to the repository root.
- `side` is `"new"` for a line as it is after the change, `"old"` for a line the change removed.
- `kind` is `"changed"` when the change touched those lines, `"context"` when it did not — a step
  that walks into untouched code to follow the flow is expected and useful.
- **A deletion is `side: "old"` with `kind: "changed"`, always.** When the change only *removed*
  lines, point at them on the old side, where they can still be read, and give the line numbers they
  had before the change. Never file a removal as `side: "new"` with `kind: "context"`: that says
  "unchanged code", and removed code is not unchanged code — it is the part of the change nobody
  will see if it is filed under context.
- `from`/`to` are line numbers at the revision `side` names, not at any other revision.
- Do not write the lines themselves. A site is the file, the side, the kind and the range, and
  nothing else — Varde reads what those lines hold off its own copy of the file.
- A site may point at prose — a design document that argues the change is a legitimate step, and
  often the most valuable one.

**`flow`** is what arrives at the site and what leaves it. Omit `flow` entirely when the step is
about a decision or an argument rather than data passing through. Do not invent an in and an out to
fill the field.

**`values`** are concrete values that make the flow real — an argument, a constant, a config value.
Omit `values` when the claim does not depend on one; most steps have none, and an unused value is
noise on a fixed-width strip. Each one's `provenance` is:
- `"literal"` — copied from a line of code. **Requires `cite`.**
- `"fixture"` — copied from a test or fixture in this repository. **Requires `cite`.**
- `"invented"` — an illustrative value you made up. **No `cite`.** Use this honestly: an assembled
  or representative value is invented even if its parts are real.

A `cite` names a `file` and a `line` that really contains that value. The reader can press one key to
jump there, and a citation that lands on the wrong code is worse than no value at all. If you are not
certain, mark it `"invented"` and drop the `cite`.

**`nudge`** is one more sentence for a reader who wants more detail than the claim. Omit `nudge` when
there is nothing further to say — a nudge restating the claim wastes the one line it gets.

**`prediction`** is optional and rare — at most one or two per story, at a step where the reasoning is
genuinely worth stopping for. Omit it everywhere else; a walkthrough is not a quiz.

When present it must offer **exactly three choices** — a set with a prediction of any other size is
refused whole, and nothing in it is walkable. Exactly one choice is `correct`. Ask **why**, never
"which line runs next" — guessing the next line measures nothing. Wrong choices must be genuinely
plausible reasons a competent developer might give, and their `feedback` must explain why they are
wrong; that explanation is the most valuable text in the file. Never invent a function, file or
behaviour that does not exist in order to make a wrong choice: draw wrong choices from real
alternatives.

**Coverage.** Every changed hunk should be claimed by some step. Do not add a list of what you left
out and do not guess at line numbers you have not read — what no story reaches is worked out from git
and reported separately. Leaving something out is allowed; misreporting it is not.

## Before you write the file

Check your own work with a script rather than by eye: open every `cite` and confirm the line contains
that value, confirm every prediction has exactly one `correct: true` and names nothing that does not
exist, confirm the JSON parses, and confirm the steps follow execution order rather than file order.

Write only the JSON file, at exactly the path above. Do not summarise it back; the file is the
whole output."#;

/// The authoring prompt for a confirmed range, with `{RANGE}`, `{OUT}` and
/// `{CONTEXT}` substituted.
pub fn prompt(spelling: &str, out: &str, context: &str) -> String {
    AUTHORING_PROMPT
        .replace("{RANGE}", spelling)
        .replace("{OUT}", out)
        .replace("{CONTEXT}", context)
}

/// The companion file's path, beside the Story set it was written for and
/// named for the same range, so re-authoring replaces it rather than leaving a
/// stale hand-over to be read against a fresh Story, and so
/// [`prune`]'s retention rule reaches it without a second rule of its own.
/// Suffix-based, so it answers for a bare filename as well as a path.
pub fn context_path(artifact: &str) -> String {
    let stem = artifact.strip_suffix(".json").unwrap_or(artifact);
    format!("{stem}.context.md")
}

/// How much of the change the companion file is allowed to carry. A tuning
/// knob, not a behaviour: what it must never do is stop short in silence, which
/// is what [`TRUNCATION`] is for.
pub const CONTEXT_CAP: usize = 256 * 1024;

/// What the companion file says when the edge could not get an answer out of
/// git for the range at all. Written rather than skipped: the prompt has
/// already pointed the AI at this path, and a missing file is a failure the AI
/// reads as silence — it would fall back to `git diff` without ever saying it
/// had to. The wording is the library's, so the two things the file can say
/// live next to each other.
pub const CONTEXT_UNAVAILABLE: &str =
    "# The change\n\nVarde could not read this range out of git, so nothing is handed over here.\n\
     Work the change out yourself, from the range the prompt names.\n";

const TRUNCATION: &str =
    "\n--- Truncated here. Read anything further you need from the files directly.\n";

/// The companion file's contents: the resolved oids, the range's
/// diff, and every changed file's lines with their numbers down the side —
/// everything the AI would otherwise spend round trips rediscovering about a
/// range the reviewer has already confirmed.
///
/// Pure, so the format the prompt describes and the format Varde writes cannot
/// drift apart without a test saying so. The edge gathers `files` from git and
/// writes the result; it formats nothing.
///
/// The changes only, and it says so: a `context` Site points deliberately at
/// code the range did not touch, and a companion file that looked like the
/// whole world would confine the Story to changed lines.
///
/// The hunks decide which files are in it — a delta with none of them is a
/// binary file or a mode change, which has nothing to hand over — and `git2`
/// renders the bodies, for the same reason [`hunks`] asks it for the
/// boundaries.
pub fn context_file(
    base: &str,
    head: &str,
    spelling: &str,
    files: &[FileHunks],
    cap: usize,
) -> String {
    let changed = || files.iter().filter(|file| !file.hunks.is_empty());
    let mut out = format!(
        "# The change in `{spelling}`\n\n\
         - base `{base}`\n\
         - head `{head}`\n\n\
         This is the changes only: the diff of the range, then each changed file's lines with their\n\
         numbers. Code the range did not touch is not here — open those files directly.\n\n\
         ## The diff\n\n```diff\n"
    );
    for file in changed() {
        out.push_str(&diff_text(&file.old_text, &file.new_text, &file.file));
    }
    out.push_str("```\n");
    // Every diff before any listing, so a file whose contents alone fill the
    // cap cannot starve a later file of the diff too. The listing is the
    // convenience — it is what saves reopening a file to count lines — and the
    // diff is the hand-over itself.
    for file in changed() {
        // A file the range deleted has no new side to number, and its lines are
        // still readable on the old one — the same reason a deletion's Site is
        // authored there.
        let (side, text) = if file.new_exists {
            ("new", &file.new_text)
        } else {
            ("old", &file.old_text)
        };
        out.push_str(&format!("\n## `{}`, {} side\n\n```\n", file.file, side));
        for (index, line) in text.lines().enumerate() {
            out.push_str(&format!("{:>5} | {}\n", index + 1, line));
        }
        out.push_str("```\n");
    }
    truncated(out, cap)
}

/// The unified diff of one file's two sides, at Varde's pinned context width —
/// `git2` renders it, for the same reason [`hunks`] asks `git2` for the hunk
/// boundaries: a hand-rolled diff is a second answer to a solved question.
fn diff_text(old: &str, new: &str, file: &str) -> String {
    let mut options = git2::DiffOptions::new();
    options.context_lines(CONTEXT_LINES);
    let named = Path::new(file);
    let Ok(mut patch) = git2::Patch::from_buffers(
        old.as_bytes(),
        Some(named),
        new.as_bytes(),
        Some(named),
        Some(&mut options),
    ) else {
        return String::new();
    };
    patch
        .to_buf()
        .ok()
        .and_then(|buffer| buffer.as_str().ok().map(str::to_string))
        .unwrap_or_default()
}

/// At a line boundary, and never in silence. Cut inside a line and the last
/// numbered line reads as a shorter line than it is.
fn truncated(text: String, cap: usize) -> String {
    if text.len() <= cap {
        return text;
    }
    // A line boundary by preference, a character boundary at worst: one
    // minified file with no newline inside the cap would otherwise cut at zero
    // and hand over nothing but the notice.
    let cut = text
        .bytes()
        .take(cap)
        .rposition(|byte| byte == b'\n')
        .map_or_else(
            || {
                (0..=cap)
                    .rev()
                    .find(|at| text.is_char_boundary(*at))
                    .unwrap_or(0)
            },
            |at| at + 1,
        );
    format!("{}{TRUNCATION}", &text[..cut])
}

/// The ten most recent story sets survive a new one arriving (ADR 0005): a
/// story set is 30–70KB against a review's 1KB, and none of them are meant
/// to be read again. `existing` is oldest-first — whatever ordering the
/// caller already has, since only the edge can list a directory and knows
/// which file is oldest.
pub const RETENTION: usize = 10;

pub fn prune(existing: &[String], limit: usize) -> Vec<String> {
    let excess = existing.len().saturating_sub(limit);
    existing[excess..].to_vec()
}

/// The offline default-branch ladder, walked in the fixed order settled
/// while charting: the remote's own answer, the current branch's configured
/// upstream, the layered `init.defaultBranch` config, then a probe for
/// `main` and then `master`. Git has no offline way to ask a remote what its
/// default branch actually is, so this walks facts the repository already
/// states about itself. The first three candidates arrive pre-resolved;
/// only the two probes are cheap enough to ask for by name.
pub fn default_branch(
    origin_head: Option<&str>,
    upstream: Option<&str>,
    configured: Option<&str>,
    exists: impl Fn(&str) -> bool,
) -> Option<String> {
    origin_head
        .or(upstream)
        .or(configured)
        .map(str::to_string)
        .or_else(|| exists("main").then(|| "main".to_string()))
        .or_else(|| exists("master").then(|| "master".to_string()))
}

/// One ref the edge read off the repository, for the branch picker. The commit
/// date rides along because ordering is this module's decision and only git can
/// answer it — the same split every other fact about a repository follows here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRef {
    /// As git names it: `feature` for a local branch, `origin/feature` for a
    /// remote-tracking one.
    pub name: String,
    pub remote: bool,
    /// Seconds, from the commit the ref points at.
    pub when: i64,
}

/// What `:story?` found. Whether the folder is a repository at all, and whether
/// its working tree is clean, are edge facts, so this is told rather than
/// derived. A dirty tree is refused up front because picking a branch checks it
/// out, and Varde never checks out over work nobody has saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Branching {
    Listed(Vec<BranchRef>),
    NotARepository,
    Dirty,
    /// The download's own exit status, as the sentinel recorded it (ADR 0015)
    /// — a clone or a fetch that failed is refused out loud, with the status,
    /// rather than leaving the picker waiting on a repository that never
    /// arrived. `None` for a sentinel that could not be read at all: a failure
    /// with no status to name is still a failure, and saying "git exited
    /// unknown" would put a word where a number belongs. `how` is the core's
    /// own, handed to the edge with the read and handed back with the answer.
    DownloadFailed {
        how: Download,
        status: Option<String>,
    },
}

/// Whether the working tree holds work a checkout would tread on. Varde's own
/// directory is not the reviewer's work: opening a project workspace writes
/// `state.json` and `risk.json` into it, and Varde never edits the project's
/// `.gitignore` (`configuration.feature`) — so counting those refused the
/// picker in every repository that had not ignored them, which is every
/// repository the first time. Ignored files are already absent from what git is
/// asked (`git_status`), so what is left here is the one exception that is
/// Varde's own making.
pub fn uncommitted(files: &[crate::review::GitFile]) -> bool {
    files
        .iter()
        .any(|file| !Path::new(&file.path).starts_with(crate::VARDE_DIR))
}

/// The branch list to show, from the refs the edge read: local and
/// remote-tracking deduped by short name, most recent commit first, narrowed
/// to what was typed in the picker. Deduped because a branch that exists
/// locally and on a remote is one branch — a list of refs is not the list a
/// reviewer came for — and ordered by date because the branch under review is
/// the one just pushed, not the one abandoned in 2019.
///
/// The narrowing is the tree filter's rule, not a second one: a literal match
/// hides everything that merely spells the letters in order, and fuzzy stands
/// alone only when nothing is literal. What it does *not* borrow is the
/// ranking — the order here is the commit date's, and a filter that reordered
/// the list would answer a question nobody asked.
pub fn branches(refs: &[BranchRef], needle: &str) -> Vec<String> {
    let mut rows: Vec<&BranchRef> = refs.iter().filter(|row| short(row) != "HEAD").collect();
    // Local before remote on a tie so the row a checkout resolves without
    // creating anything comes first, and by name after that: a branch and the
    // remote it tracks sit on the same commit, so the order the edge read them
    // in cannot be what decides the list.
    rows.sort_by(|a, b| {
        b.when
            .cmp(&a.when)
            .then(a.remote.cmp(&b.remote))
            .then_with(|| short(a).cmp(short(b)))
    });
    let mut seen = BTreeSet::new();
    let names: Vec<String> = rows
        .into_iter()
        .filter(|row| seen.insert(short(row)))
        .map(|row| short(row).to_string())
        .collect();
    if needle.is_empty() {
        return names;
    }
    let typed = needle.to_lowercase();
    let literal: Vec<String> = names
        .iter()
        .filter(|name| name.to_lowercase().contains(&typed))
        .cloned()
        .collect();
    match literal.is_empty() {
        false => literal,
        true => names
            .into_iter()
            .filter(|name| crate::filter::score(needle, name).is_some())
            .collect(),
    }
}

/// Where the clone reports its exit status, inside the Sidecar the Guest repo
/// is cloned into. A file rather than the pane's output, for the reason
/// [`crate::risk::SENTINEL`] is one: Varde may not read what a hosted pane
/// prints, so completion is made a filesystem fact the watcher already sees.
pub const DOWNLOAD_SENTINEL: &str = "download-done";

/// Which git the shell pane is asked to run for a Guest repo. A repository
/// already downloaded this session is fetched rather than cloned again:
/// reviewing two branches of one repository downloads it once, and a clone
/// into a directory that already exists only fails. The wait, the sentinel
/// and the refusal are the same either way — this is what the reviewer is
/// told is happening, and which line the shell pane runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Download {
    Clone,
    Fetch,
}

/// The Guest repo's directory name: the URL's last segment without `.git`, so
/// `git@github.com:them/theirs.git` clones to `theirs`. Flat inside the
/// Sidecar — a Bare workspace reviews one Guest repo at a time, and a name
/// nobody can eyeball is what the Sidecar's own name already costs.
///
/// A URL is untrusted input, and the name is a path component: anything that
/// could climb out of the Sidecar — a segment that is empty, or one starting
/// with a dot — is refused a name of its own rather than sanitised into a
/// plausible one.
pub fn guest_name(url: &str) -> String {
    let last = url
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default();
    let name = last.strip_suffix(".git").unwrap_or(last);
    match name.starts_with('.') || name.is_empty() {
        true => "guest".to_string(),
        false => name.to_string(),
    }
}

/// The clone or the fetch, as one line for the shell pane to run (ADR 0015). The sentinel
/// is removed first and written last: `echo $?` after a `;` records the status
/// whatever it was, where `&& touch` writes nothing on a failure and leaves
/// Varde waiting forever, and a sentinel left over from an earlier clone is a
/// completion the watcher reports before this one has started.
///
/// Written to a temporary name and moved into place, because this sentinel is
/// *read* where the Refactor loop's is only counted: a redirection creates the
/// file before the shell writes a byte into it, and the watcher reports that
/// empty file as the clone having finished with no status at all — a clone that
/// worked, refused. There is no second chance, since the contents arriving
/// afterwards are a modification and the wait is already over. A rename makes
/// the file's appearance and its contents the same event.
///
/// Both paths and the URL are quoted by `shlex`: a URL is untrusted input,
/// and a folder path can hold anything a filesystem allows.
pub fn download_command(how: Download, url: &str, into: &Path, sentinel: &Path) -> String {
    let quoted = |text: &str| {
        shlex::try_quote(text)
            .expect("no NUL in a URL or a path")
            .into_owned()
    };
    let into = quoted(&into.to_string_lossy());
    let writing = quoted(&sentinel.with_extension("writing").to_string_lossy());
    let sentinel = quoted(&sentinel.to_string_lossy());
    // The fetch names no URL: `git fetch <url>` writes FETCH_HEAD and no
    // remote-tracking ref, so the picker would list the branches the clone saw
    // and nothing since. `origin` is the URL, since the clone set it. No
    // `--prune`: a ref for a branch deleted on the remote is still a ref this
    // Guest repo can check out and review, so pruning is a behaviour nothing
    // asked for and no scenario covers.
    let git = match how {
        Download::Clone => format!("git clone {} {into}", quoted(url)),
        Download::Fetch => format!("git -C {into} fetch"),
    };
    format!("rm -f {sentinel}; {git}; echo $? > {writing}; mv {writing} {sentinel}")
}

/// A ref's branch name: a remote-tracking ref is named for its remote, and the
/// remote is not part of the branch.
fn short(row: &BranchRef) -> &str {
    match row.remote {
        true => row
            .name
            .split_once('/')
            .map_or(&*row.name, |(_, rest)| rest),
        false => &row.name,
    }
}

/// What Story view's tree pane is listing. The changed-files list is joined to
/// the spine, never replaced, so toggling with `t` touches neither list's
/// selection nor its scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    Spine,
    Files,
}

/// Whether git can still resolve the revisions a story set was written
/// against. Only the edge can ask git, so this is told rather than derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeStatus {
    Resolves,
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineRow {
    pub name: String,
    pub steps: usize,
    pub stale: usize,
}

/// What a Step's Site turned out to be, next to the text it was authored
/// against. Three distinct kinds because they want different words — the
/// file is gone, the file is now too short to hold the range at all, or the
/// range's own lines say something else — and only the last of those has
/// anything to show for "what the Site holds now": the other two describe an
/// absence, not a substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Staleness {
    Fresh,
    Gone,
    TooShort,
    Changed { now: String },
}

impl Staleness {
    pub fn is_stale(&self) -> bool {
        !matches!(self, Staleness::Fresh)
    }

    /// The word a scenario (and the overlay) names this by.
    pub fn kind(&self) -> Option<&'static str> {
        match self {
            Staleness::Fresh => None,
            Staleness::Gone => Some("file-missing"),
            Staleness::TooShort => Some("range-out-of-bounds"),
            Staleness::Changed { .. } => Some("text-changed"),
        }
    }
}

/// Collapses whitespace, but does not erase it: a reindent shifts how much
/// space separates two tokens, which is exactly what this must ignore, but
/// removing a space that separated two words is a real content change and
/// must still count as one. Comparing on the words each line holds, not on
/// every character in it, draws that line — `"a b"` and `"ab"` still differ,
/// while `"    a"` and `"a"` do not.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The lines `from..=to` (1-based, inclusive) out of a file's full text, or
/// `None` when the file no longer reaches that far — the "too short" case,
/// kept distinct from a line that is merely different.
///
/// Private: the edge hands over whole files and every question about which
/// lines a range names is answered here, because arithmetic in `main.rs` is
/// arithmetic without a test.
fn lines_in(text: &str, from: u32, to: u32) -> Option<String> {
    if from == 0 || from > to {
        return None;
    }
    // A file on disk ends with a trailing newline as a matter of convention,
    // not as an extra blank line past the last one — counting it as a line
    // would make every site one line short of "too short".
    let lines: Vec<&str> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect();
    if to as usize > lines.len() {
        return None;
    }
    Some(lines[(from as usize - 1)..(to as usize)].join("\n"))
}

fn staleness_against(site: &Site, exists: bool, current_text: &str) -> Staleness {
    if !exists {
        return Staleness::Gone;
    }
    match lines_in(current_text, site.from, site.to) {
        None => Staleness::TooShort,
        Some(now) if normalize_whitespace(&now) == normalize_whitespace(&site.text) => {
            Staleness::Fresh
        }
        Some(now) => Staleness::Changed { now },
    }
}

/// Whether a Step's Site still holds the text it was authored against. An
/// old-side Site is immune: it names a base that does not move, unless the
/// range itself was uncommitted (`head` is `worktree`) — committing then
/// moves what `HEAD` holds, which is exactly the same "old" text this reads,
/// so it is not a special case here so much as a consequence of reading it
/// live rather than freezing it at authoring time.
///
/// Never the file watcher: it only covers folders the tree has expanded to,
/// and a Site can sit anywhere in the project. `state.file_hunks` is polled
/// on its own cadence regardless of the tree, which is what the Remainder
/// already relies on — this rides the same poll rather than a second one. A
/// buffer the reviewer is actively editing overrides it for the new side,
/// since an unsaved edit is the freshest thing Varde knows about a file and
/// the poll cannot see it until it is written.
///
/// The new side is the working tree even over a committed range, which the
/// hunk inventory behind it deliberately is not ([`Inventory`]). The two
/// answer different questions: a hunk is part of the range, and a Site's
/// staleness is about the code the reviewer is looking at — `OpenAt` reads the
/// working tree, so comparing a Site against the range's own head would report
/// every Step fresh while the screen showed something else.
pub fn staleness(state: &State, step: &Step) -> Staleness {
    let hunks = state
        .file_hunks
        .iter()
        .find(|file| file.file == step.site.file);
    match step.site.side {
        Side::Old => match hunks {
            None => Staleness::Fresh,
            Some(file) => staleness_against(&step.site, file.old_exists, &file.old_text),
        },
        Side::New => {
            let path = state.root.join(&step.site.file);
            if let Some(buffer) = state.buffers.get(&path).filter(|buffer| buffer.is_dirty()) {
                return staleness_against(&step.site, true, buffer.shown());
            }
            match hunks {
                None => Staleness::Fresh,
                Some(file) => staleness_against(&step.site, file.new_exists, &file.new_text),
            }
        }
    }
}

/// Parses and validates a story artifact in one pass: `serde` refuses a
/// structural defect, and the one rule it cannot express — a prediction must
/// offer exactly three choices — is checked here, whole-artifact, before any
/// of it is trusted.
pub fn parse(text: &str) -> Result<Artifact, String> {
    let artifact: Artifact = serde_json::from_str(text).map_err(|error| error.to_string())?;
    for story in &artifact.stories {
        for step in &story.steps {
            if let Some(prediction) = &step.prediction {
                if prediction.choices.len() != 3 {
                    return Err(format!(
                        "a prediction must offer exactly three choices, not {}",
                        prediction.choices.len()
                    ));
                }
            }
        }
    }
    Ok(artifact)
}

/// Every file the arriving artifact names — each Site's, and each cited
/// value's — deduped, in Step order. The whole of what the edge must diff and
/// read before an artifact can be checked or filled: a `context` Site's file
/// carries no delta at all and a citation can point anywhere, so neither is in
/// the range's own diff.
pub fn story_files(artifact: &Artifact) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for step in artifact.stories.iter().flat_map(|story| &story.steps) {
        let cited = step
            .values
            .iter()
            .filter_map(|value| value.cite.as_ref())
            .map(|cite| &cite.file);
        for file in std::iter::once(&step.site.file).chain(cited) {
            if !files.contains(file) {
                files.push(file.clone());
            }
        }
    }
    files
}

/// One side of one file as the reviewer sees it — the range's base, or the
/// working tree — or `None` when that side does not hold the file at all. A
/// file with no entry was never asked for, which only happens for a file the
/// artifact does not name, so reading it as absent answers the only question
/// anyone asks about it.
fn side_text<'a>(files: &'a [FileHunks], file: &str, side: Side) -> Option<&'a str> {
    let entry = files.iter().find(|entry| entry.file == file)?;
    let (exists, text) = match side {
        Side::Old => (entry.old_exists, &entry.old_text),
        Side::New => (entry.new_exists, &entry.new_text),
    };
    exists.then_some(text.as_str())
}

/// The same file at the range's own two ends, which is what [`problems`]
/// judges against — see [`FileHunks::head_text`] for why that is not
/// [`side_text`].
fn range_text<'a>(files: &'a [FileHunks], file: &str, side: Side) -> Option<&'a str> {
    let entry = files.iter().find(|entry| entry.file == file)?;
    let (exists, text) = match side {
        Side::Old => (entry.old_exists, &entry.old_text),
        Side::New => (entry.head_exists, &entry.head_text),
    };
    exists.then_some(text.as_str())
}

/// Fills in the text the AI no longer writes, out of the same file texts the
/// stale check reads — the two must agree, or a Step arrives already stale
/// against a baseline nothing else uses. Only where the artifact carried
/// nothing: a set that transcribed its own text keeps it, for the reason
/// [`same_set`] gives.
pub fn fill(artifact: &mut Artifact, files: &[FileHunks]) {
    for step in artifact
        .stories
        .iter_mut()
        .flat_map(|story| story.steps.iter_mut())
    {
        if !step.site.text.is_empty() {
            continue;
        }
        step.site.text = side_text(files, &step.site.file, step.site.side)
            .and_then(|text| lines_in(text, step.site.from, step.site.to))
            .unwrap_or_default();
    }
}

/// What is wrong with one Step, in the four ways Varde can tell without
/// judgement. Never "the code does not do what the claim says" and never "the
/// Steps are out of execution order" — those are judgements and they stay with
/// the AI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The file the Site names is not there on the side it names.
    FileMissing,
    /// The file is there, but does not reach the Site's last line.
    RangeOutOfBounds,
    /// A `changed` Site that overlaps no hunk of the range — a Step claiming a
    /// change that is not where it says it is.
    ClaimsNoChange,
    /// A cited value whose characters are not on the line it cites. The
    /// weakest honest check: whether the value is really a *literal* would
    /// need a parser per language, and ADR 0006 declines that for good.
    CitationAbsent,
}

impl Fault {
    /// The word a scenario names this by, and the word the fix request names
    /// it by — one spelling, so what a reviewer reads in a refusal and what
    /// the AI is asked to fix cannot drift apart.
    fn as_str(self) -> &'static str {
        match self {
            Fault::FileMissing => "file-missing",
            Fault::RangeOutOfBounds => "range-out-of-bounds",
            Fault::ClaimsNoChange => "claims-no-change",
            Fault::CitationAbsent => "citation-absent",
        }
    }

    /// What the AI is told is wrong, and what would make it right. The
    /// reviewer's notice says only the word above: they can open the file,
    /// and the AI is the one that has to act.
    fn complaint(self) -> &'static str {
        match self {
            Fault::FileMissing => {
                "there is no file at the path this step's `site.file` names. Paths are relative to \
                 the repository root."
            }
            Fault::RangeOutOfBounds => {
                "this step's `site.from`/`site.to` run past the end of that file at the side it \
                 names. Line numbers are 1-based, at the revision `side` names."
            }
            Fault::ClaimsNoChange => {
                "this step says `kind: \"changed\"` but those lines are not in this range's diff. \
                 Point at lines the change touched, or say `kind: \"context\"` if the step \
                 deliberately walks into untouched code. A removal is `side: \"old\"` with \
                 `kind: \"changed\"`."
            }
            Fault::CitationAbsent => {
                "a value on this step cites a line that does not contain it. Fix the line number, \
                 or mark the value `\"invented\"` and drop its `cite`."
            }
        }
    }
}

/// One Step and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub step: String,
    pub fault: Fault,
}

/// The checks that need no judgement, run once on arrival against the same
/// file texts and hunks the Remainder and the stale check read. Milliseconds
/// and no AI time: a Step pointing at the wrong code is caught here rather
/// than walked with exactly the confidence of a right one.
///
/// The overlap question is [`claims`]'s, the same predicate the Remainder
/// counts coverage with — one question about one set of Sites and hunks, asked
/// in one place, so the check and the count cannot disagree.
pub fn problems(artifact: &Artifact, files: &[FileHunks]) -> Vec<Problem> {
    let mut found = Vec::new();
    for step in artifact.stories.iter().flat_map(|story| &story.steps) {
        let site = &step.site;
        let mut fault = |fault| {
            found.push(Problem {
                step: step.id.clone(),
                fault,
            })
        };
        let placed = match range_text(files, &site.file, site.side) {
            None => {
                fault(Fault::FileMissing);
                false
            }
            Some(text) if lines_in(text, site.from, site.to).is_none() => {
                fault(Fault::RangeOutOfBounds);
                false
            }
            Some(_) => true,
        };
        // Only asked of a Site that is somewhere: a Step pointing at nothing
        // trivially overlaps nothing too, and reporting both says the same
        // thing twice while burying the fault that explains the other.
        if placed
            && matches!(site.kind, Kind::Changed)
            && !files.iter().any(|entry| {
                entry
                    .hunks
                    .iter()
                    .any(|hunk| claims(site, &entry.file, hunk))
            })
        {
            fault(Fault::ClaimsNoChange);
        }
        for value in &step.values {
            let Some(cite) = &value.cite else {
                continue;
            };
            // The range's new side, the same one a Site is judged at: a
            // `Cite` names no side of its own, and reading it off the working
            // tree would call a citation fabricated because the reviewer
            // edited the line after the Story was written.
            let present = range_text(files, &cite.file, Side::New)
                .and_then(|cited| lines_in(cited, cite.line, cite.line))
                .is_some_and(|line| line.contains(&value.value));
            if !present {
                fault(Fault::CitationAbsent);
            }
        }
    }
    found
}

/// What a set refused by [`problems`] says it was refused for — every failing
/// Step and its fault, in the order the checks found them. Same shape as a
/// parse refusal, and the same [`Set::Refused`] behind it: a reviewer whose
/// CLI wrote a Story pointing at nothing has to be able to find which Step.
pub fn refusal(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|problem| format!("{}: {}", problem.step, problem.fault.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The fix request handed back to the AI when an artifact fails [`problems`]
/// (ticket 06). Ten to twenty minutes of authoring is not thrown away over
/// four bad line numbers: the failing Steps are named, the rest of the set is
/// declared untouchable, and the same path is asked for again — a rewrite of a
/// few Steps is seconds, and re-authoring is minutes.
///
/// `{OUT}` is worked out from the range the artifact records rather than
/// remembered: a set is named for its revisions (ADR 0005), and by the time
/// the checks have run, the `out` the reviewer confirmed is two events behind.
const FIX_PROMPT: &str = r#"Varde checked the story set you wrote to `{OUT}` and cannot accept it yet. These steps do not hold up:

{PROBLEMS}

Rewrite only those steps. Keep the set's `title`, its `range`, its stories and every other step
exactly as they are, and write the whole set back to `{OUT}` — the same path, replacing what is
there. Do not re-author the walkthrough and do not renumber or drop the steps that were fine.

`{CONTEXT}` still holds the range's two oids, its diff and every changed file's contents with line
numbers; read the line numbers off it rather than guessing again.

Write only the JSON file. Do not summarise it back."#;

/// See [`FIX_PROMPT`]. Names every failing Step and no passing one: the AI
/// rewrites four Steps rather than the whole set.
pub fn fix_prompt(dir: &Path, artifact: &Artifact, problems: &[Problem]) -> String {
    let out = artifact_path_of(dir, artifact);
    let listed = problems
        .iter()
        .map(|problem| {
            format!(
                "- `{}`: {} — {}",
                problem.step,
                problem.fault.as_str(),
                problem.fault.complaint()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    FIX_PROMPT
        .replace("{PROBLEMS}", &listed)
        .replace("{CONTEXT}", &context_path(&out))
        .replace("{OUT}", &out)
}

/// Where the artifact Varde just read was written, worked out from the range
/// it records rather than remembered. A set is named for its revisions (ADR
/// 0005), so the name is a function of the artifact — which is what lets a fix
/// request name the same path without the confirmed `out` being carried
/// through the two events between confirming and checking. Truncating by
/// characters rather than slicing: the prompt asks for full oids, and an
/// artifact that abbreviated one anyway must not panic here.
fn artifact_path_of(dir: &Path, artifact: &Artifact) -> String {
    let short = |oid: &str| oid.chars().take(12).collect::<String>();
    let head = if artifact.range.head == WORKTREE {
        WORKTREE.to_string()
    } else {
        short(&artifact.range.head)
    };
    artifact_path(dir, &short(&artifact.range.base), &head)
}

/// Whether an arriving artifact is the set already loaded, ignoring the text
/// Varde filled in — how a re-read of what is on screen is told from a
/// newly-authored set. Story view re-reads its folder every time it is opened,
/// and a set the AI no longer transcribes arrives from disk with its text
/// missing every time: refilling on the re-read would take the baseline from
/// the file as it reads now, compare the file against itself, and report every
/// Step fresh for good.
///
/// Blanks the text and compares the whole artifact rather than walking the
/// fields it cares about: a hand-written comparison is one more list to keep
/// in step with the schema, and this one cannot drift. It costs two clones of
/// a 30–70KB set per Story-view open, which is the cheaper of the two.
pub fn same_set(loaded: &Artifact, arrived: &Artifact) -> bool {
    let without_text = |artifact: &Artifact| {
        let mut copy = artifact.clone();
        for story in &mut copy.stories {
            for step in &mut story.steps {
                step.site.text = String::new();
            }
        }
        copy
    };
    without_text(loaded) == without_text(arrived)
}

/// The reviewer's position while walking the spine: a Story and one of its
/// Steps, or the Remainder and one of its bare locations. Two variants
/// rather than an `Option<usize>` step tacked onto a Remainder case, because
/// [`current_step`] must refuse a claim for the Remainder outright — it is
/// not a Story with an absent Step, it is not a Story at all. `step` and
/// `index` are always in bounds for what they index — [`advance`] clamps
/// rather than letting either run off either end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walking {
    Story {
        story: usize,
        step: usize,
        diff: Diff,
    },
    Remainder {
        index: usize,
    },
}

/// Whether `d` has laid the Range's diff over the Site. Part of the position
/// rather than a field of its own on `State`, so it cannot be shown with no
/// Story being walked, survives `n`/`p`, and is gone with the walk it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diff {
    Hidden,
    Shown,
}

/// The narration band's fixed height in Story view (ADR "the narration
/// band's rectangle"): one separator, four content rows, one row of key
/// hints. A values row takes a content row rather than adding one, so the
/// code never moves under a keypress.
pub const BAND_HEIGHT: u16 = 6;

/// What `layout::panes` should shorten the editor by: the band's fixed
/// height while a Story is being walked, zero everywhere else. One function
/// so every call site asks the same question rather than re-deriving it.
pub fn band_height(state: &State) -> u16 {
    if state.walking.is_some() {
        BAND_HEIGHT
    } else {
        0
    }
}

/// What `layout::panes` should narrow the editor by: the step-menu's fixed
/// width while walking a Story, zero everywhere else — including the
/// Remainder, which has no Step names to show. Mirrors [`band_height`].
pub fn step_menu_width(state: &State) -> u16 {
    if matches!(state.walking, Some(Walking::Story { .. })) {
        crate::layout::STEP_MENU_WIDTH
    } else {
        0
    }
}

/// One row of the step-menu: a Step's name, and whether it is the one
/// currently being walked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepMenuRow {
    pub name: String,
    pub current: bool,
}

/// The current Story's Steps, in order, for the step-menu — empty while not
/// walking, and empty for [`Walking::Remainder`], which has no names to show.
/// Mirrors [`mark`]'s precedent: one pure query, the widget draws what it is
/// told.
pub fn step_menu(state: &State) -> Vec<StepMenuRow> {
    let Some(Walking::Story { story, step, .. }) = state.walking else {
        return Vec::new();
    };
    let Set::Loaded(artifact) = &state.story_set else {
        return Vec::new();
    };
    let Some(current) = artifact.stories.get(story) else {
        return Vec::new();
    };
    current
        .steps
        .iter()
        .enumerate()
        .map(|(index, s)| StepMenuRow {
            name: s.name.clone(),
            current: index == step,
        })
        .collect()
}

/// `n`/`p`'s clamp: stepping past the last Step holds there, and stepping
/// back at the first Step holds there too — walking a Story never leaves it.
pub fn advance(total_steps: usize, current: usize, direction: Direction) -> usize {
    match direction {
        Direction::Left | Direction::Up => current.saturating_sub(1),
        _ => (current + 1).min(total_steps.saturating_sub(1)),
    }
}

/// The overlay's sections, in the fixed order `D` always shows them. Claim
/// and why are always there; flow and nudge are each omitted when the Step
/// does not have one, the same way the overlay itself renders — one model,
/// not two that could disagree. `stale` sits right after why: a Step whose
/// Site moved is not read the same way as one that did not, so the overlay
/// says so before it gets to flow or the nudge.
pub fn detail_sections(state: &State, step: &Step) -> Vec<&'static str> {
    let mut sections = vec!["claim", "why"];
    if staleness(state, step).is_stale() {
        sections.push("stale");
    }
    if step.flow.is_some() {
        sections.push("flow");
    }
    if step.nudge.is_some() {
        sections.push("nudge");
    }
    sections
}

/// The Step the reviewer is on, or `None` with nothing loaded, nothing being
/// walked, or the Remainder being walked instead — the Remainder is never a
/// Story, so it never has a Step to answer with.
pub fn current_step(state: &State) -> Option<&Step> {
    let Walking::Story { story, step, .. } = state.walking? else {
        return None;
    };
    match &state.story_set {
        Set::Loaded(artifact) => artifact.stories.get(story)?.steps.get(step),
        _ => None,
    }
}

/// The choice texts the prediction overlay currently offers: every choice
/// before any pick and after a wrong one, none once a correct pick replaced
/// them with its feedback alone. Never paired with which one is correct —
/// that is exactly what a wrong pick must not reveal.
pub fn prediction_choices(state: &State) -> Vec<&str> {
    let Modal::Prediction { picked } = &state.modal else {
        return Vec::new();
    };
    let Some(prediction) = current_step(state).and_then(|step| step.prediction.as_ref()) else {
        return Vec::new();
    };
    let answered_correctly = picked
        .and_then(|index| prediction.choices.get(index))
        .is_some_and(|choice| choice.correct);
    if answered_correctly {
        Vec::new()
    } else {
        prediction
            .choices
            .iter()
            .map(|choice| choice.text.as_str())
            .collect()
    }
}

pub fn prediction_feedback(state: &State) -> Option<&str> {
    let Modal::Prediction { picked } = &state.modal else {
        return None;
    };
    let prediction = current_step(state)?.prediction.as_ref()?;
    prediction
        .choices
        .get((*picked)?)
        .map(|choice| choice.feedback.as_str())
}

/// The loaded artifact, or `None` for every other `Set` variant — the one
/// match every Loaded-only query (`spine`, `title`) reduces to, spelled out
/// in full here so a `Set` variant nobody has answered for yet is a compiler
/// error at this one site rather than a silent gap at each of its callers.
fn loaded(state: &State) -> Option<&Artifact> {
    match &state.story_set {
        Set::Loaded(artifact) => Some(artifact),
        Set::None
        | Set::Filling { .. }
        | Set::Refused { .. }
        | Set::RangeGone
        | Set::NoDefaultBranch
        | Set::BadRange
        | Set::NotARepository
        | Set::WorkingTreeDirty
        | Set::GuestNeedsBareWorkspace
        | Set::NoGit
        | Set::Downloading { .. }
        | Set::DownloadFailed { .. }
        | Set::Authoring { .. }
        | Set::AuthoringAbandoned
        | Set::Fixing { .. } => None,
    }
}

/// Each Story by name, with how many Steps it has — the shape of the change,
/// before any of it is walked.
pub fn spine(state: &State) -> Vec<SpineRow> {
    let Some(artifact) = loaded(state) else {
        return Vec::new();
    };
    artifact
        .stories
        .iter()
        .map(|story| SpineRow {
            name: story.name.clone(),
            steps: story.steps.len(),
            stale: story
                .steps
                .iter()
                .filter(|step| staleness(state, step).is_stale())
                .count(),
        })
        .collect()
}

/// What the whole range accomplishes at large — the header the spine draws
/// above its Story rows. `None` for every `Set` variant that isn't `Loaded`,
/// matching how `spine` itself answers nothing for the same states.
pub fn title(state: &State) -> Option<&str> {
    loaded(state).map(|artifact| artifact.title.as_str())
}

/// The spine's selectable rows: every Story, plus one more for the Remainder
/// when it holds anything to walk. What `story_selection` ranges over, and
/// what the spine's own scroll viewport is computed against — one source of
/// truth for "how many rows are actually there" so the selection clamp and
/// the wheel can never disagree about it.
pub fn spine_row_count(state: &State) -> usize {
    spine(state).len() + usize::from(!remainder_locations(state).is_empty())
}

/// How many rows of the tree pane's budget the spine's title header spends —
/// one when the spine is showing a loaded title, none otherwise. The single
/// source of truth `fits` reads so the visible Story-row count and the row
/// the header actually draws never disagree, the same way `tree::filter_rows`
/// already is for the filter box.
pub fn title_rows(state: &State) -> usize {
    if state.view == crate::View::Story
        && state.story_listing == Listing::Spine
        && title(state).is_some()
    {
        1
    } else {
        0
    }
}

/// A folder with no story set is not the same situation as one whose set was
/// refused or whose range is gone, so they get different answers.
pub fn view_state(state: &State) -> &'static str {
    match &state.story_set {
        Set::None => "no-stories",
        Set::Loaded(_) => "spine",
        Set::Filling { .. } => "filling",
        Set::Refused { .. } => "story-artifact-invalid",
        Set::RangeGone => "range-unresolvable",
        Set::NoDefaultBranch => "no-default-branch",
        Set::BadRange => "bad-range",
        Set::NotARepository => "not-a-git-repository",
        Set::WorkingTreeDirty => "working-tree-dirty",
        Set::GuestNeedsBareWorkspace => "guest-needs-bare-workspace",
        Set::NoGit => "git-not-installed",
        Set::Downloading {
            how: Download::Clone,
            ..
        } => "cloning",
        Set::Downloading {
            how: Download::Fetch,
            ..
        } => "fetching",
        Set::DownloadFailed {
            how: Download::Clone,
            ..
        } => "clone-failed",
        Set::DownloadFailed {
            how: Download::Fetch,
            ..
        } => "fetch-failed",
        Set::Authoring { .. } => "authoring",
        Set::AuthoringAbandoned => "authoring-abandoned",
        Set::Fixing { .. } => "fixing",
    }
}

/// A story set is named `<base>-<head>.json` for the revisions it describes
/// (ADR 0005). `resolves` is git's answer for a revision, supplied by the
/// caller so this module never has to touch git itself; `-worktree` needs no
/// commit to exist, since it names the uncommitted tree rather than one.
pub fn range_status(file: &Path, resolves: impl Fn(&str) -> bool) -> RangeStatus {
    let stem = file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    match stem.split_once('-') {
        Some((base, head)) if resolves(base) && (head == WORKTREE || resolves(head)) => {
            RangeStatus::Resolves
        }
        _ => RangeStatus::Gone,
    }
}

/// Where a story set for these revisions belongs, named per ADR 0005 so
/// re-authoring replaces by naming rather than by logic. `base` and `head`
/// are each a 12-char oid prefix, or `head` is the literal `worktree` — the
/// same shape [`range_status`] reads back out of a filename.
/// `dir` is Varde's own directory ([`crate::varde_dir`]), which is absolute:
/// the path is handed to an AI session whose working directory Varde did not
/// set, so a relative one would name a file somewhere nobody agreed on — and
/// in a Bare workspace it would land in the folder ticket 03 promises to
/// leave alone.
pub fn artifact_path(dir: &Path, base: &str, head: &str) -> String {
    format!("{}/stories/{base}-{head}.json", dir.display())
}

/// The spelling for a base and a head: git's three-dot form, so the base is
/// the merge-base rather than the base's tip. A base branch that moved on
/// after the head forked otherwise reads as the head having deleted every
/// commit that landed meanwhile, and the Story set authored from it describes
/// a change nobody made.
pub fn range(base: &str, head: &str) -> String {
    format!("{base}...{head}")
}

/// The two revisions a spelling names, and whether the base is their
/// merge-base. Which two to resolve is this module's decision; resolving them
/// is the edge's `git2` work.
pub fn revisions(spelling: &str) -> Option<(&str, &str, bool)> {
    match spelling.split_once("...") {
        Some((base, head)) => Some((base, head, true)),
        None => spelling
            .split_once("..")
            .map(|(base, head)| (base, head, false)),
    }
}

/// What a range's head is spelled when it is the working tree rather than a
/// commit (ADR 0005). One home for it: it appears in a story set's filename,
/// in a recorded `Range`, and in every edge read that has to tell a committed
/// range from a dirty one.
pub const WORKTREE: &str = "worktree";

/// Varde's own pinned `context_lines`, shared with `main.rs`'s `diff_lines`
/// so a claim and the Remainder always subtract the same shape.
pub const CONTEXT_LINES: u32 = 3;

/// Which two revisions the hunk inventory behind the Remainder is a diff of.
/// Only the edge can ask git, but *what* to ask it is a decision about what
/// the range means, so it is decided here and read there. The one canonical
/// statement of why: every other site that needs it links here rather than
/// restating it, so the invariant cannot drift into three versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inventory<'a> {
    /// `HEAD` against the working tree, as git reports it. What a bare
    /// `:story` on a dirty tree names, and the answer whenever no Story is
    /// loaded to name anything else.
    Worktree,
    /// A range between two real commits, diffed against itself. Diffing the
    /// working tree instead is why a fully committed range used to produce no
    /// hunks at all — a clean tree differs from `HEAD` in nothing — so the
    /// Remainder reported vacuous full coverage for every one of them.
    Committed { base: &'a str, head: &'a str },
}

/// See [`Inventory`]. A loaded Story's range names its own two revisions,
/// except when its head is the literal `worktree`: that range names no commit
/// of its own, so its new side is whatever is on disk right now.
pub fn inventory(set: &Set) -> Inventory<'_> {
    match set {
        // `Filling` as well as `Loaded`: the range is known the moment the
        // artifact parses, and the checks that run before it is walkable are
        // run against these hunks. Waiting for `Loaded` would check a
        // committed range against the working tree and flag every Step.
        Set::Loaded(artifact) | Set::Filling { artifact, .. } => inventory_of(artifact),
        _ => Inventory::Worktree,
    }
}

/// Every file the Story in this [`Set`] names, so the edge's read gives each
/// one an entry whether or not the range touched it. The same two variants
/// [`inventory`] answers for, and for the same reason: a set still filling
/// names its files too, because the checks that decide whether it becomes
/// walkable are run against exactly that read.
pub fn named_files(set: &Set) -> Vec<String> {
    match set {
        Set::Loaded(artifact) | Set::Filling { artifact, .. } => story_files(artifact),
        Set::None
        | Set::Refused { .. }
        | Set::RangeGone
        | Set::NoDefaultBranch
        | Set::BadRange
        | Set::NotARepository
        | Set::WorkingTreeDirty
        | Set::GuestNeedsBareWorkspace
        | Set::NoGit
        | Set::Downloading { .. }
        | Set::DownloadFailed { .. }
        | Set::Authoring { .. }
        | Set::AuthoringAbandoned
        | Set::Fixing { .. } => Vec::new(),
    }
}

/// See [`Inventory`]. Split out because the arriving artifact is asked this
/// before it is anywhere in a [`Set`] — the read effect carries the answer, so
/// the checks run against the range's own hunks rather than the poll's.
pub fn inventory_of(artifact: &Artifact) -> Inventory<'_> {
    if artifact.range.head == WORKTREE {
        return Inventory::Worktree;
    }
    Inventory::Committed {
        base: &artifact.range.base,
        head: &artifact.range.head,
    }
}

/// One hunk of a diff between a file's two sides, at [`CONTEXT_LINES`] so a
/// claim and the Remainder always subtract the same shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

impl Hunk {
    /// `git2`'s own `+n,0` shape for a hunk that removes every line it
    /// touches and adds none — the only fact the deletions line needs.
    fn is_deletion(&self) -> bool {
        self.new_lines == 0
    }
}

/// The hunks in one changed file, as the edge hands them to the core after
/// diffing both sides — the same shape `repo` already arrives in. Carries
/// each side's full current text too, polled on the same cadence as the
/// hunks: a Site's staleness needs the text at its own range, not only
/// whether a hunk overlaps it, and this is the one channel that already
/// reads both sides regardless of whether the tree has expanded to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHunks {
    pub file: String,
    pub hunks: Vec<Hunk>,
    pub old_exists: bool,
    pub old_text: String,
    pub new_exists: bool,
    pub new_text: String,
    /// The range's *own* new side — the head commit's copy for a committed
    /// range, and the working tree's when the range has no commit of its own.
    /// The same file as `new_text` in the ordinary case, and deliberately not
    /// the same read: [`staleness`] and [`fill`] are about the code the
    /// reviewer is looking at, so they read the working tree, while
    /// [`problems`] judges what the AI wrote against the range it wrote it
    /// for. Judging a Step at the working tree would refuse a correct Story
    /// the morning after, for the varde of the reviewer having kept typing —
    /// which is what the stale marker is for.
    pub head_exists: bool,
    pub head_text: String,
}

/// Diffs two sides of one file with Varde's pinned options. Pure: bytes in,
/// hunks out. No repository is opened and no file is read — `git2::Patch`
/// diffs the two buffers handed to it and nothing else.
pub fn hunks(old: &[u8], new: &[u8]) -> Vec<Hunk> {
    let mut options = git2::DiffOptions::new();
    options.context_lines(CONTEXT_LINES);
    let Ok(patch) = git2::Patch::from_buffers(old, None, new, None, Some(&mut options)) else {
        return Vec::new();
    };
    (0..patch.num_hunks())
        .filter_map(|index| patch.hunk(index).ok())
        .map(|(hunk, _lines)| Hunk {
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
        })
        .collect()
}

/// What the spine shows under the Stories: the hunks no Step claims, and how
/// many of the range's deletions went unwalked. Never a Story itself — no
/// premise, nothing authored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Remainder {
    pub unclaimed: usize,
    pub locations: Vec<String>,
    /// `None` when the range deletes nothing at all; `Some(n)` — even
    /// `Some(0)` — once it deletes something, so the spine can tell "nothing
    /// was deleted" from "everything deleted was walked".
    pub unwalked_deletions: Option<usize>,
}

fn overlaps(from: u32, to: u32, start: u32, lines: u32) -> bool {
    lines > 0 && from < start + lines && start <= to
}

/// Every Site any Story's Step holds, flattened once so both `remainder` and
/// `remainder_locations` ask the same question about the same set.
fn claim_sites(state: &State) -> Vec<&Site> {
    match &state.story_set {
        Set::Loaded(artifact) => artifact
            .stories
            .iter()
            .flat_map(|story| story.steps.iter().map(|step| &step.site))
            .collect(),
        Set::None
        | Set::Filling { .. }
        | Set::Refused { .. }
        | Set::RangeGone
        | Set::NoDefaultBranch
        | Set::BadRange
        | Set::NotARepository
        | Set::WorkingTreeDirty
        | Set::GuestNeedsBareWorkspace
        | Set::NoGit
        | Set::Downloading { .. }
        | Set::DownloadFailed { .. }
        | Set::Authoring { .. }
        | Set::AuthoringAbandoned
        | Set::Fixing { .. } => Vec::new(),
    }
}

/// Whether one `changed` Site claims one hunk of one file. The single
/// predicate behind both halves of the same question: the Remainder asks it of
/// every Site to count what is covered, and [`problems`] asks it of every hunk
/// to catch a Step claiming a change that is not there. A second predicate
/// could disagree with the count, and the disagreement would be invisible.
fn claims(site: &Site, file: &str, hunk: &Hunk) -> bool {
    site.file == file
        && matches!(site.kind, Kind::Changed)
        && match site.side {
            Side::Old => overlaps(site.from, site.to, hunk.old_start, hunk.old_lines),
            Side::New => overlaps(site.from, site.to, hunk.new_start, hunk.new_lines),
        }
}

/// A hunk is claimed the moment any `changed`-kind Site overlaps it, however
/// many Sites point at it — coverage is a set, not a sum.
fn is_claimed(sites: &[&Site], file: &str, hunk: &Hunk) -> bool {
    sites.iter().any(|site| claims(site, file, hunk))
}

/// Recomputed every call from `state.file_hunks`, never cached against the
/// artifact — a Remainder frozen at authoring would read "nothing left" while
/// the AI kept writing.
pub fn remainder(state: &State) -> Remainder {
    let sites = claim_sites(state);
    let mut result = Remainder::default();
    let mut deletions_total = 0usize;
    let mut deletions_unwalked = 0usize;

    for file in &state.file_hunks {
        for hunk in &file.hunks {
            let claimed = is_claimed(&sites, &file.file, hunk);
            if hunk.is_deletion() {
                deletions_total += 1;
                if !claimed {
                    deletions_unwalked += 1;
                }
            } else if !claimed {
                result.unclaimed += 1;
                result.locations.push(file.file.clone());
            }
        }
    }

    result.unwalked_deletions = (deletions_total > 0).then_some(deletions_unwalked);
    result
}

/// One unclaimed hunk's file and its extent, for walking the Remainder —
/// never a Step: the Remainder claims nothing, so there is nothing here
/// beyond a bare location. In the same order `remainder`'s own
/// `locations` lists them, over the same hunks, so walking and the count
/// never disagree about what is unclaimed. Deletions are excluded, the same
/// way they never inflate `Remainder::unclaimed`: there is no new-side line
/// left to land a cursor on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemainderLocation {
    pub file: String,
    /// The hunk's first line, where the cursor lands, and its last. The end
    /// is carried because the Remainder has no claim at all: the code on
    /// screen is the whole message, so a mark on the line the walk starts at
    /// says where to look and nothing about how far.
    pub from: u32,
    pub to: u32,
}

pub fn remainder_locations(state: &State) -> Vec<RemainderLocation> {
    let sites = claim_sites(state);
    let mut locations = Vec::new();
    for file in &state.file_hunks {
        for hunk in &file.hunks {
            if !hunk.is_deletion() && !is_claimed(&sites, &file.file, hunk) {
                // `new_lines - 1` leans on the `is_deletion` guard above —
                // that is exactly `new_lines == 0`, and in release the
                // subtraction would wrap rather than panic, marking four
                // billion lines instead of crashing.
                locations.push(RemainderLocation {
                    file: file.file.clone(),
                    from: hunk.new_start,
                    to: hunk.new_start + hunk.new_lines - 1,
                });
            }
        }
    }
    locations
}

/// What the code surface must draw for where the Walkthrough stands. Three
/// variants rather than an `Option`, because a refusal is not an absence: the
/// old side has a Site and it cannot be shown, and folding that into "nothing
/// to draw" is exactly how new code comes to sit under old line numbers with
/// nothing on screen saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteMark {
    Nothing,
    Site {
        file: String,
        from: u32,
        to: u32,
        kind: Kind,
    },
    /// The old side: `OpenAt` reads the working tree, so the code on screen is
    /// the new side at old line numbers. Carries no reason yet — there is only
    /// one, and a payload nothing reads is a field that drifts.
    Refused,
}

impl SiteMark {
    /// The one question the code surface asks per line. Dimming has no query
    /// of its own: a line is dimmed exactly when a mark exists and this
    /// answers `false`, so the renderer asks [`mark`] once and derives the
    /// rest — two queries could disagree, and a disagreement here shows the
    /// reviewer a mark over the wrong lines.
    pub fn covers(&self, file: &str, line: u32) -> bool {
        match self {
            SiteMark::Site {
                file: marked,
                from,
                to,
                ..
            } => marked == file && (*from..=*to).contains(&line),
            SiteMark::Nothing | SiteMark::Refused => false,
        }
    }

    /// The word a scenario names this by. A refusal and an absence answer
    /// differently on purpose: "no mark" is true of both, so it is the only
    /// thing that tells a Step whose code is being withheld from a reviewer who
    /// is not walking at all.
    pub fn as_str(&self) -> &'static str {
        match self {
            SiteMark::Nothing => "not-walking",
            SiteMark::Site { .. } => "shown",
            SiteMark::Refused => "old-side-not-shown",
        }
    }
}

/// Which lines the current Step points at, derived from where the Walkthrough
/// stands on every draw. Never a `State` field and never a toggle: a mark that
/// is remembered has a second author and goes stale against the position it
/// claims to describe, and a mark that can be dismissed leaves a reviewer
/// reading a screen of code rather than a Step.
///
/// Independent of the cursor, which is what keeps it drawn under an overlay —
/// the caret is suppressed while one is up, and a Step carrying a Prediction
/// arrives with its overlay already open.
///
/// An old-side Site refuses rather than marking: `OpenAt` is executed at the
/// edge as a read of the working-tree file, so the code on screen is the new
/// side at old line numbers. A mark over it would be a lie about what is
/// underneath it.
pub fn mark(state: &State) -> SiteMark {
    match state.walking {
        None => SiteMark::Nothing,
        Some(Walking::Story { .. }) => match current_step(state) {
            None => SiteMark::Nothing,
            Some(step) => match step.site.side {
                Side::Old => SiteMark::Refused,
                Side::New => SiteMark::Site {
                    file: step.site.file.clone(),
                    from: step.site.from,
                    to: step.site.to,
                    kind: step.site.kind,
                },
            },
        },
        // The Remainder claims nothing, so there is no `kind` authored for
        // it — but every unclaimed hunk is by definition part of the change,
        // which is the whole reason it is walked at all.
        Some(Walking::Remainder { index }) => match remainder_locations(state).get(index) {
            None => SiteMark::Nothing,
            Some(location) => SiteMark::Site {
                file: location.file.clone(),
                from: location.from,
                to: location.to,
                kind: Kind::Changed,
            },
        },
    }
}

/// Whether the code surface draws the old-side notice instead of code: an
/// old-side Site, unless `d` is showing the Range's diff over it — the one way
/// its old text is shown truthfully, as the rows the change removed. Asked by
/// the renderer, the caret and both scroll clamps, which must agree on it.
pub fn refused(state: &State) -> bool {
    matches!(mark(state), SiteMark::Refused) && site_diff(state).is_none()
}

/// The file on screen, relative to the repository under review — the spelling
/// a Site is authored with, and so the one the mark and the comment lookup must
/// use. The buffer's path is absolute; comparing that finds nothing and shows no
/// comment, silently. The *repository's* root, not the workspace's: a Guest
/// repo's files are opened inside the clone, and relative to the workspace they
/// spell `.varde/…/src/x.rs` — a name no Site has, so the mark covered nothing
/// and every line of a Guest walk was dimmed.
pub fn shown_file(state: &State) -> String {
    state
        .current_buffer
        .as_ref()
        .map(|path| {
            path.strip_prefix(state.repo_root())
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_default()
}

/// One row of Story view's code surface. A code row carries its **line
/// number** rather than being found by its position, because a comment row
/// sits between lines: once one is there, the row a line is drawn on is no
/// longer the line it is. Asking [`SiteMark::covers`] about a row instead of a
/// line would slide the bar down one row per comment above it — the mark
/// pointing at code it does not cover.
#[derive(Debug, PartialEq)]
pub enum Row<'a> {
    Code(u32),
    Comment(&'a crate::review::Comment),
    /// A line the Range removed, drawn where it was while [`Diff::Shown`]. It
    /// has no line in the buffer, so it answers the line it sits under, the
    /// way a comment row does.
    Removed(String),
}

/// Story view's rows for a code surface of `lines` lines: every line in order,
/// each followed by the comments anchored to it. Reuses Review's own
/// [`crate::review::comments_at`] rather than growing a second lookup.
///
/// Gated on `walking` because that is exactly what `ui::editor_widget`
/// dispatches the story surface on: everything deriving a row from a line
/// (the caret, the scroll clamp, the mouse hit-test) asks this for every
/// buffer, so a surface drawn without comment rows must answer without them.
pub fn rows(state: &State, lines: usize) -> Vec<Row<'_>> {
    // A folded block's body is not drawn, so it is not a row — asked here
    // rather than by each of this map's readers, for the reason the comment
    // rows are interleaved here: a second count of which lines are on screen
    // is one of them landing a row off.
    let hidden = crate::fold::hidden(state);
    let shown = (1..=lines as u32).filter(move |number| !hidden.contains(&(*number as usize)));
    let file = match state.walking {
        Some(_) => shown_file(state),
        None => return shown.map(Row::Code).collect(),
    };
    // Under the comments: a comment is about the line above it, and a removed
    // line is about the gap before the next one.
    let removed = site_diff(state).map_or_else(Vec::new, |diff| diff.removed);
    let removed_under = |line: u32| {
        removed
            .iter()
            .filter(move |(under, _)| *under == line)
            .map(|(_, text)| Row::Removed(text.clone()))
    };
    removed_under(0)
        .chain(shown.flat_map(|number| {
            std::iter::once(Row::Code(number))
                .chain(
                    crate::review::comments_at(state, &file, number)
                        .into_iter()
                        .map(Row::Comment),
                )
                .chain(removed_under(number))
        }))
        .collect()
}

/// What the Range changed inside a Site: the new-side lines it added, and the
/// lines it removed, each with the new-side line it sits under (0 above the
/// first).
#[derive(Debug, PartialEq, Eq)]
pub struct SiteDiff {
    pub added: Vec<u32>,
    pub removed: Vec<(u32, String)>,
}

/// The Range's diff over the current Step's Site, or `None` wherever `d` draws
/// nothing: the diff hidden, the Remainder, a context Site — which the change
/// did not touch by definition — or a file on screen other than the Site's,
/// which `g` leaves the walk on.
///
/// The Range's two sides, `old_text` against `head_text`, never the buffer: the
/// question is what the change did, and the buffer is what the reviewer has
/// done since.
pub fn site_diff(state: &State) -> Option<SiteDiff> {
    let Some(Walking::Story {
        diff: Diff::Shown, ..
    }) = state.walking
    else {
        return None;
    };
    let site = &current_step(state)?.site;
    if site.kind == Kind::Context || shown_file(state) != site.file {
        return None;
    }
    let file = state
        .file_hunks
        .iter()
        .find(|file| file.file == site.file)?;
    Some(changes(
        &file.old_text,
        &file.head_text,
        site.side,
        site.from,
        site.to,
    ))
}

/// One run of `-` and `+` lines between two context lines, which is the unit a
/// Site either reaches or does not: an edited line is a removal and an
/// addition, and showing one without the other shows half the edit.
#[derive(Default)]
struct Block {
    removed: Vec<(u32, u32, String)>,
    added: Vec<u32>,
}

/// [`site_diff`] over two texts. A new-side Site takes the blocks with an
/// addition inside it, or a bare deletion between two of its lines, and marks
/// only the additions inside it — code outside a Site is drawn as it always
/// is. An old-side Site names removed lines, so it takes the blocks that
/// removed one of them, whole. `git2` finds the lines, for the reason [`hunks`]
/// asks it for the boundaries.
fn changes(old: &str, new: &str, side: Side, from: u32, to: u32) -> SiteDiff {
    let mut diff = SiteDiff {
        added: Vec::new(),
        removed: Vec::new(),
    };
    let Ok(patch) = git2::Patch::from_buffers(old.as_bytes(), None, new.as_bytes(), None, None)
    else {
        return diff;
    };
    let mut keep = |block: Block| {
        let reached = match side {
            Side::New if block.added.is_empty() => block
                .removed
                .first()
                .is_some_and(|(_, under, _)| (from..to).contains(under)),
            Side::New => block.added.iter().any(|line| (from..=to).contains(line)),
            Side::Old => block
                .removed
                .iter()
                .any(|(line, _, _)| (from..=to).contains(line)),
        };
        if !reached {
            return;
        }
        diff.added.extend(
            block
                .added
                .into_iter()
                .filter(|line| side == Side::Old || (from..=to).contains(line)),
        );
        diff.removed.extend(
            block
                .removed
                .into_iter()
                .map(|(_, under, text)| (under, text)),
        );
    };
    for hunk in 0..patch.num_hunks() {
        let Ok((header, count)) = patch.hunk(hunk) else {
            continue;
        };
        // git names the line *before* a deletion as the start of an empty new
        // side, and the first line of a non-empty one.
        let mut under = match header.new_lines() {
            0 => header.new_start(),
            _ => header.new_start().saturating_sub(1),
        };
        let mut block = Block::default();
        for index in 0..count {
            let Ok(line) = patch.line_in_hunk(hunk, index) else {
                continue;
            };
            match (line.origin(), line.old_lineno(), line.new_lineno()) {
                ('-', Some(old_line), _) => block.removed.push((
                    old_line,
                    under,
                    String::from_utf8_lossy(line.content())
                        .trim_end_matches(['\n', '\r'])
                        .to_string(),
                )),
                ('+', _, Some(new_line)) => {
                    under = new_line;
                    block.added.push(new_line);
                }
                (' ', _, Some(new_line)) => {
                    keep(std::mem::take(&mut block));
                    under = new_line;
                }
                // The "no newline at end of file" markers, which are no line.
                _ => {}
            }
        }
        keep(block);
    }
    diff
}

/// The screen row a line is drawn on, 1-based, counting the comment rows above
/// it. The mark and the Site count *lines*; the caret counts *rows*, and a
/// comment row between them makes the two stop agreeing. Derived from [`rows`]
/// rather than counting comments a second time: two counts of one interleave
/// is exactly how a caret comes to sit a row off the line it is on.
pub fn row_of(state: &State, line: u32) -> usize {
    let hidden = crate::fold::hidden(state);
    if state.walking.is_none() && hidden.is_empty() {
        return line as usize;
    }
    // Every row above this line, comment rows counted and folded-away lines
    // not. A line inside a fold answers the row its opening line is drawn on,
    // which is where the reader can see it: there is no row of its own to name.
    let above = rows(state, line.saturating_sub(1) as usize).len();
    match hidden.contains(&(line as usize)) {
        true => above.max(1),
        false => above + 1,
    }
}

/// The line a screen row shows — the inverse of [`row_of`], for a click. Both
/// directions have to exist or they are two derivations of one coordinate: a
/// caret that counts comment rows and a click that does not put the cursor
/// somewhere the pointer never was.
///
/// A comment row answers the line it sits under, because a comment is *about*
/// that line and clicking one means it.
pub fn line_at_row(state: &State, row: usize) -> usize {
    let hidden = crate::fold::hidden(state);
    if state.walking.is_none() && hidden.is_empty() {
        return row;
    }
    let mut line = row;
    // `rows` yields at most one row per line while lines are folded away, so
    // the whole buffer is laid out rather than the first `row` lines of it —
    // a row past the last one answers the last line there is, which is the
    // click landing on the bottom of what is on screen.
    let lines = crate::current_buffer(state).map_or(0, |buffer| buffer.shown().split('\n').count());
    for (index, entry) in rows(state, lines.max(row)).iter().enumerate() {
        if let Row::Code(number) = entry {
            line = *number as usize;
        }
        if index + 1 == row {
            break;
        }
    }
    line
}

#[cfg(test)]
mod tests {

    use super::*;

    /// The picker's whole ordering and dedup contract. A repository states its
    /// branches as refs; what a reviewer wants is a list of *branches*, newest
    /// first, so a branch pushed but never checked out is reachable and a
    /// branch that exists both locally and on a remote is one row.
    #[test]
    fn the_branch_list_is_deduped_by_short_name_and_newest_first() {
        let refs = [
            branch_ref("stale", false, 100),
            branch_ref("origin/feature", true, 300),
            branch_ref("feature", false, 300),
            branch_ref("main", false, 200),
            branch_ref("origin/pushed-only", true, 250),
            // Not a branch: `origin/HEAD` is the remote's own pointer at one
            // of the rows already here, so listing it offers the same branch
            // under a name nobody typed.
            branch_ref("origin/HEAD", true, 400),
        ];
        assert_eq!(
            branches(&refs, ""),
            ["feature", "pushed-only", "main", "stale"]
        );
    }

    /// Two refs at the same commit date is the ordinary case for a branch and
    /// the remote it tracks, so the tie cannot be left to whatever order the
    /// edge happened to read them in.
    #[test]
    fn a_tie_on_the_commit_date_is_broken_by_name() {
        let refs = [
            branch_ref("b", false, 1),
            branch_ref("a", false, 1),
            branch_ref("origin/c", true, 1),
        ];
        assert_eq!(branches(&refs, ""), ["a", "b", "c"]);
    }

    /// The picker's refusal is about the reviewer's work, and Varde's own
    /// directory is not it — a project workspace has files written into it by
    /// being opened, and Varde never edits the `.gitignore` that would hide
    /// them.
    #[test]
    fn varde_s_own_directory_is_not_uncommitted_work() {
        let file = |path: &str| crate::review::GitFile {
            path: path.to_string(),
            status: crate::review::GitStatus::Untracked,
        };
        assert!(!uncommitted(&[]));
        assert!(!uncommitted(&[
            file(".varde/state.json"),
            file(".varde/risk.json")
        ]));
        assert!(uncommitted(&[
            file(".varde/state.json"),
            file("src/keys.rs")
        ]));
        // Not a prefix match on the string: a project's own folder whose name
        // begins with Varde's is the project's.
        assert!(uncommitted(&[file(".varden/notes.md")]));
    }

    /// Typing narrows the same list rather than a second one: the order the
    /// dates decided survives it, a literal match hides the scattered ones the
    /// tree filter would otherwise offer, and clearing what was typed brings
    /// the whole list back. A filter matching nothing answers nothing — what
    /// the box says about that is `ui`'s.
    #[test]
    fn typing_narrows_the_list_without_reordering_it() {
        let refs = [
            branch_ref("fix-the-tree", false, 100),
            branch_ref("origin/feature", true, 300),
            branch_ref("main", false, 200),
        ];
        assert_eq!(branches(&refs, "fea"), ["feature"]);
        // Date order, not match order: "fix-the-tree" is the closer match to
        // "fe" by every ranking, and it is still last because it is older.
        assert_eq!(branches(&refs, "e"), ["feature", "fix-the-tree"]);
        // Fuzzy only where nothing is literal — the tree's rule, so "fxt"
        // still reaches a name nobody can spell in full.
        assert_eq!(branches(&refs, "fxt"), ["fix-the-tree"]);
        assert!(branches(&refs, "nope").is_empty());
        assert_eq!(branches(&refs, ""), ["feature", "main", "fix-the-tree"]);
    }

    fn branch_ref(name: &str, remote: bool, when: i64) -> BranchRef {
        BranchRef {
            name: name.to_string(),
            remote,
            when,
        }
    }

    #[test]
    fn a_range_is_the_merge_base_against_the_base() {
        assert_eq!(range("main", "HEAD"), "main...HEAD");
        assert_eq!(revisions("main...HEAD"), Some(("main", "HEAD", true)));
    }

    /// The uncommitted range keeps two dots: there is no merge-base to take
    /// against a working tree, and `HEAD` is by definition not behind itself.
    #[test]
    fn the_uncommitted_range_is_still_two_dots() {
        assert_eq!(revisions("HEAD..worktree"), Some(("HEAD", WORKTREE, false)));
        assert_eq!(revisions("HEAD"), None);
    }

    /// The shape a real authoring run produces: `choices[].id`, no
    /// `site.hash`, no `flow`/`values`/`nudge` — every field this schema
    /// does not name, which must not refuse the artifact.
    #[test]
    fn an_artifact_with_fields_this_schema_does_not_name_still_parses() {
        let text = r#"{
            "protocolVersion": 2,
            "title": "Keys reach the child",
            "range": { "base": "aaaaaaaaaaaa", "head": "bbbbbbbbbbbb", "spelling": "main..HEAD" },
            "stories": [{
                "id": "s1", "name": "Keys reach the child", "premise": "…",
                "steps": [{
                    "id": "s1e1", "name": "Router matches", "claim": "…", "why": "…",
                    "site": { "file": "src/keys.rs", "side": "new", "kind": "changed",
                              "from": 10, "to": 12, "hash": "abc123", "text": "a\nb\nc" },
                    "prediction": { "question": "why?", "choices": [
                        { "id": "c1", "text": "a", "correct": true, "feedback": "yes" },
                        { "id": "c2", "text": "b", "correct": false, "feedback": "no" },
                        { "id": "c3", "text": "c", "correct": false, "feedback": "no" }
                    ] }
                }]
            }]
        }"#;
        assert!(parse(text).is_ok());
    }

    #[test]
    fn a_missing_required_field_refuses_the_whole_artifact() {
        let text =
            r#"{ "protocolVersion": 2, "stories": [ { "id": "s1", "name": "Half a story" } ] }"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn a_side_the_schema_does_not_name_refuses() {
        let text = r#"{
            "protocolVersion": 2,
            "title": "t",
            "range": { "base": "a", "head": "b", "spelling": "main..HEAD" },
            "stories": [{ "id": "s1", "name": "n", "premise": "p", "steps": [{
                "id": "s1e1", "name": "n", "claim": "c", "why": "w",
                "site": { "file": "f", "side": "left", "kind": "changed", "from": 1, "to": 1, "text": "t" }
            }] }]
        }"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn an_artifact_missing_a_title_refuses() {
        let text = r#"{
            "protocolVersion": 2,
            "range": { "base": "a", "head": "b", "spelling": "main..HEAD" },
            "stories": [{ "id": "s1", "name": "n", "premise": "p", "steps": [{
                "id": "s1e1", "name": "n", "claim": "c", "why": "w",
                "site": { "file": "f", "side": "new", "kind": "changed", "from": 1, "to": 1, "text": "t" }
            }] }]
        }"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn a_step_missing_a_name_refuses_the_whole_artifact() {
        let text = r#"{
            "protocolVersion": 2,
            "title": "t",
            "range": { "base": "a", "head": "b", "spelling": "main..HEAD" },
            "stories": [{ "id": "s1", "name": "n", "premise": "p", "steps": [{
                "id": "s1e1", "claim": "c", "why": "w",
                "site": { "file": "f", "side": "new", "kind": "changed", "from": 1, "to": 1, "text": "t" }
            }] }]
        }"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn a_prediction_offering_other_than_three_choices_refuses_the_whole_artifact() {
        let text = r#"{
            "protocolVersion": 2,
            "title": "t",
            "range": { "base": "a", "head": "b", "spelling": "main..HEAD" },
            "stories": [{ "id": "s1", "name": "n", "premise": "p", "steps": [{
                "id": "s1e1", "name": "n", "claim": "c", "why": "w",
                "site": { "file": "f", "side": "new", "kind": "changed", "from": 1, "to": 1, "text": "t" },
                "prediction": { "question": "q", "choices": [
                    { "text": "a", "correct": true, "feedback": "f" },
                    { "text": "b", "correct": false, "feedback": "f" }
                ] }
            }] }]
        }"#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn the_ladder_prefers_origin_head_over_every_other_candidate() {
        assert_eq!(
            default_branch(Some("main"), Some("develop"), Some("trunk"), |_| true),
            Some("main".to_string())
        );
    }

    #[test]
    fn the_ladder_falls_back_to_the_upstream_branch() {
        assert_eq!(
            default_branch(None, Some("develop"), Some("trunk"), |_| true),
            Some("develop".to_string())
        );
    }

    #[test]
    fn the_ladder_falls_back_to_the_configured_default() {
        assert_eq!(
            default_branch(None, None, Some("trunk"), |_| true),
            Some("trunk".to_string())
        );
    }

    #[test]
    fn the_ladder_probes_main_before_master() {
        assert_eq!(
            default_branch(None, None, None, |name| name == "main" || name == "master"),
            Some("main".to_string())
        );
    }

    #[test]
    fn the_ladder_falls_back_to_master_when_main_does_not_exist() {
        assert_eq!(
            default_branch(None, None, None, |name| name == "master"),
            Some("master".to_string())
        );
    }

    #[test]
    fn the_ladder_resolves_nothing_when_no_candidate_exists() {
        assert_eq!(default_branch(None, None, None, |_| false), None);
    }

    #[test]
    fn an_already_authored_range_loads_rather_than_re_authoring() {
        assert_eq!(
            decide(
                false,
                true,
                "main..HEAD".to_string(),
                "out.json".to_string()
            ),
            Resolution::Authored
        );
    }

    #[test]
    fn force_re_authors_even_over_a_range_already_on_disk() {
        assert_eq!(
            decide(true, true, "main..HEAD".to_string(), "out.json".to_string()),
            Resolution::ToAuthor {
                spelling: "main..HEAD".to_string(),
                out: "out.json".to_string(),
            }
        );
    }

    #[test]
    fn a_range_not_yet_authored_is_offered_for_authoring() {
        assert_eq!(
            decide(
                false,
                false,
                "main..HEAD".to_string(),
                "out.json".to_string()
            ),
            Resolution::ToAuthor {
                spelling: "main..HEAD".to_string(),
                out: "out.json".to_string(),
            }
        );
    }

    #[test]
    fn worktree_needs_no_commit_to_resolve() {
        let file = Path::new(".varde/stories/aaaaaaaaaaaa-worktree.json");
        // The base still has to resolve; only "worktree" is exempt.
        assert_eq!(
            range_status(file, |revision| revision == "aaaaaaaaaaaa"),
            RangeStatus::Resolves
        );
    }

    #[test]
    fn a_base_git_cannot_resolve_is_gone() {
        let file = Path::new(".varde/stories/aaaaaaaaaaaa-bbbbbbbbbbbb.json");
        assert_eq!(range_status(file, |_| false), RangeStatus::Gone);
    }

    #[test]
    fn a_filename_varde_did_not_write_is_gone() {
        let file = Path::new(".varde/stories/manifest.json");
        assert_eq!(range_status(file, |_| true), RangeStatus::Gone);
    }

    /// Absolute, under whichever directory is Varde's own: the AI is handed
    /// this path and its working directory is not Varde's to set, so a
    /// relative one names a file somewhere nobody agreed on.
    #[test]
    fn artifact_path_names_a_commit_range() {
        assert_eq!(
            artifact_path(Path::new("/w/.varde"), "aaaaaaaaaaaa", "bbbbbbbbbbbb"),
            "/w/.varde/stories/aaaaaaaaaaaa-bbbbbbbbbbbb.json"
        );
        assert_eq!(
            artifact_path(Path::new("/home/me/.varde/paths/%w-9"), "aaaaaaaaaaaa", "b"),
            "/home/me/.varde/paths/%w-9/stories/aaaaaaaaaaaa-b.json"
        );
    }

    #[test]
    fn artifact_path_names_a_dirty_range_distinctly() {
        assert_eq!(
            artifact_path(Path::new("/w/.varde"), "aaaaaaaaaaaa", "worktree"),
            "/w/.varde/stories/aaaaaaaaaaaa-worktree.json"
        );
    }

    /// Beside the set and named for the same range and nothing else — which is
    /// the whole of how re-authoring replaces the companion file rather than
    /// leaving a stale one to be read against a fresh Story.
    #[test]
    fn the_companion_file_is_named_for_the_range_the_story_set_is() {
        let set = artifact_path(Path::new("/w/.varde"), "aaaaaaaaaaaa", "bbbbbbbbbbbb");
        let companion = context_path(&set);
        assert_eq!(
            companion,
            "/w/.varde/stories/aaaaaaaaaaaa-bbbbbbbbbbbb.context.md"
        );
        assert_eq!(
            companion.strip_suffix(".context.md"),
            set.strip_suffix(".json")
        );
        assert_eq!(
            context_path(&artifact_path(
                Path::new("/w/.varde"),
                "aaaaaaaaaaaa",
                WORKTREE
            )),
            "/w/.varde/stories/aaaaaaaaaaaa-worktree.context.md"
        );
    }

    #[test]
    fn the_companion_file_carries_both_resolved_oids() {
        let text = context_file(
            "aaaaaaaaaaaa1111",
            "bbbbbbbbbbbb2222",
            "main..HEAD",
            &[file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")],
            CONTEXT_CAP,
        );
        assert!(text.contains("aaaaaaaaaaaa1111"));
        assert!(text.contains("bbbbbbbbbbbb2222"));
        assert!(text.contains("main..HEAD"));
    }

    #[test]
    fn the_companion_file_carries_the_ranges_diff() {
        let text = context_file(
            "base",
            "head",
            "main..HEAD",
            &[file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")],
            CONTEXT_CAP,
        );
        assert!(text.contains("keys.rs"), "{text}");
        assert!(text.contains("@@"), "{text}");
        assert!(text.contains("-b"), "{text}");
        assert!(text.contains("+X"), "{text}");
    }

    #[test]
    fn the_companion_file_numbers_the_changed_files_lines() {
        let text = context_file(
            "base",
            "head",
            "main..HEAD",
            &[file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")],
            CONTEXT_CAP,
        );
        assert!(text.contains("2 | X"), "{text}");
        assert!(text.contains("3 | c"), "{text}");
    }

    /// A file the range deleted has no new side to number, so its old side is
    /// what the AI is handed — the lines are still readable there, which is
    /// the same reason a deletion's Site is authored on the old side.
    #[test]
    fn a_deleted_file_is_numbered_on_the_side_that_still_has_lines() {
        let mut gone = file_hunks("keys.rs", "a\nb\n", "");
        gone.new_exists = false;
        let text = context_file("base", "head", "main..HEAD", &[gone], CONTEXT_CAP);
        assert!(text.contains("1 | a"), "{text}");
        assert!(text.contains("old"), "{text}");
    }

    /// The cap is a tuning knob, not a behaviour: what it must never do is
    /// stop short in silence.
    #[test]
    fn the_companion_files_cap_truncates_out_loud() {
        let long = (0..400)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        let files = [file_hunks(
            "keys.rs",
            &long,
            &long.replace("line 7\n", "LINE 7\n"),
        )];
        let whole = context_file("base", "head", "main..HEAD", &files, CONTEXT_CAP);
        let capped = context_file("base", "head", "main..HEAD", &files, 400);
        assert!(whole.len() > 400, "the fixture has to overflow the cap");
        assert!(!whole.contains(TRUNCATION));
        assert!(capped.len() < whole.len());
        assert!(capped.contains(TRUNCATION), "{capped}");
        assert!(capped.ends_with('\n'));
        assert!(whole.starts_with(&capped[..200]));
    }

    /// Text with no line boundary inside the cap still has to hand over what
    /// fits: a cut at zero would leave nothing but the notice.
    #[test]
    fn text_with_no_line_boundary_is_cut_at_a_character_instead() {
        let one_long_line = "x".repeat(4096);
        let capped = truncated(one_long_line, 300);
        assert_eq!(capped, format!("{}{TRUNCATION}", "x".repeat(300)));
    }

    /// Cutting mid-character would write bytes no reader can decode.
    #[test]
    fn a_cut_lands_on_a_character_boundary() {
        let wide = "é".repeat(400);
        let capped = truncated(wide, 301);
        assert_eq!(capped, format!("{}{TRUNCATION}", "é".repeat(150)));
    }

    /// An unchanged file is not the companion file's business: a `context`
    /// Site points at one deliberately, and the prompt says to open it.
    #[test]
    fn the_companion_file_holds_the_changes_only() {
        let text = context_file(
            "base",
            "head",
            "main..HEAD",
            &[
                file_hunks("keys.rs", "a\nb\n", "a\nX\n"),
                file_hunks("mouse.rs", "same\n", "same\n"),
            ],
            CONTEXT_CAP,
        );
        assert!(text.contains("keys.rs"));
        assert!(!text.contains("mouse.rs"), "{text}");
        assert!(text.contains("changes only"), "{text}");
    }

    #[test]
    fn the_authoring_prompt_points_at_the_companion_file() {
        let text = prompt("main..HEAD", "out.json", ".varde/stories/a-b.context.md");
        assert!(text.contains(".varde/stories/a-b.context.md"));
        assert!(text.contains("changes only"));
    }

    /// Without this the AI reads the companion file as the whole world and
    /// confines the Story to changed lines — which is a file review, not a
    /// walkthrough.
    #[test]
    fn the_authoring_prompt_says_a_context_step_means_opening_a_file() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        let sentence = text
            .split("\n\n")
            .find(|paragraph| paragraph.contains("context.md"))
            .expect("the companion file is named somewhere");
        assert!(sentence.contains("`context`"), "{sentence}");
        assert!(sentence.contains("open"), "{sentence}");
    }

    #[test]
    fn the_authoring_prompt_names_the_confirmed_range() {
        assert!(prompt("main..HEAD", "out.json", "context.md").contains("main..HEAD"));
    }

    #[test]
    fn the_authoring_prompt_names_the_output_file() {
        assert!(
            prompt("main..HEAD", ".varde/stories/a-b.json", "context.md")
                .contains(".varde/stories/a-b.json")
        );
    }

    #[test]
    fn the_authoring_prompt_leaves_no_substitution_unfilled() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        assert!(!text.contains("{RANGE}"));
        assert!(!text.contains("{OUT}"));
        assert!(!text.contains("{CONTEXT}"));
    }

    #[test]
    fn the_authoring_prompt_asks_for_a_set_title_and_a_step_name_at_the_bumped_version() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        assert!(text.contains("protocolVersion 2"));
        assert!(text.contains("`title`"));
        assert!(text.contains("`name`"));
    }

    #[test]
    fn the_authoring_prompt_asks_for_a_concrete_subject_not_a_metaphor() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        assert!(text.contains("concrete subject"));
        assert!(text.contains("metaphor"));
        assert!(text.contains("`premise` is where the why goes"));
    }

    /// Every field the parser reads, spelled as the JSON key it arrives as.
    /// Hand-maintained: nothing makes the compiler notice a field added to
    /// [`Artifact`], so adding one here is the step that stops the prompt and
    /// the schema drifting apart.
    const SCHEMA_FIELDS: &[&str] = &[
        "protocolVersion",
        "title",
        "range",
        "base",
        "head",
        "spelling",
        "stories",
        "id",
        "name",
        "premise",
        "steps",
        "claim",
        "why",
        "site",
        "file",
        "side",
        "kind",
        "from",
        "to",
        "text",
        "flow",
        "in",
        "out",
        "values",
        "value",
        "provenance",
        "cite",
        "line",
        "nudge",
        "prediction",
        "question",
        "choices",
        "correct",
        "feedback",
    ];

    /// The JSON block of the prompt's worked example.
    fn worked_example(text: &str) -> String {
        let start = text.find("```json").expect("a fenced worked example");
        let body = &text[start + "```json".len()..];
        let end = body.find("```").expect("a closed fence");
        body[..end].to_string()
    }

    #[test]
    fn the_authoring_prompt_names_every_field_the_schema_reads() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        for field in SCHEMA_FIELDS {
            assert!(
                text.contains(&format!("\"{field}\":")),
                "the prompt never names `{field}`"
            );
        }
    }

    #[test]
    fn the_authoring_prompts_worked_example_is_a_valid_story_set() {
        let artifact = parse(&worked_example(&prompt(
            "main..HEAD",
            "out.json",
            "context.md",
        )))
        .expect("the worked example parses");
        assert_eq!(artifact.protocol_version, 2);
        assert_eq!(artifact.range.spelling, "main..HEAD");
        let step = &artifact.stories[0].steps[0];
        assert!(step.flow.is_some());
        assert!(step.nudge.is_some());
        assert!(step.prediction.is_some());
        assert!(!step.values.is_empty());
        assert!(step.values.iter().any(|value| value.cite.is_some()));
    }

    /// The bounds the prompt is the only place to state: the parser refuses a
    /// prediction of any other size, and the rest are spec decisions no
    /// artifact can be checked against after the fact.
    #[test]
    fn the_authoring_prompt_states_the_bounds_an_author_is_held_to() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        for bound in [
            "optional and rare",
            "at most one or two per story",
            "exactly three choices",
            "Exactly one choice is `correct`",
            "Omit `flow`",
            "Omit `nudge`",
            "Omit `values`",
        ] {
            assert!(text.contains(bound), "the prompt never states `{bound}`");
        }
    }

    /// The transcription is Varde's job now, so the two instructions that
    /// asked for it are gone: extracting the text with a command, and
    /// re-reading every site to confirm it still matches.
    #[test]
    fn the_authoring_prompt_no_longer_asks_for_the_code_to_be_transcribed() {
        let text = prompt("main..HEAD", "out.json", "context.md");
        for gone in [
            "site.text",
            "extract it with a command",
            "re-read every site",
            "verbatim",
        ] {
            assert!(!text.contains(gone), "the prompt still says `{gone}`");
        }
        assert!(text.contains("Do not write the lines themselves"));
    }

    /// The one judgement Varde cannot make, and so the one self-check that
    /// stays with the author.
    #[test]
    fn the_authoring_prompt_still_asks_for_execution_order() {
        assert!(prompt("main..HEAD", "out.json", "context.md")
            .contains("confirm the steps follow execution order rather than file order"));
    }

    #[test]
    fn a_set_still_being_filled_in_is_not_a_spine() {
        let state = State {
            story_set: Set::Filling {
                artifact: artifact_of(vec![(
                    "first",
                    vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
                )]),
                attempt: Some(1),
            },
            ..State::default()
        };
        assert_eq!(view_state(&state), "filling");
        assert!(spine(&state).is_empty());
        assert!(current_step(&state).is_none());
    }

    #[test]
    fn pruning_under_the_limit_keeps_everything() {
        let existing = vec!["a".to_string(), "b".to_string()];
        assert_eq!(prune(&existing, 10), existing);
    }

    #[test]
    fn pruning_over_the_limit_drops_the_oldest_first() {
        let existing: Vec<String> = (0..11).map(|n| n.to_string()).collect();
        let kept = prune(&existing, 10);
        assert_eq!(kept.len(), 10);
        assert_eq!(kept.first(), Some(&"1".to_string()));
        assert!(!kept.contains(&"0".to_string()));
    }

    #[test]
    fn authoring_reports_its_own_view_state() {
        let state = State {
            story_set: Set::Authoring {
                spelling: "main..HEAD".to_string(),
            },
            ..State::default()
        };
        assert_eq!(view_state(&state), "authoring");
        assert!(spine(&state).is_empty());
    }

    #[test]
    fn an_abandoned_authoring_reports_its_own_view_state() {
        let state = State {
            story_set: Set::AuthoringAbandoned,
            ..State::default()
        };
        assert_eq!(view_state(&state), "authoring-abandoned");
    }

    #[test]
    fn a_one_line_change_is_one_hunk_with_both_sides_lines() {
        let old = "a\nb\nc\n";
        let new = "a\nX\nc\n";
        let found = hunks(old.as_bytes(), new.as_bytes());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].old_start, 1);
        assert_eq!(found[0].old_lines, 3);
        assert_eq!(found[0].new_start, 1);
        assert_eq!(found[0].new_lines, 3);
    }

    #[test]
    fn identical_bytes_are_no_hunks_at_all() {
        assert!(hunks(b"same\n", b"same\n").is_empty());
    }

    #[test]
    fn a_file_deleted_entirely_is_one_hunk_with_no_new_lines() {
        let found = hunks(b"gone\n", b"");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].new_lines, 0);
        assert!(found[0].is_deletion());
    }

    fn site(file: &str, side: Side, kind: Kind, from: u32, to: u32) -> Site {
        Site {
            file: file.to_string(),
            side,
            kind,
            from,
            to,
            text: String::new(),
        }
    }

    fn step(id: &str, site: Site) -> Step {
        Step {
            id: id.to_string(),
            name: id.to_string(),
            claim: "claim".to_string(),
            why: "why".to_string(),
            site,
            flow: None,
            values: Vec::new(),
            nudge: None,
            prediction: None,
        }
    }

    /// The artifact out of the same builder the `State` helpers below use, so
    /// the fill's tests and the staleness tests describe the same set.
    fn artifact_of(stories: Vec<(&str, Vec<Step>)>) -> Artifact {
        let Set::Loaded(artifact) = loaded(stories).story_set else {
            unreachable!("the helper loads one")
        };
        artifact
    }

    #[test]
    fn every_file_the_artifact_names_is_asked_after_once() {
        let mut cited = step("2", site("b.rs", Side::Old, Kind::Changed, 3, 4));
        cited.values = vec![Value {
            name: "limit".to_string(),
            value: "10".to_string(),
            provenance: Provenance::Literal,
            cite: Some(Cite {
                file: "d.rs".to_string(),
                line: 7,
            }),
        }];
        let artifact = artifact_of(vec![
            (
                "first",
                vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 2))],
            ),
            (
                "second",
                vec![
                    cited,
                    step("3", site("a.rs", Side::New, Kind::Context, 5, 6)),
                ],
            ),
        ]);
        assert_eq!(story_files(&artifact), ["a.rs", "b.rs", "d.rs"]);
    }

    #[test]
    fn the_fill_lands_each_site_on_the_lines_it_names() {
        let mut artifact = artifact_of(vec![
            (
                "first",
                vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
            ),
            (
                "second",
                vec![step("2", site("b.rs", Side::Old, Kind::Changed, 2, 2))],
            ),
        ]);
        fill(
            &mut artifact,
            &[
                file_hunks(
                    "a.rs",
                    "",
                    "from a
second
",
                ),
                file_hunks(
                    "b.rs",
                    "first
from b
",
                    "",
                ),
            ],
        );
        assert_eq!(artifact.stories[0].steps[0].site.text, "from a");
        assert_eq!(artifact.stories[1].steps[0].site.text, "from b");
    }

    /// The three sets already in `.varde/stories/` transcribed their own text,
    /// and that transcription is the baseline their stale check has always
    /// compared against — overwriting it with the file as it reads now would
    /// report every one of their Steps fresh.
    #[test]
    fn the_fill_leaves_a_text_the_artifact_transcribed_alone() {
        let mut transcribed = site("a.rs", Side::New, Kind::Changed, 1, 1);
        transcribed.text = "what it said then".to_string();
        let mut artifact = artifact_of(vec![("first", vec![step("1", transcribed)])]);
        fill(
            &mut artifact,
            &[file_hunks(
                "a.rs",
                "",
                "what it says now
",
            )],
        );
        assert_eq!(artifact.stories[0].steps[0].site.text, "what it said then");
    }

    #[test]
    fn a_re_read_of_the_loaded_set_is_the_same_set_however_it_was_filled() {
        let arrived = artifact_of(vec![(
            "first",
            vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
        )]);
        let mut loaded = arrived.clone();
        fill(
            &mut loaded,
            &[file_hunks(
                "a.rs",
                "",
                "what the file held
",
            )],
        );
        assert!(same_set(&loaded, &arrived));
    }

    #[test]
    fn a_re_authored_set_is_not_the_set_already_loaded() {
        let loaded = artifact_of(vec![(
            "first",
            vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
        )]);
        let arrived = artifact_of(vec![(
            "first",
            vec![step("1", site("a.rs", Side::New, Kind::Changed, 9, 9))],
        )]);
        assert!(!same_set(&loaded, &arrived));
    }

    fn loaded(stories: Vec<(&str, Vec<Step>)>) -> State {
        loaded_over("b", stories)
    }

    /// A Story over a range whose head is named — `"worktree"` for the
    /// uncommitted one, an oid for a range between two commits.
    fn loaded_over(head: &str, stories: Vec<(&str, Vec<Step>)>) -> State {
        let artifact = Artifact {
            protocol_version: 2,
            title: "Title".to_string(),
            range: Range {
                base: "a".to_string(),
                head: head.to_string(),
                spelling: "main..HEAD".to_string(),
            },
            stories: stories
                .into_iter()
                .enumerate()
                .map(|(index, (name, steps))| Story {
                    id: format!("s{index}"),
                    name: name.to_string(),
                    premise: "premise".to_string(),
                    steps,
                })
                .collect(),
        };
        State {
            story_set: Set::Loaded(artifact),
            ..State::default()
        }
    }

    /// A range with no commit of its own, so its head side and the working
    /// tree are the same file — the ordinary fixture. A committed range whose
    /// tree has since moved sets the two apart itself.
    fn file_hunks(file: &str, old: &str, new: &str) -> FileHunks {
        FileHunks {
            file: file.to_string(),
            hunks: hunks(old.as_bytes(), new.as_bytes()),
            old_exists: true,
            old_text: old.to_string(),
            head_exists: !new.is_empty(),
            head_text: new.to_string(),
            new_exists: !new.is_empty(),
            new_text: new.to_string(),
        }
    }

    /// A file the range added, whose one hunk covers both its lines — the
    /// clean fixture every check below is a single deviation from.
    fn checked(sites: Vec<(&str, Site)>) -> Vec<Problem> {
        let artifact = artifact_of(vec![(
            "first",
            sites.into_iter().map(|(id, site)| step(id, site)).collect(),
        )]);
        problems(&artifact, &[file_hunks("a.rs", "", "one\ntwo\n")])
    }

    #[test]
    fn a_story_set_that_points_where_it_says_has_no_problems() {
        assert!(checked(vec![("1", site("a.rs", Side::New, Kind::Changed, 1, 2))]).is_empty());
    }

    #[test]
    fn a_site_naming_a_file_that_is_not_there_is_a_problem() {
        assert_eq!(
            checked(vec![("1", site("gone.rs", Side::New, Kind::Changed, 1, 1))]),
            [Problem {
                step: "1".to_string(),
                fault: Fault::FileMissing,
            }]
        );
    }

    #[test]
    fn a_site_running_past_the_end_of_its_file_is_a_problem() {
        assert_eq!(
            checked(vec![("1", site("a.rs", Side::New, Kind::Changed, 1, 9))]),
            [Problem {
                step: "1".to_string(),
                fault: Fault::RangeOutOfBounds,
            }]
        );
    }

    #[test]
    fn a_changed_site_that_overlaps_no_hunk_is_a_problem() {
        let artifact = artifact_of(vec![(
            "first",
            vec![step("1", site("b.rs", Side::New, Kind::Changed, 1, 1))],
        )]);
        let unchanged = file_hunks("b.rs", "same\n", "same\n");
        assert_eq!(
            problems(&artifact, &[unchanged]),
            [Problem {
                step: "1".to_string(),
                fault: Fault::ClaimsNoChange,
            }]
        );
    }

    /// Only `changed` Sites are held to coverage: a Story that walks into code
    /// the range never touched is a walkthrough doing its job, and flagging it
    /// would confine every Story to the diff.
    #[test]
    fn a_context_site_that_overlaps_no_hunk_is_not_a_problem() {
        let artifact = artifact_of(vec![(
            "first",
            vec![step("1", site("b.rs", Side::New, Kind::Context, 1, 1))],
        )]);
        let unchanged = file_hunks("b.rs", "same\n", "same\n");
        assert!(problems(&artifact, &[unchanged]).is_empty());
    }

    /// A committed range's Story is judged at the range, never at the working
    /// tree: the reviewer kept typing after the Story was written, and that is
    /// what the stale marker says. Refusing here would throw ten minutes of
    /// authoring away for an edit that has nothing to do with the Story.
    #[test]
    fn a_working_tree_that_moved_under_a_committed_range_is_not_a_problem() {
        let mut moved = file_hunks("a.rs", "", "one\ntwo\n");
        moved.new_text = "one\n".to_string();
        let mut steps = vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 2))];
        steps[0].values = vec![Value {
            name: "limit".to_string(),
            value: "two".to_string(),
            provenance: Provenance::Literal,
            cite: Some(Cite {
                file: "a.rs".to_string(),
                line: 2,
            }),
        }];
        let artifact = artifact_of(vec![("first", steps)]);
        assert!(problems(&artifact, &[moved]).is_empty());
    }

    #[test]
    fn a_cited_value_that_is_not_on_the_line_it_cites_is_a_problem() {
        let cite = |value: &str| Value {
            name: "limit".to_string(),
            value: value.to_string(),
            provenance: Provenance::Literal,
            cite: Some(Cite {
                file: "a.rs".to_string(),
                line: 2,
            }),
        };
        let mut steps = vec![
            step("1", site("a.rs", Side::New, Kind::Changed, 1, 2)),
            step("2", site("a.rs", Side::New, Kind::Changed, 1, 2)),
        ];
        steps[0].values = vec![cite("two")];
        steps[1].values = vec![cite("three")];
        let artifact = artifact_of(vec![("first", steps)]);
        assert_eq!(
            problems(&artifact, &[file_hunks("a.rs", "", "one\ntwo\n")]),
            [Problem {
                step: "2".to_string(),
                fault: Fault::CitationAbsent,
            }]
        );
    }

    /// An `invented` value has nowhere to point by design, and nothing to
    /// check — the citation check is about a citation that is wrong, never
    /// about one that was honestly never made.
    #[test]
    fn a_value_with_no_citation_is_not_checked() {
        let mut steps = vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 2))];
        steps[0].values = vec![Value {
            name: "guess".to_string(),
            value: "nowhere in the file".to_string(),
            provenance: Provenance::Invented,
            cite: None,
        }];
        let artifact = artifact_of(vec![("first", steps)]);
        assert!(problems(&artifact, &[file_hunks("a.rs", "", "one\ntwo\n")]).is_empty());
    }

    #[test]
    fn a_refusal_names_every_failing_step_and_its_fault() {
        let problems = vec![
            Problem {
                step: "s1e1".to_string(),
                fault: Fault::FileMissing,
            },
            Problem {
                step: "s2e3".to_string(),
                fault: Fault::CitationAbsent,
            },
        ];
        let because = refusal(&problems);
        assert!(because.contains("s1e1: file-missing"), "{because}");
        assert!(because.contains("s2e3: citation-absent"), "{because}");
    }

    fn one_failing_step() -> (Artifact, Vec<Problem>) {
        let artifact = artifact_of(vec![(
            "Story",
            vec![
                step("s1", site("a.rs", Side::New, Kind::Changed, 1, 1)),
                step("s2", site("b.rs", Side::New, Kind::Changed, 1, 1)),
            ],
        )]);
        let problems = vec![Problem {
            step: "s2".to_string(),
            fault: Fault::ClaimsNoChange,
        }];
        (artifact, problems)
    }

    /// The whole point of a fix round: four Steps rewritten, not a set
    /// re-authored. A request naming a Step that passed spends the ten minutes
    /// this exists to save.
    #[test]
    fn a_fix_request_names_the_failing_step_and_no_other() {
        let (artifact, problems) = one_failing_step();
        let text = fix_prompt(Path::new("/w/.varde"), &artifact, &problems);
        assert!(text.contains("`s2`: claims-no-change"), "{text}");
        assert!(!text.contains("`s1`"), "{text}");
    }

    /// Every failing Step, not the first one: an AI told about one of four
    /// bad line numbers hands back a set that fails on the other three, and
    /// there is only one round left to spend.
    #[test]
    fn a_fix_request_names_every_failing_step() {
        let (artifact, _) = one_failing_step();
        let problems = vec![
            Problem {
                step: "s1".to_string(),
                fault: Fault::FileMissing,
            },
            Problem {
                step: "s2".to_string(),
                fault: Fault::RangeOutOfBounds,
            },
        ];
        let text = fix_prompt(Path::new("/w/.varde"), &artifact, &problems);
        assert!(text.contains("`s1`: file-missing"), "{text}");
        assert!(text.contains("`s2`: range-out-of-bounds"), "{text}");
    }

    /// The same path, worked out from the range the artifact records — the
    /// `out` the reviewer confirmed is two events behind by now.
    #[test]
    fn a_fix_request_asks_for_the_path_the_artifact_came_from() {
        let (artifact, problems) = one_failing_step();
        let text = fix_prompt(Path::new("/w/.varde"), &artifact, &problems);
        assert!(
            text.contains(&artifact_path(Path::new("/w/.varde"), "a", "b")),
            "{text}"
        );
        assert!(
            text.contains(&context_path(&artifact_path(
                Path::new("/w/.varde"),
                "a",
                "b"
            ))),
            "{text}"
        );
    }

    /// A dirty range is named for the working tree, not for a commit (ADR
    /// 0005), and a fix request has to name that same file.
    #[test]
    fn a_fix_request_for_a_dirty_range_names_the_worktree_set() {
        let mut artifact = artifact_of(vec![(
            "Story",
            vec![step("s1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
        )]);
        artifact.range.head = WORKTREE.to_string();
        let problems = vec![Problem {
            step: "s1".to_string(),
            fault: Fault::FileMissing,
        }];
        assert!(
            fix_prompt(Path::new("/w/.varde"), &artifact, &problems).contains(&artifact_path(
                Path::new("/w/.varde"),
                "a",
                WORKTREE
            )),
            "{artifact:?}"
        );
    }

    /// An oid shorter than the twelve characters a set is named by is a
    /// misbehaving CLI, not a panic.
    #[test]
    fn a_fix_request_for_an_abbreviated_oid_still_names_a_path() {
        let mut artifact = artifact_of(vec![(
            "Story",
            vec![step("s1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
        )]);
        artifact.range.base = "ab".to_string();
        artifact.range.head = "cd".to_string();
        let problems = vec![Problem {
            step: "s1".to_string(),
            fault: Fault::FileMissing,
        }];
        assert!(
            fix_prompt(Path::new("/w/.varde"), &artifact, &problems).contains(&artifact_path(
                Path::new("/w/.varde"),
                "ab",
                "cd"
            )),
            "{artifact:?}"
        );
    }

    #[test]
    fn a_fix_request_leaves_no_substitution_unfilled() {
        let (artifact, problems) = one_failing_step();
        let text = fix_prompt(Path::new("/w/.varde"), &artifact, &problems);
        assert!(!text.contains("{OUT}"));
        assert!(!text.contains("{PROBLEMS}"));
        assert!(!text.contains("{CONTEXT}"));
    }

    /// Every fault has to arrive at the AI as something it can act on: a word
    /// with no explanation behind it is a Step rewritten by guesswork.
    #[test]
    fn every_fault_a_check_can_report_says_what_would_make_it_right() {
        let (artifact, _) = one_failing_step();
        for fault in [
            Fault::FileMissing,
            Fault::RangeOutOfBounds,
            Fault::ClaimsNoChange,
            Fault::CitationAbsent,
        ] {
            let problems = vec![Problem {
                step: "s2".to_string(),
                fault,
            }];
            let text = fix_prompt(Path::new("/w/.varde"), &artifact, &problems);
            assert!(text.contains(fault.complaint()), "{fault:?}");
        }
    }

    /// A reviewer who cannot tell a second round from a slow first one has no
    /// way to know whether waiting longer is reasonable.
    #[test]
    fn a_set_being_fixed_reports_its_own_view_state() {
        let state = State {
            story_set: Set::Fixing {
                artifact: artifact_of(vec![(
                    "Story",
                    vec![step("s1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
                )]),
                attempt: 2,
                problems: vec![Problem {
                    step: "s1".to_string(),
                    fault: Fault::FileMissing,
                }],
            },
            ..State::default()
        };
        assert_eq!(view_state(&state), "fixing");
        assert!(spine(&state).is_empty());
        assert!(current_step(&state).is_none());
    }

    #[test]
    fn a_hunk_no_step_claims_is_the_remainder() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 4, 4))],
        )]);
        state.file_hunks = vec![
            file_hunks("keys.rs", "a\nb\nc\nd\ne\n", "a\nb\nc\nX\ne\n"),
            file_hunks("mouse.rs", "a\nb\nc\n", "a\nX\nc\n"),
        ];
        let found = remainder(&state);
        assert_eq!(found.unclaimed, 1);
        assert_eq!(found.locations, vec!["mouse.rs".to_string()]);
    }

    #[test]
    fn remainder_locations_names_the_same_file_the_count_does() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 4, 4))],
        )]);
        state.file_hunks = vec![
            file_hunks("keys.rs", "a\nb\nc\nd\ne\n", "a\nb\nc\nX\ne\n"),
            file_hunks("mouse.rs", "a\nb\nc\n", "a\nX\nc\n"),
        ];
        let found = remainder_locations(&state);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "mouse.rs");
        assert_eq!((found[0].from, found[0].to), (1, 3));
    }

    #[test]
    fn remainder_locations_excludes_a_pure_deletion() {
        let mut state = loaded(Vec::new());
        state.file_hunks = vec![file_hunks("dead.rs", "gone\n", "")];
        assert!(remainder_locations(&state).is_empty());
    }

    #[test]
    fn current_step_answers_nothing_while_walking_the_remainder() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 4, 4))],
        )]);
        state.walking = Some(Walking::Remainder { index: 0 });
        assert!(current_step(&state).is_none());
    }

    #[test]
    fn a_site_claims_a_hunk_by_overlapping_it_not_by_containing_it() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 1, 2))],
        )]);
        // Whole file falls inside one hunk (context 3 on a 5-line file); the
        // site names only two of its five lines.
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\nd\ne\n", "a\nb\nc\nX\ne\n")];
        assert_eq!(remainder(&state).unclaimed, 0);
    }

    #[test]
    fn a_context_site_claims_nothing() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Context, 4, 4))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\nd\ne\n", "a\nb\nc\nX\ne\n")];
        assert_eq!(remainder(&state).unclaimed, 1);
    }

    #[test]
    fn two_stories_claiming_the_same_hunk_count_it_once() {
        let mut state = loaded(vec![
            (
                "Keys reach the child",
                vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 4, 4))],
            ),
            (
                "Clicks find a pane",
                vec![step("s2", site("keys.rs", Side::New, Kind::Changed, 4, 4))],
            ),
        ]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\nd\ne\n", "a\nb\nc\nX\ne\n")];
        assert_eq!(remainder(&state).unclaimed, 0);
    }

    #[test]
    fn a_range_that_deletes_nothing_has_no_deletions_line() {
        let state = loaded(Vec::new());
        assert_eq!(remainder(&state).unwalked_deletions, None);
    }

    #[test]
    fn a_deletion_no_old_side_step_claims_is_an_unwalked_deletion() {
        let mut state = loaded(Vec::new());
        state.file_hunks = vec![file_hunks("dead.rs", "gone\n", "")];
        assert_eq!(remainder(&state).unwalked_deletions, Some(1));
        // A pure deletion carries no new-side lines, so it never inflates the
        // ordinary unclaimed count too.
        assert_eq!(remainder(&state).unclaimed, 0);
    }

    #[test]
    fn stepping_forward_past_the_last_step_holds_there() {
        assert_eq!(advance(3, 2, Direction::Right), 2);
    }

    #[test]
    fn stepping_back_at_the_first_step_holds_there() {
        assert_eq!(advance(3, 0, Direction::Left), 0);
    }

    #[test]
    fn stepping_forward_advances_by_one() {
        assert_eq!(advance(3, 0, Direction::Right), 1);
    }

    #[test]
    fn an_old_side_step_over_the_deleted_range_walks_it() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("dead.rs", Side::Old, Kind::Changed, 1, 1))],
        )]);
        state.file_hunks = vec![file_hunks("dead.rs", "gone\n", "")];
        assert_eq!(remainder(&state).unwalked_deletions, Some(0));
    }

    fn only_step(state: &State) -> &Step {
        let Set::Loaded(artifact) = &state.story_set else {
            panic!("no story set loaded");
        };
        &artifact.stories[0].steps[0]
    }

    fn site_with_text(file: &str, side: Side, from: u32, to: u32, text: &str) -> Site {
        Site {
            file: file.to_string(),
            side,
            kind: Kind::Changed,
            from,
            to,
            text: text.to_string(),
        }
    }

    #[test]
    fn a_site_naming_a_file_never_diffed_is_fresh() {
        let state = loaded(vec![(
            "Story",
            vec![step(
                "s1",
                site_with_text("keys.rs", Side::New, 2, 2, "anything"),
            )],
        )]);
        assert_eq!(staleness(&state, only_step(&state)), Staleness::Fresh);
    }

    #[test]
    fn a_site_whose_lines_still_match_is_fresh() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::New, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nb\nc\n")];
        assert_eq!(staleness(&state, only_step(&state)), Staleness::Fresh);
    }

    #[test]
    fn a_site_whose_file_no_longer_exists_is_gone() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::New, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![FileHunks {
            file: "keys.rs".to_string(),
            hunks: Vec::new(),
            old_exists: true,
            old_text: "a\nb\nc\n".to_string(),
            head_exists: false,
            head_text: String::new(),
            new_exists: false,
            new_text: String::new(),
        }];
        assert_eq!(staleness(&state, only_step(&state)), Staleness::Gone);
    }

    #[test]
    fn a_site_past_the_files_new_length_is_too_short() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::New, 4, 4, "d"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\nd\n", "a\nb\nc\n")];
        assert_eq!(staleness(&state, only_step(&state)), Staleness::TooShort);
    }

    #[test]
    fn a_site_whose_line_now_reads_something_else_is_changed() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::New, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")];
        assert_eq!(
            staleness(&state, only_step(&state)),
            Staleness::Changed {
                now: "X".to_string()
            }
        );
    }

    #[test]
    fn reindenting_the_site_does_not_mark_it_stale() {
        let mut state = loaded(vec![(
            "Story",
            vec![step(
                "s1",
                site_with_text("keys.rs", Side::New, 2, 2, "    b"),
            )],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\n    b\nc\n", "a\nb\nc\n")];
        assert_eq!(staleness(&state, only_step(&state)), Staleness::Fresh);
    }

    #[test]
    fn an_old_side_site_is_immune_to_the_new_side_moving() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::Old, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")];
        assert_eq!(staleness(&state, only_step(&state)), Staleness::Fresh);
    }

    #[test]
    fn an_old_side_site_goes_stale_once_the_old_text_itself_moves() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::Old, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nX\nc\n", "a\nX\nc\n")];
        assert_eq!(
            staleness(&state, only_step(&state)),
            Staleness::Changed {
                now: "X".to_string()
            }
        );
    }

    #[test]
    fn a_dirty_buffer_overrides_the_polled_new_text() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site_with_text("keys.rs", Side::New, 2, 2, "b"))],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nb\nc\n")];
        let path = state.root.join("keys.rs");
        let mut buffer = crate::editor::Buffer::open("a\nb\nc\n", false, 4);
        buffer.key('j'); // move onto line 2
        buffer.key('x'); // delete the 'b', making the buffer dirty
        state.buffers.insert(path, buffer);
        assert_eq!(
            staleness(&state, only_step(&state)),
            Staleness::Changed { now: String::new() }
        );
    }

    #[test]
    fn a_committed_range_is_an_inventory_of_its_own_two_revisions() {
        let state = loaded(vec![("Story", Vec::new())]);
        assert_eq!(
            inventory(&state.story_set),
            Inventory::Committed {
                base: "a",
                head: "b"
            }
        );
    }

    #[test]
    fn a_range_headed_at_the_worktree_reads_the_working_tree() {
        let state = loaded_over("worktree", vec![("Story", Vec::new())]);
        assert_eq!(inventory(&state.story_set), Inventory::Worktree);
    }

    /// The arriving artifact is asked before it is in a `Set` at all — that
    /// answer is what the read effect carries, and it is why a freshly
    /// authored committed range is checked against its own two commits rather
    /// than against the working tree the poll last diffed.
    #[test]
    fn an_arriving_artifact_names_its_own_two_revisions() {
        let committed = artifact_of(vec![("Story", Vec::new())]);
        assert_eq!(
            inventory_of(&committed),
            Inventory::Committed {
                base: "a",
                head: "b"
            }
        );
        let Set::Loaded(uncommitted) =
            loaded_over("worktree", vec![("Story", Vec::new())]).story_set
        else {
            unreachable!("the helper loads one")
        };
        assert_eq!(inventory_of(&uncommitted), Inventory::Worktree);
    }

    /// A set still filling names its files and its revisions, so the poll that
    /// lands mid-fill describes the arriving range rather than the one before
    /// it.
    #[test]
    fn a_set_still_filling_is_read_as_the_range_it_describes() {
        let set = Set::Filling {
            artifact: artifact_of(vec![(
                "Story",
                vec![step("1", site("a.rs", Side::New, Kind::Changed, 1, 1))],
            )]),
            attempt: Some(1),
        };
        assert_eq!(
            inventory(&set),
            Inventory::Committed {
                base: "a",
                head: "b"
            }
        );
        assert_eq!(named_files(&set), ["a.rs"]);
    }

    #[test]
    fn a_set_that_loaded_nothing_reads_the_working_tree() {
        for set in [
            Set::None,
            Set::Refused {
                because: "why".to_string(),
            },
            Set::RangeGone,
            Set::NoDefaultBranch,
            Set::BadRange,
            Set::NotARepository,
            Set::WorkingTreeDirty,
            Set::GuestNeedsBareWorkspace,
            Set::NoGit,
            Set::Downloading {
                url: "git@github.com:them/theirs.git".to_string(),
                how: Download::Clone,
            },
            Set::DownloadFailed {
                how: Download::Fetch,
                status: Some("128".to_string()),
            },
            Set::Authoring {
                spelling: "main..HEAD".to_string(),
            },
            Set::AuthoringAbandoned,
        ] {
            assert_eq!(inventory(&set), Inventory::Worktree, "{set:?}");
            assert!(named_files(&set).is_empty(), "{set:?}");
        }
    }

    #[test]
    fn the_spine_counts_stale_steps_per_story() {
        let mut state = loaded(vec![(
            "Story",
            vec![
                step("s1", site_with_text("keys.rs", Side::New, 2, 2, "b")),
                step("s2", site_with_text("keys.rs", Side::New, 3, 3, "WRONG")),
            ],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nb\nc\n")];
        assert_eq!(spine(&state)[0].stale, 1);
    }

    /// A Step's own Site, whole — the range the claim was authored against,
    /// not the line the cursor happened to land on.
    #[test]
    fn a_step_marks_its_whole_site_with_its_kind() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 2, 4))],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert_eq!(
            mark(&state),
            SiteMark::Site {
                file: "keys.rs".to_string(),
                from: 2,
                to: 4,
                kind: Kind::Changed,
            }
        );
    }

    /// A Story deliberately walks into unchanged code to follow execution, so
    /// the mark has to say which it is or the reviewer cannot tell what the
    /// change did from what it merely passes through.
    #[test]
    fn a_site_the_story_only_passes_through_is_marked_as_context() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Context, 2, 2))],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert!(matches!(
            mark(&state),
            SiteMark::Site {
                kind: Kind::Context,
                ..
            }
        ));
    }

    /// The Remainder has no claim and no authored `kind`, but every hunk in it
    /// is by definition part of the change nobody narrated.
    #[test]
    fn the_remainder_marks_its_location_as_changed_code() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 2, 2))],
        )]);
        state.file_hunks = vec![file_hunks("mouse.rs", "a\nb\nc\n", "a\nX\nc\n")];
        state.walking = Some(Walking::Remainder { index: 0 });
        assert!(matches!(
            mark(&state),
            SiteMark::Site {
                kind: Kind::Changed,
                ..
            }
        ));
    }

    fn named_step(id: &str, name: &str, site: Site) -> Step {
        Step {
            name: name.to_string(),
            ..step(id, site)
        }
    }

    #[test]
    fn the_step_menu_is_empty_while_not_walking() {
        let state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 2, 2))],
        )]);
        assert!(step_menu(&state).is_empty());
    }

    #[test]
    fn the_step_menu_is_empty_while_walking_the_remainder() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 2, 2))],
        )]);
        state.file_hunks = vec![file_hunks("mouse.rs", "a\nb\nc\n", "a\nX\nc\n")];
        state.walking = Some(Walking::Remainder { index: 0 });
        assert!(step_menu(&state).is_empty());
    }

    #[test]
    fn the_step_menu_lists_every_step_of_the_current_story_in_order() {
        let mut state = loaded(vec![(
            "Story",
            vec![
                named_step(
                    "s1",
                    "First",
                    site("keys.rs", Side::New, Kind::Changed, 1, 1),
                ),
                named_step(
                    "s2",
                    "Second",
                    site("keys.rs", Side::New, Kind::Changed, 2, 2),
                ),
                named_step(
                    "s3",
                    "Third",
                    site("keys.rs", Side::New, Kind::Changed, 3, 3),
                ),
            ],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert_eq!(
            step_menu(&state),
            vec![
                StepMenuRow {
                    name: "First".to_string(),
                    current: true
                },
                StepMenuRow {
                    name: "Second".to_string(),
                    current: false
                },
                StepMenuRow {
                    name: "Third".to_string(),
                    current: false
                },
            ]
        );
    }

    #[test]
    fn the_step_menu_marks_a_middle_step_current() {
        let mut state = loaded(vec![(
            "Story",
            vec![
                named_step(
                    "s1",
                    "First",
                    site("keys.rs", Side::New, Kind::Changed, 1, 1),
                ),
                named_step(
                    "s2",
                    "Second",
                    site("keys.rs", Side::New, Kind::Changed, 2, 2),
                ),
                named_step(
                    "s3",
                    "Third",
                    site("keys.rs", Side::New, Kind::Changed, 3, 3),
                ),
            ],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 1,
            diff: Diff::Hidden,
        });
        let current: Vec<String> = step_menu(&state)
            .into_iter()
            .filter(|row| row.current)
            .map(|row| row.name)
            .collect();
        assert_eq!(current, vec!["Second".to_string()]);
    }

    #[test]
    fn the_step_menu_marks_the_last_step_current() {
        let mut state = loaded(vec![(
            "Story",
            vec![
                named_step(
                    "s1",
                    "First",
                    site("keys.rs", Side::New, Kind::Changed, 1, 1),
                ),
                named_step(
                    "s2",
                    "Second",
                    site("keys.rs", Side::New, Kind::Changed, 2, 2),
                ),
                named_step(
                    "s3",
                    "Third",
                    site("keys.rs", Side::New, Kind::Changed, 3, 3),
                ),
            ],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 2,
            diff: Diff::Hidden,
        });
        let current: Vec<String> = step_menu(&state)
            .into_iter()
            .filter(|row| row.current)
            .map(|row| row.name)
            .collect();
        assert_eq!(current, vec!["Third".to_string()]);
    }

    /// Never re-anchored and never clamped (ADR 0005): the mark is the range
    /// the Step was authored against, and a file too short to hold it is a
    /// staleness the band reports rather than a range the mark quietly shrinks
    /// to fit.
    #[test]
    fn a_site_running_past_the_end_of_the_file_is_marked_whole() {
        let mut state = loaded(vec![(
            "Story",
            vec![step(
                "s1",
                site("keys.rs", Side::New, Kind::Changed, 2, 900),
            )],
        )]);
        state.file_hunks = vec![file_hunks("keys.rs", "a\nb\nc\n", "a\nX\nc\n")];
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert!(matches!(mark(&state), SiteMark::Site { to: 900, .. }));
    }

    /// `OpenAt` reads the working tree, so an old-side Site shows new text at
    /// old line numbers. Refusing is a third answer on purpose: folded into
    /// "nothing to draw" it would read as an ordinary unmarked file.
    #[test]
    fn an_old_side_site_refuses_rather_than_marking() {
        let mut state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::Old, Kind::Changed, 2, 2))],
        )]);
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert_eq!(mark(&state), SiteMark::Refused);
    }

    #[test]
    fn nothing_is_marked_when_no_story_is_being_walked() {
        let state = loaded(vec![(
            "Story",
            vec![step("s1", site("keys.rs", Side::New, Kind::Changed, 2, 2))],
        )]);
        assert_eq!(mark(&state), SiteMark::Nothing);
    }

    /// What the renderer dims is what this answers `false` for — including
    /// every line of a file the mark does not name at all.
    #[test]
    fn the_mark_covers_its_range_and_nothing_else() {
        let marked = SiteMark::Site {
            file: "keys.rs".to_string(),
            from: 2,
            to: 4,
            kind: Kind::Changed,
        };
        assert!(marked.covers("keys.rs", 2));
        assert!(marked.covers("keys.rs", 4));
        assert!(!marked.covers("keys.rs", 1));
        assert!(!marked.covers("keys.rs", 5));
        assert!(!marked.covers("mouse.rs", 3));
    }

    #[test]
    fn a_refusal_covers_nothing() {
        assert!(!SiteMark::Refused.covers("keys.rs", 1));
    }

    fn commented_on(file: &str, line: u32, body: &str) -> crate::review::Comment {
        crate::review::Comment {
            file: file.to_string(),
            from_line: line,
            to_line: line,
            kind: "ISSUE".to_string(),
            body: body.to_string(),
            revision: "abc".to_string(),
            story: Some("Story".to_string()),
            step: Some(1),
        }
    }

    /// Walking, because Story view is the surface that draws comment rows.
    fn showing(file: &str, comments: Vec<crate::review::Comment>) -> State {
        State {
            root: std::path::PathBuf::from("/work"),
            current_buffer: Some(std::path::PathBuf::from("/work").join(file)),
            comments,
            walking: Some(Walking::Story {
                story: 0,
                step: 0,
                diff: Diff::Hidden,
            }),
            ..State::default()
        }
    }

    #[test]
    fn a_comment_gets_a_row_under_the_line_it_covers() {
        let state = showing(
            "src/keys.rs",
            vec![commented_on("src/keys.rs", 2, "the catch-all")],
        );
        let rows = rows(&state, 3);
        assert_eq!(rows[0], Row::Code(1));
        assert_eq!(rows[1], Row::Code(2));
        assert!(matches!(rows[2], Row::Comment(comment) if comment.body == "the catch-all"));
        assert_eq!(rows[3], Row::Code(3));
    }

    /// The whole point of carrying the number: line 3 is still line 3 with a
    /// comment row above it. Read positionally it would be row 3, and the mark
    /// would bar the line above the one it names.
    #[test]
    fn a_comment_row_does_not_renumber_the_lines_below_it() {
        let state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 1, "first")]);
        assert_eq!(
            rows(&state, 3)
                .into_iter()
                .filter_map(|row| match row {
                    Row::Code(number) => Some(number),
                    Row::Comment(_) | Row::Removed(_) => None,
                })
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    /// Comments anchor to a file, and Story view shows one. A comment on
    /// another file drawn here would sit under a line it says nothing about.
    #[test]
    fn a_comment_on_another_file_gets_no_row() {
        let state = showing(
            "src/keys.rs",
            vec![commented_on("src/mouse.rs", 2, "elsewhere")],
        );
        assert_eq!(rows(&state, 2), vec![Row::Code(1), Row::Code(2)]);
    }

    #[test]
    fn every_comment_on_one_line_gets_its_own_row() {
        let state = showing(
            "src/keys.rs",
            vec![
                commented_on("src/keys.rs", 2, "first"),
                commented_on("src/keys.rs", 2, "second"),
            ],
        );
        assert_eq!(rows(&state, 2).len(), 4);
    }

    /// The lookup anchors to the workspace-relative name the Site is authored
    /// with; the buffer is absolute. Comparing the absolute path would find
    /// nothing and the comment would silently never appear.
    /// Nothing walked means no story surface, so a comment on the open file must
    /// move neither a line nor the caret and click that count rows to find it.
    #[test]
    fn a_file_being_edited_rather_than_walked_gets_no_comment_rows() {
        let mut state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 1, "first")]);
        state.walking = None;
        assert_eq!(rows(&state, 2), vec![Row::Code(1), Row::Code(2)]);
        assert_eq!(row_of(&state, 2), 2);
        assert_eq!(line_at_row(&state, 2), 2);
    }

    #[test]
    fn a_line_with_no_comment_above_it_is_drawn_on_its_own_row() {
        let state = showing("src/keys.rs", Vec::new());
        assert_eq!(row_of(&state, 3), 3);
    }

    /// Where the caret would go wrong: line 3 is drawn a row lower than it is
    /// numbered once a comment sits under line 1.
    #[test]
    fn a_comment_above_a_line_pushes_it_down_a_row() {
        let state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 1, "first")]);
        assert_eq!(row_of(&state, 3), 4);
    }

    /// A comment on the line itself sits *under* it, so it moves nothing.
    #[test]
    fn a_comment_on_the_line_itself_does_not_move_it() {
        let state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 3, "here")]);
        assert_eq!(row_of(&state, 3), 3);
    }

    /// A click has to land on the line the pointer is over. Without the inverse,
    /// row 4 named line 4 while `row_of` drew line 3 there.
    #[test]
    fn a_row_below_a_comment_names_the_line_actually_drawn_on_it() {
        let state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 1, "first")]);
        assert_eq!(line_at_row(&state, 1), 1);
        assert_eq!(line_at_row(&state, 3), 2);
        assert_eq!(line_at_row(&state, 4), 3);
    }

    /// Clicking the comment itself means the line it argues with.
    #[test]
    fn a_comment_row_names_the_line_it_sits_under() {
        let state = showing("src/keys.rs", vec![commented_on("src/keys.rs", 2, "here")]);
        assert_eq!(line_at_row(&state, 3), 2);
    }

    /// The two directions must compose, or a click and the caret disagree about
    /// the same line.
    #[test]
    fn a_line_survives_the_trip_through_a_row_and_back() {
        let state = showing(
            "src/keys.rs",
            vec![
                commented_on("src/keys.rs", 1, "first"),
                commented_on("src/keys.rs", 3, "third"),
            ],
        );
        for line in 1..=6u32 {
            assert_eq!(line_at_row(&state, row_of(&state, line)), line as usize);
        }
    }

    const BASE: &str = "a\nb\nc\nd\ne\nf\n";

    /// Walking a Site of `src/keys.rs` with `d` showing the diff from [`BASE`]
    /// to `head`, and the buffer open on `head`.
    fn diff_shown(head: &str, side: Side, from: u32, to: u32) -> State {
        let path = std::path::PathBuf::from("/work/src/keys.rs");
        let mut state = State {
            root: std::path::PathBuf::from("/work"),
            current_buffer: Some(path.clone()),
            file_hunks: vec![file_hunks("src/keys.rs", BASE, head)],
            ..loaded(vec![(
                "Story",
                vec![step(
                    "s1",
                    site("src/keys.rs", side, Kind::Changed, from, to),
                )],
            )])
        };
        state
            .buffers
            .insert(path, crate::editor::Buffer::open(head, false, 4));
        state.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Shown,
        });
        state
    }

    /// An edit that straddles the Site's edge: the addition inside it is marked
    /// and the one outside is not, because code outside a Site is drawn as it
    /// always is — while the removal is the whole edit's, since half an edit
    /// shows a line replaced by nothing.
    #[test]
    fn a_new_side_site_marks_only_the_additions_inside_it() {
        assert_eq!(
            changes(BASE, "a\nB\nC\nd\ne\nf\n", Side::New, 3, 5),
            SiteDiff {
                added: vec![3],
                removed: vec![(1, "b".to_string()), (1, "c".to_string())],
            }
        );
    }

    #[test]
    fn a_deletion_between_a_sites_lines_is_shown_and_one_beside_it_is_not() {
        let head = "a\nb\nd\ne\nf\n";
        assert_eq!(
            changes(BASE, head, Side::New, 1, 3).removed,
            vec![(2, "c".to_string())]
        );
        assert_eq!(changes(BASE, head, Side::New, 3, 5).removed, vec![]);
    }

    /// Removed at the top of the file: there is no line above to sit under.
    #[test]
    fn a_deletion_of_the_first_line_sits_above_the_first_row() {
        assert_eq!(
            changes(BASE, "b\nc\nd\ne\nf\n", Side::Old, 1, 1).removed,
            vec![(0, "a".to_string())]
        );
    }

    /// An old-side Site names removed lines, so it takes what replaced them as
    /// well — and nothing from an edit that removed none of its lines.
    #[test]
    fn an_old_side_site_takes_the_edit_that_removed_its_lines_whole() {
        assert_eq!(
            changes(BASE, "X\nb\nc\nd\nY\nf\n", Side::Old, 5, 5),
            SiteDiff {
                added: vec![5],
                removed: vec![(4, "e".to_string())],
            }
        );
    }

    #[test]
    fn a_hidden_diff_and_a_context_site_draw_nothing() {
        let mut hidden = diff_shown("a\nB\nc\nd\ne\nf\n", Side::New, 1, 3);
        hidden.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Hidden,
        });
        assert_eq!(site_diff(&hidden), None);
        let Set::Loaded(artifact) = &mut hidden.story_set else {
            unreachable!("the helper loads one")
        };
        artifact.stories[0].steps[0].site.kind = Kind::Context;
        hidden.walking = Some(Walking::Story {
            story: 0,
            step: 0,
            diff: Diff::Shown,
        });
        assert_eq!(site_diff(&hidden), None);
    }

    #[test]
    fn a_removed_row_sits_under_the_line_before_it_and_pushes_the_rest_down() {
        let state = diff_shown("a\nB\nc\nd\ne\nf\n", Side::New, 1, 3);
        assert_eq!(
            rows(&state, 3),
            vec![
                Row::Code(1),
                Row::Removed("b".to_string()),
                Row::Code(2),
                Row::Code(3),
            ]
        );
        assert_eq!(row_of(&state, 2), 3);
        assert_eq!(row_of(&state, 3), 4);
    }

    /// A click on a removed row has no buffer line of its own to land on, so it
    /// lands where a click on a comment does: on the line the row sits under.
    /// The rows below it still name the lines drawn on them.
    #[test]
    fn a_click_on_a_removed_row_lands_on_the_line_it_sits_under() {
        let state = diff_shown("a\nB\nc\nd\ne\nf\n", Side::New, 1, 3);
        assert_eq!(line_at_row(&state, 2), 1);
        assert_eq!(line_at_row(&state, 3), 2);
        assert_eq!(line_at_row(&state, 7), 6);
        for line in 1..=6u32 {
            assert_eq!(line_at_row(&state, row_of(&state, line)), line as usize);
        }
    }

    /// The scroll clamp bounds rows, so the last line is reachable with removed
    /// rows above it — and an old-side Site under `d` is code to scroll rather
    /// than a notice with nothing to scroll.
    #[test]
    fn the_scroll_clamp_counts_removed_rows() {
        let state = diff_shown("a\nB\nC\nd\ne\nf", Side::New, 1, 3);
        assert_eq!(crate::editor_focus(&state, &[]).1, 8);
        let old = diff_shown("a\nB\nC\nd\ne\nf", Side::Old, 2, 3);
        assert!(!refused(&old));
        assert_eq!(crate::editor_focus(&old, &[]).1, 8);
    }

    #[test]
    fn the_file_on_screen_is_named_relative_to_the_repository_under_review() {
        assert_eq!(
            shown_file(&showing("src/keys.rs", Vec::new())),
            "src/keys.rs"
        );
        let guest = std::path::PathBuf::from("/work/.varde/guest/theirs");
        let state = State {
            current_buffer: Some(guest.join("src/theirs.rs")),
            guest: Some(guest),
            ..showing("src/keys.rs", Vec::new())
        };
        assert_eq!(shown_file(&state), "src/theirs.rs");
    }

    /// Every spelling of a repository URL a reviewer pastes, and the two that
    /// could name a directory outside the Sidecar.
    #[test]
    fn a_guest_repo_is_named_for_the_repository() {
        for (url, expected) in [
            ("git@github.com:them/theirs.git", "theirs"),
            ("https://github.com/them/theirs.git", "theirs"),
            ("https://github.com/them/theirs", "theirs"),
            ("ssh://git@host:2222/them/theirs.git/", "theirs"),
            ("/home/me/projects/theirs", "theirs"),
            ("https://host/them/..", "guest"),
            ("https://host/them/", "them"),
            ("", "guest"),
        ] {
            assert_eq!(guest_name(url), expected, "for {url}");
        }
    }

    /// The status, not the fact of finishing: `&& touch` writes nothing when
    /// the clone fails, which is the wait that never ends.
    #[test]
    fn a_clone_records_the_exit_status_of_the_clone() {
        let command = download_command(
            Download::Clone,
            "git@github.com:them/theirs.git",
            Path::new("/home/me/.varde/paths/x-1/theirs"),
            Path::new("/home/me/.varde/paths/x-1/clone-done"),
        );
        assert_eq!(
            command,
            "rm -f /home/me/.varde/paths/x-1/clone-done; \
             git clone git@github.com:them/theirs.git /home/me/.varde/paths/x-1/theirs; \
             echo $? > /home/me/.varde/paths/x-1/clone-done.writing; \
             mv /home/me/.varde/paths/x-1/clone-done.writing \
             /home/me/.varde/paths/x-1/clone-done"
        );
    }

    /// The second `:story?` on a URL already downloaded: `origin` rather than
    /// the URL, because a fetch given a URL updates no remote-tracking ref and
    /// would list the branches the clone saw and nothing since.
    #[test]
    fn a_fetch_updates_the_guest_repos_refs_and_records_its_status() {
        let command = download_command(
            Download::Fetch,
            "git@github.com:them/theirs.git",
            Path::new("/home/me/.varde/paths/x-1/theirs"),
            Path::new("/home/me/.varde/paths/x-1/download-done"),
        );
        assert_eq!(
            command,
            "rm -f /home/me/.varde/paths/x-1/download-done; \
             git -C /home/me/.varde/paths/x-1/theirs fetch; \
             echo $? > /home/me/.varde/paths/x-1/download-done.writing; \
             mv /home/me/.varde/paths/x-1/download-done.writing \
             /home/me/.varde/paths/x-1/download-done"
        );
    }

    /// A URL is untrusted input, and it reaches a shell.
    #[test]
    fn a_url_that_is_shell_syntax_is_quoted() {
        let command = download_command(
            Download::Clone,
            "https://host/x; rm -rf ~",
            Path::new("/tmp/guest"),
            Path::new("/tmp/done"),
        );
        assert!(
            command.contains("'https://host/x; rm -rf ~'"),
            "unquoted: {command}"
        );
    }
}
