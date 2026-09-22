//! Markdown as rows.
//!
//! A Preview row is not a source line: markdown reflows, so one line becomes
//! several rows or none, and every row carries the source line of the block it
//! came from. The argument, and the four consumers that used to re-derive
//! `row = line + editor_scroll` on their own, are in
//! `docs/adr/0007-a-preview-row-is-not-a-line.md`.
//!
//! Nothing here draws. A glyph and a colour are a theme's business, exactly as
//! they are in [`crate::highlight`] — this module answers what each row *is*
//! and where it came from, and `ui` decides what that looks like.
//!
//! Nothing renders as nothing (R26.5a), and R26.5b is the mechanism that keeps
//! that checkable: [`block_kind`] classifies every parser tag with no `_ =>`
//! arm, [`FLAGS`] records a decision per parser flag, and the sweep beside the
//! tests drives one sample of every construct with an explicit `UNRENDERED`
//! list. Four gaps in this feature were found by eye in a Preview of this
//! repo's own documents, two of them owned by no ticket at all; that is what
//! the three of them together exist to prevent.

use crate::highlight;
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use std::collections::HashMap;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

/// What a rendered row is, so `ui` can style it and a scenario can assert it
/// without naming a character. `Heading` carries the level a `#` run named and
/// `Quote` the GFM alert kind a `[!NOTE]`-style marker named (`None` for a
/// plain quote), so a scenario can assert what kind of row came back without
/// `ui` reaching back into the source to find out. `Code`, `Diagram`,
/// `Metadata` and `Table` are named here so that the tickets which add them
/// are a match arm rather than a change to this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Heading(HeadingLevel),
    Paragraph,
    Code,
    Diagram,
    Metadata,
    Quote(Option<BlockQuoteKind>),
    List(ListItem),
    Table,
    Rule,
}

/// A bulleted, ordered or task marker, mutually exclusive by construction —
/// unlike [`Emphasis`], a list item is never two of these at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Bullet,
    Ordinal(u64),
    Task(bool),
}

/// A list row's nesting and marker. `marker` is `Some` only on the first row
/// an item takes — a wrapped continuation row carries the same `depth` so it
/// still indents under the item, but `None` so `ui` does not repeat the
/// bullet, ordinal or checkbox down the wrapped lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListItem {
    pub depth: usize,
    pub marker: Option<Marker>,
}

/// A run of text's modifiers. Independent and combinable — `*_**both**_*` is
/// `italic` and `strong` both true, not a fifth variant — which is why this is
/// a struct of flags rather than an enum: an enum would need one arm per
/// combination that markdown can nest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Emphasis {
    pub italic: bool,
    pub strong: bool,
    pub struck: bool,
    pub code: bool,
    /// Set the same way `code` is — an image's alt text is prose carrying a
    /// modifier, not a row of its own, since there is no picture to draw.
    /// `ui` picks whatever glyph or colour marks it; no scenario asserts
    /// either.
    pub image: bool,
}

/// One styled run inside a row. Names what it *is*, never what colour it is,
/// exactly as [`crate::highlight::Token`] does — `ui` maps `emphasis` to a
/// weight and a colour, and no scenario asserts either. `token` is a code
/// row's piece of that same distinction — `Some` only inside [`RowKind::Code`],
/// where a piece is a highlighted token rather than a run of prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub emphasis: Emphasis,
    pub token: Option<highlight::Kind>,
}

/// One row of a Preview: what it is, the source line of the block it came
/// from, and the styled pieces it draws. `line` is 1-based, like
/// [`crate::editor::Buffer`]'s. A single string could not carry a difference
/// *inside* a row — emphasis in the middle of a sentence, or a heading's own
/// level — which is why a row is pieces rather than one `text: String`; see
/// `docs/adr/0007-a-preview-row-is-not-a-line.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub line: usize,
    pub pieces: Vec<Piece>,
    /// `Some` only on the one [`RowKind::Code`] row a mermaid fence's fallback
    /// produces — never on a fence that was never mermaid, and never on a
    /// [`RowKind::Diagram`] row, which only exists because rendering
    /// succeeded. A reason rather than the crate's own wording, for the
    /// reason [`Refusal`] is: `mermaid-text` can reword an error message
    /// without turning this suite red.
    pub refused: Option<DiagramRefusal>,
}

impl Row {
    /// The row's text with no styling, for a caller that only wants to know
    /// what is on screen — search, drag-copy, a scenario checking a marker is
    /// gone. Concatenating `pieces` rather than each caller doing it keeps the
    /// join in one place.
    pub fn text(&self) -> String {
        self.pieces
            .iter()
            .map(|piece| piece.text.as_str())
            .collect()
    }
}

/// Why a key did nothing — `:preview` refusing to switch, or an editing key
/// refusing to touch a Preview. A reason rather than a sentence: `update`
/// records this and `ui` renders the wording, so a scenario asserts the
/// decision and a reworded footer never turns the suite red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    NotMarkdown,
    NoFileOpen,
    ReadOnlyPreview,
    /// An edit to a buffer on a Guest repo's file. The clone goes with the
    /// Sidecar when Varde exits, so an edit there is work nobody can keep.
    GuestReadOnly,
    /// The install key on a row whose command this machine already has. Said
    /// out loud rather than passed over: the row reads `installed`, so a key
    /// that quietly did nothing would read as a key that failed.
    ToolAlreadyInstalled,
    /// A Tools row taken while the global config is one Varde would refuse
    /// to start on. Nothing is written and nothing runs, and the fault is
    /// named, since the reader has to fix the file before anything is taken.
    BrokenConfig(crate::startup::ConfigError),
    /// A Tools row taken whose install starts with a program this machine
    /// lacks. Nothing is written and nothing runs; the program is named.
    NeedsInstaller(String),
    /// A Launch configuration started while a Debug session exists: there is
    /// one session, and a second would be the session picker the spec declines.
    SessionRunning,
    /// No command runs the adapter a Launch configuration names — no
    /// `[dap.*]` row, or a row whose command is not on this machine. Named.
    NoDebugAdapter(String),
    DebugAdapterFailed,
    /// The adapter went away with a session still going.
    DebugAdapterExited,
    /// The adapter said no to starting the program, in its own words.
    LaunchFailed(String),
}

impl Refusal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Refusal::NotMarkdown => "not-a-markdown-file",
            Refusal::NoFileOpen => "no-file-open",
            Refusal::ReadOnlyPreview => "read-only-preview",
            Refusal::GuestReadOnly => "guest-read-only",
            Refusal::ToolAlreadyInstalled => "tool-already-installed",
            Refusal::BrokenConfig(_) => "broken-config",
            Refusal::NeedsInstaller(_) => "needs-installer",
            Refusal::SessionRunning => "debug-session-running",
            Refusal::NoDebugAdapter(_) => "no-debug-adapter",
            Refusal::DebugAdapterFailed => "debug-adapter-failed",
            Refusal::DebugAdapterExited => "debug-adapter-exited",
            Refusal::LaunchFailed(_) => "launch-failed",
        }
    }
}

/// Why a mermaid fence's source did not become a diagram — ticket 07's own
/// `Refusal`. `mermaid_text::Error` names the same three shapes with its own
/// wording, which is the crate's to change; this is the word Varde promises
/// to keep saying regardless. `EmptyInput` folds into `MalformedDiagram`: an
/// empty fence is not a distinct failure mode either ticket names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramRefusal {
    UnsupportedDiagram,
    MalformedDiagram,
    TooWide,
}

impl DiagramRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagramRefusal::UnsupportedDiagram => "unsupported-diagram",
            DiagramRefusal::MalformedDiagram => "malformed-diagram",
            DiagramRefusal::TooWide => "diagram-too-wide",
        }
    }
}

impl From<&mermaid_text::Error> for DiagramRefusal {
    fn from(error: &mermaid_text::Error) -> Self {
        match error {
            mermaid_text::Error::UnsupportedDiagram(_) => DiagramRefusal::UnsupportedDiagram,
            mermaid_text::Error::EmptyInput | mermaid_text::Error::ParseError(_) => {
                DiagramRefusal::MalformedDiagram
            }
            mermaid_text::Error::TooWide { .. } => DiagramRefusal::TooWide,
        }
    }
}

/// Whether a path is a file this can render. `.mdx` is deliberately absent: a
/// CommonMark parser renders its components as garbage text, which is worse
/// than reading the source.
pub fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ["md", "markdown"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

/// Every parser flag, with the decision and the reason for it. A flag left off
/// does not lose its construct: the parser hands back the characters the author
/// typed, which for a construct a cell cannot draw — a superscript, an equation
/// — is the *better* terminal rendering. A flag turned on is a promise to
/// render the construct as something, and the sweep holds it to that promise,
/// so the wrong move is enabling one and having nowhere to put what comes back.
///
/// Held to covering `Options::all()` by a test rather than by the compiler:
/// `Options` is a bitflags struct and cannot be matched exhaustively, so a flag
/// `pulldown-cmark` grows fails the suite instead of quietly defaulting to off.
/// The `bool` column is held to a test of its own —
/// [`tests::strikethrough_tasklists_gfm_tables_and_yaml_metadata_are_the_only_flags_on`]
/// pins the fold this feeds `rows()`, so flipping one without wiring its
/// construct's layout fails the suite rather than silently changing what
/// `pulldown-cmark` parses.
const FLAGS: [(Options, bool, &str); 15] = [
    (
        Options::ENABLE_TABLES,
        true,
        "ticket 04: a cell's pieces now have somewhere to land — a table row \
         laid out with columns aligned to the widest cell and each column's \
         declared alignment honoured — rather than the pipes and dashes the \
         parser hands back when this flag is off",
    ),
    (
        Options::ENABLE_FOOTNOTES,
        true,
        "ticket 13: a reference now has somewhere to become `[1]` rather than \
         staying the literal `[^1]`, and a definition's own frame carries the \
         same number so the two read as one construct rather than two \
         unrelated paragraphs",
    ),
    (
        Options::ENABLE_OLD_FOOTNOTES,
        false,
        "the pre-GFM footnote syntax. `ENABLE_FOOTNOTES` already renders the \
         construct; this is a second spelling of the same thing and turning \
         it on too would only risk a document tripping both parsers at once",
    ),
    (
        Options::ENABLE_STRIKETHROUGH,
        true,
        "ticket 12: now that a row carries pieces rather than one string, a \
         struck span has somewhere to put its modifier, so `~~struck~~` \
         renders struck instead of showing its tildes. `pulldown-cmark` \
         strikes a *single* pair of tildes too, so `H~2~O` arrives as the \
         three spans `H`, `2`, `O` and would render `H2O` — the exact example \
         ticket 13 uses to argue a subscript is better left as the author's \
         characters, and why subscript's own flag stays off rather than this \
         one turning back off",
    ),
    (
        Options::ENABLE_TASKLISTS,
        true,
        "ticket 03: a list item's row now carries a marker, so a task item's \
         checked state has somewhere to go — `Event::TaskListMarker` overrides \
         the item's `Marker::Bullet` with `Marker::Task(checked)` instead of \
         leaving `[ ]` and `[x]` as characters",
    ),
    (
        Options::ENABLE_SMART_PUNCTUATION,
        false,
        "it rewrites the author's characters — `--` becomes an em dash, quotes \
         become curly ones. A Preview that changes the text lies about the \
         file, and `/` searching rendered rows would stop finding what the \
         source holds",
    ),
    (
        Options::ENABLE_HEADING_ATTRIBUTES,
        true,
        "ticket 13: `block_kind`'s `Tag::Heading { level, .. }` arm already \
         ignores `id`, `classes` and `attrs` to classify a heading by its \
         level alone, so turning this on consumes `{#id}` into a field \
         nothing reads instead of leaving it as prose at the end of the \
         heading's text — no render code needed, since there is nowhere \
         left for the attribute block to leak into",
    ),
    (
        Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
        true,
        "ticket 06: a metadata block now has a layout — RowKind::Metadata's \
         own frame in rows() — so the block renders set apart from prose \
         instead of the closing `---` becoming a stray thematic break",
    ),
    (
        Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
        true,
        "ticket 13: `Tag::MetadataBlock(_)` already matches either style, so \
         `+++` frontmatter reuses the exact frame `RowKind::Metadata` gives \
         YAML's — no new arm, since the block only cares that it is a \
         metadata block, never which delimiter opened it",
    ),
    (
        Options::ENABLE_MATH,
        false,
        "ticket 13's decision, and it is to leave this off for good: a cell \
         cannot set an equation, so `$x^2$` reads better as itself than as the \
         `x2` that collecting the event's contents would produce",
    ),
    (
        Options::ENABLE_GFM,
        true,
        "ticket 03: `RowKind::Quote` now carries the `BlockQuoteKind` this \
         flag parses, so `> [!NOTE]` renders set apart as the note it \
         announces rather than as a quote whose first line says NOTE — the \
         parser consumes the `[!NOTE]` marker itself once the flag is on",
    ),
    (
        Options::ENABLE_DEFINITION_LIST,
        true,
        "ticket 13: a title and its definition now have somewhere to land — \
         `RowKind::List` frames at depth 0 and 1, the same shape nesting \
         already gives an indented list item — rather than the `term` and \
         `: meaning` the parser hands back as two bare prose lines when this \
         is off",
    ),
    (
        Options::ENABLE_SUPERSCRIPT,
        false,
        "ticket 13's decision, and it is to leave this off for good: a cell \
         cannot raise a glyph, so `x^2^` reads better as itself than as `x2`",
    ),
    (
        Options::ENABLE_SUBSCRIPT,
        false,
        "ticket 13's decision, and it is to leave this off for good, for the \
         same reason as superscript. `ENABLE_STRIKETHROUGH` is already on \
         (ticket 12), and pulldown-cmark strikes a single pair of tildes \
         too, so a whitespace-flanked subscript still loses its markers to \
         strikethrough regardless of this flag — `H~2~O`'s tildes are \
         non-flanking and survive, `log ~2~ n`'s do not. Turning this flag \
         on as well would not recover that case; it would only take the \
         tildes away from strikethrough for constructs where they flank",
    ),
    (
        Options::ENABLE_WIKILINKS,
        false,
        "ticket 13's decision, and it is to leave this off for good: `[[page]]` \
         is not standard markdown and following one is out of scope, so the \
         brackets are the honest rendering",
    ),
];

/// The flags actually handed to the parser: the `bool` column of [`FLAGS`],
/// folded. One caller in production (`rows`) and one in the test that pins it
/// — [`tests::strikethrough_tasklists_gfm_tables_and_yaml_metadata_are_the_only_flags_on`]
/// — which is why this earns being a
/// function rather than the fold sitting inline at either call site.
fn options() -> Options {
    FLAGS
        .iter()
        .filter(|(_, on, _)| *on)
        .fold(Options::empty(), |all, (flag, _, _)| all | *flag)
}

/// What a tag's block becomes, or `None` for an inline tag that is part of the
/// block around it and contributes no row of its own.
///
/// Exhaustive, with no `_ =>` arm, on purpose: a construct `pulldown-cmark`
/// grows does not compile until somebody has decided what it renders as. That
/// is R26.5b, and it is the same shape `src/keys.rs` carries one layer down.
///
/// Deciding what a construct *is* is not the same as laying it out — the match
/// in [`rows`] is where a kind gets rows, and the constructs still waiting for
/// one are named in the sweep's `UNRENDERED` list with the ticket that owes
/// them.
fn block_kind(tag: &Tag) -> Option<RowKind> {
    match tag {
        Tag::Paragraph => Some(RowKind::Paragraph),
        Tag::Heading { level, .. } => Some(RowKind::Heading(*level)),
        // The exact `BlockQuoteKind` this carries only matters at the point a
        // quote's own `Paragraph` is classified — `rows()` reads it off the
        // open quote stack there rather than off this generic classification,
        // which exists only to prove the tag has a decided layout.
        Tag::BlockQuote(kind) => Some(RowKind::Quote(*kind)),
        // A fence and an indented block are both verbatim text. This generic
        // classification only feeds the no-op match below — the real
        // decision between `Code` and a mermaid fence's `Diagram` is made
        // from the fence's language where the frame is actually opened.
        Tag::CodeBlock(_) => Some(RowKind::Code),
        // Verbatim for the same reason a fence is: HTML is preformatted, and
        // wrapping a tag across rows would break it. `RowKind::Code` rather
        // than a kind of its own — ticket 06's decision — since unwrapped and
        // set apart is exactly what a fence already gets.
        Tag::HtmlBlock => Some(RowKind::Code),
        // As with `BlockQuote`, the depth and marker only matter once
        // `Tag::Item` is reached in `rows()`; this placeholder just proves the
        // tag has a decided layout.
        Tag::List(_) | Tag::Item => Some(RowKind::List(ListItem {
            depth: 0,
            marker: None,
        })),
        // Inline in the sense this classification means: the container
        // carries no text of its own — its number is stamped onto the first
        // row its *own* `Tag::Paragraph` produces, in `rows()`'s
        // `RowKind::Paragraph` arm, exactly as a quote's alert is read off
        // `quotes` rather than given a row here. Returning `Some(Paragraph)`
        // instead would make this arm indistinguishable from a real
        // paragraph in the generic dispatch below and open two frames for
        // one block.
        Tag::FootnoteDefinition(_) => None,
        // A term and its definition are an indented list in everything but
        // name.
        Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
            Some(RowKind::List(ListItem {
                depth: 0,
                marker: None,
            }))
        }
        Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => Some(RowKind::Table),
        // Inline: a difference *inside* a row. Emphasis, Strong and
        // Strikethrough are carried as a piece's `Emphasis` by `rows` rather
        // than as a row kind — contributing a row of their own would break
        // the sentence they sit in. Superscript and Subscript stay `None`
        // with no modifier to carry: ticket 13 leaves both flags off for
        // good, so neither ever reaches this arm.
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Superscript | Tag::Subscript => {
            None
        }
        // Inline, and the text between the tags is the link text — which is
        // what "text shown, URL hidden" already amounts to.
        Tag::Link { .. } | Tag::Image { .. } => None,
        Tag::MetadataBlock(_) => Some(RowKind::Metadata),
    }
}

/// One open block, leaf or container. A leaf (`Heading`, `Paragraph`, a
/// quote's paragraph, a list item's own text) accumulates `segments`; a list
/// item that holds a nested list is *also* one of these, because a tight
/// item's text arrives with no `Paragraph` wrapper at all — `Tag::Item`
/// pushes a frame directly rather than waiting for one.
///
/// Frames nest — a list item containing a nested list is two frames open at
/// once — which `own_rows` is for: a child frame closing merges into its
/// parent's `own_rows` rather than the document's rows directly, so an
/// item's own text (still pending in `segments` when its nested list starts)
/// lands *before* the nested list's rows instead of after, matching source
/// order rather than close order.
struct Frame {
    kind: RowKind,
    line: usize,
    closes: TagEnd,
    segments: Vec<Vec<Piece>>,
    emphasis: Vec<Emphasis>,
    own_rows: Vec<Row>,
    /// The fence's language, `RowKind::Code` only — read by `flush` instead
    /// of wrapping, and `None` for an indented block or an unnamed fence.
    language: Option<String>,
}

/// One open table. Never nested — a table's own frame is not on `stack` at
/// all; a cell's content is, so that emphasis and inline code inside a cell
/// reuse the same [`Frame`] machinery a paragraph does, and this only tracks
/// the 2-D shape a `Frame` has nowhere to put: a header row set apart from a
/// body of rows, each a list of cells, each cell a list of pieces.
///
/// The header has no `TableRow` of its own — `pulldown-cmark` nests
/// `TableCell` directly under `TableHead`, unlike a body row, which nests
/// cells under a `TableRow` under the table — so `current_row` is flushed
/// into `header` on `TagEnd::TableHead` rather than on `TagEnd::TableRow`.
struct TableBuild {
    line: usize,
    alignments: Vec<Alignment>,
    header: Vec<Vec<Piece>>,
    body: Vec<Vec<Vec<Piece>>>,
    current_row: Vec<Vec<Piece>>,
}

/// The document being built: everything one markdown event may change, in one
/// place. The parse loop's arms are methods on this rather than a single
/// match over nine locals, so that "what does a `Tag::Item` touch" is answered
/// by a signature instead of by reading past every other tag.
struct Build {
    /// Byte offset of each source line, for [`Build::line`].
    starts: Vec<usize>,
    columns: usize,
    rows: Vec<Row>,
    stack: Vec<Frame>,
    /// The ordinal a list's next item takes, `None` for a bulleted list — one
    /// entry per open `Tag::List`, so a nested ordered list inside a bulleted
    /// one keeps its own count. Depth is this stack's length, read fresh at
    /// each `Tag::Item` rather than stored, so it never drifts from the lists
    /// actually open.
    lists: Vec<Option<u64>>,
    /// One entry per open `Tag::DefinitionList`, read fresh at each title or
    /// definition to derive its indent — nothing is stored past this.
    definition_lists: Vec<()>,
    /// The GFM alert kind of each open quote, `None` for a plain one — a
    /// paragraph opened while this is non-empty reads its innermost entry
    /// rather than becoming a plain `Paragraph`.
    quotes: Vec<Option<BlockQuoteKind>>,
    /// The table currently open, if any. Tables do not nest, so `Option`
    /// rather than a stack — unlike a cell's own content, which nests onto
    /// `stack` exactly like any other frame.
    table: Option<TableBuild>,
    /// A footnote's display number, assigned once per label regardless of
    /// whether a reference or the definition is seen first. See
    /// [`footnote_number`].
    footnotes: HashMap<String, usize>,
    /// The number of each open footnote definition, `Some` until stamped onto
    /// that definition's first paragraph and `None` after — a second
    /// paragraph in the same definition is prose with nowhere left to carry
    /// the number, the same way a list item's marker is cleared past its
    /// first row. Deliberately scoped to a paragraph: a definition whose
    /// first block is a list, a fence, a heading or a table would need the
    /// same stamp threaded through every one of those frame openings for a
    /// shape no sample in this repo or `CONSTRUCTS` uses, so it stays
    /// unmarked there rather than earning that spread — the number is
    /// dropped, never the definition's own text.
    footnote_defs: Vec<Option<usize>>,
}

impl Build {
    fn line(&self, offset: usize) -> usize {
        line_of(&self.starts, offset)
    }

    /// A tag opens: whatever emphasis, nesting depth and frame it brings, then
    /// the block layout `block_kind` gives it. The three tag groups are
    /// disjoint, so all three run rather than one being chosen.
    fn start(&mut self, tag: &Tag, line: usize) {
        self.open_emphasis(tag);
        self.open_depth(tag);
        self.open_frame(tag, line);
        self.open_table(tag, line);
        if let Some(kind) = block_kind(tag) {
            self.open_block(kind, tag, line);
        }
    }

    /// Tags that only style the text inside them. An image's alt text is the
    /// text between its tags, exactly as a link's text is — the only
    /// difference is the modifier it carries.
    fn open_emphasis(&mut self, tag: &Tag) {
        match tag {
            Tag::Emphasis => push_emphasis_on_top(&mut self.stack, |top| top.italic = true),
            Tag::Strong => push_emphasis_on_top(&mut self.stack, |top| top.strong = true),
            Tag::Strikethrough => push_emphasis_on_top(&mut self.stack, |top| top.struck = true),
            Tag::Image { .. } => push_emphasis_on_top(&mut self.stack, |top| top.image = true),
            _ => {}
        }
    }

    /// Tags that open a nesting level their children read back. The footnote
    /// container itself carries no text — see `block_kind`'s
    /// `Tag::FootnoteDefinition` arm — so only the number is tracked here, for
    /// the definition's own `Tag::Paragraph` to pick up.
    fn open_depth(&mut self, tag: &Tag) {
        match tag {
            Tag::List(start) => self.lists.push(*start),
            Tag::DefinitionList => self.definition_lists.push(()),
            Tag::BlockQuote(kind) => self.quotes.push(*kind),
            Tag::FootnoteDefinition(label) => {
                let n = footnote_number(label, &mut self.footnotes);
                self.footnote_defs.push(Some(n));
            }
            _ => {}
        }
    }

    /// Tags that push a [`Frame`] of their own onto the stack.
    fn open_frame(&mut self, tag: &Tag, line: usize) {
        match tag {
            Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                self.open_definition(tag, line)
            }
            Tag::Item => self.open_item(line),
            // Verbatim, one row per source line, unwrapped and highlighted
            // with the empty language — the same treatment a fence with no
            // language gets, which is what "shown literally and set apart"
            // amounts to for markup rather than prose.
            Tag::HtmlBlock => push_frame(
                &mut self.stack,
                RowKind::Code,
                line,
                TagEnd::HtmlBlock,
                self.columns,
            ),
            // A metadata block's own frame, so its lines land as rows set
            // apart from the document rather than as text with nowhere to go.
            Tag::MetadataBlock(_) => push_frame(
                &mut self.stack,
                RowKind::Metadata,
                line,
                tag.to_end(),
                self.columns,
            ),
            Tag::CodeBlock(kind) => self.open_code_block(kind, line),
            Tag::TableCell => push_frame(
                &mut self.stack,
                RowKind::Table,
                line,
                TagEnd::TableCell,
                self.columns,
            ),
            _ => {}
        }
    }

    /// Depth doubles per nesting level, one unit for the term and a second for
    /// its own definition — the same `lists.len()`-reads-nesting-fresh pattern
    /// `Tag::Item` uses, widened by one bit rather than stored, since a
    /// definition list nested inside a definition must indent deeper than its
    /// own outer term.
    fn open_definition(&mut self, tag: &Tag, line: usize) {
        let depth = 2 * self.definition_lists.len().saturating_sub(1)
            + usize::from(matches!(tag, Tag::DefinitionListDefinition));
        push_frame(
            &mut self.stack,
            RowKind::List(ListItem {
                depth,
                marker: None,
            }),
            line,
            tag.to_end(),
            self.columns,
        );
    }

    /// A tight item's content is bare text with no `Paragraph` wrapper, so the
    /// frame opens here rather than waiting for one. A loose item's nested
    /// `Paragraph` finds this frame already open and stays transparent — see
    /// [`Build::open_paragraph`].
    fn open_item(&mut self, line: usize) {
        let marker = match self.lists.last_mut() {
            Some(Some(ordinal)) => {
                let this_one = *ordinal;
                *ordinal += 1;
                Marker::Ordinal(this_one)
            }
            _ => Marker::Bullet,
        };
        let kind = RowKind::List(ListItem {
            depth: self.lists.len(),
            marker: Some(marker),
        });
        push_frame(&mut self.stack, kind, line, TagEnd::Item, self.columns);
    }

    /// Verbatim text, pushed directly rather than through `block_kind`'s
    /// generic `RowKind::Code` arm — that arm stays a no-op so
    /// `Tag::HtmlBlock` does not also push a second frame here. The language
    /// is the fence's info string's first word, or `None` for an indented
    /// block, which is `flush`'s signal to highlight rather than wrap. A fence
    /// whose language is exactly `mermaid` opens as `RowKind::Diagram` instead
    /// — ticket 07's signal to `flush` to lay it out as a diagram rather than
    /// highlight it as code.
    fn open_code_block(&mut self, kind: &CodeBlockKind, line: usize) {
        let language = match kind {
            CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(str::to_string),
            CodeBlockKind::Indented => None,
        };
        let kind = if language.as_deref() == Some("mermaid") {
            RowKind::Diagram
        } else {
            RowKind::Code
        };
        push_frame(&mut self.stack, kind, line, TagEnd::CodeBlock, self.columns);
        if let Some(frame) = self.stack.last_mut() {
            frame.language = language;
        }
    }

    /// A table's own shape lives in `table`, never on `stack` — see
    /// [`TableBuild`]. Only a cell's content goes through a real `Frame`,
    /// pushed by [`Build::open_frame`], so its own emphasis and inline code
    /// reuse the exact machinery a paragraph does.
    fn open_table(&mut self, tag: &Tag, line: usize) {
        match tag {
            Tag::Table(alignments) => {
                self.table = Some(TableBuild {
                    line,
                    alignments: alignments.clone(),
                    header: Vec::new(),
                    body: Vec::new(),
                    current_row: Vec::new(),
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(build) = self.table.as_mut() {
                    build.current_row = Vec::new();
                }
            }
            _ => {}
        }
    }

    /// Which kinds have a layout yet. Exhaustive on `RowKind`, so a kind added
    /// for a construct is a compile error here until somebody decides how its
    /// rows are built; the sweep's `UNRENDERED` list names the ticket that
    /// owes each of the rest.
    fn open_block(&mut self, kind: RowKind, tag: &Tag, line: usize) {
        match kind {
            RowKind::Heading(_) => {
                push_frame(&mut self.stack, kind, line, tag.to_end(), self.columns);
            }
            RowKind::Paragraph => self.open_paragraph(line),
            RowKind::Quote(_) | RowKind::List(_) => {}
            RowKind::Code
            | RowKind::Diagram
            | RowKind::Metadata
            | RowKind::Table
            | RowKind::Rule => {}
        }
    }

    fn open_paragraph(&mut self, line: usize) {
        // A loose item and a loose definition both wrap their content in a
        // real `Tag::Paragraph`, and both must stay transparent for the same
        // reason: the container's own frame already holds the text (a tight
        // item's, with no wrapper at all), so a second frame here would render
        // the definition at depth 0 instead of indented under its term.
        let transparent = matches!(
            self.stack.last().map(|frame| frame.closes),
            Some(TagEnd::Item | TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition)
        );
        if transparent {
            // A second paragraph in a loose item or definition still needs a
            // break from the first, the same way a hard break does.
            if let Some(frame) = self.stack.last_mut() {
                if !frame.own_rows.is_empty() || frame.segments != [Vec::new()] {
                    frame.segments.push(Vec::new());
                }
            }
            return;
        }
        let kind = match self.quotes.last() {
            Some(alert) => RowKind::Quote(*alert),
            None => RowKind::Paragraph,
        };
        push_frame(&mut self.stack, kind, line, TagEnd::Paragraph, self.columns);
        // The definition's own number, stamped onto this paragraph's first
        // piece so `[1]` and its text land in the same row rather than two —
        // `take` leaves a later paragraph in a loose definition unmarked.
        if let Some(n) = self.footnote_defs.last_mut().and_then(Option::take) {
            let frame = self.stack.last_mut().expect("just pushed above");
            push_piece(&mut frame.segments, &format!("[{n}] "), Emphasis::default());
        }
    }

    /// The four table endings are handled apart from the generic
    /// frame-closing below: a cell's frame closes into `table.current_row`
    /// instead of into a parent frame or the document, and a row and the table
    /// itself close into `table`'s own fields, never onto `stack`.
    fn end(&mut self, end: TagEnd) {
        match end {
            TagEnd::TableCell => self.close_cell(),
            TagEnd::TableRow => {
                if let Some(build) = self.table.as_mut() {
                    let row = std::mem::take(&mut build.current_row);
                    build.body.push(row);
                }
            }
            TagEnd::TableHead => {
                if let Some(build) = self.table.as_mut() {
                    build.header = std::mem::take(&mut build.current_row);
                }
            }
            TagEnd::Table => self.close_table(),
            _ => self.close_frame(end),
        }
    }

    fn close_cell(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        // A cell's content is always one segment: a pipe-table row is confined
        // to one source line, so nothing inside a cell can produce the
        // `Event::HardBreak` that starts a second one — unlike a paragraph,
        // which can span several lines.
        let pieces = frame.segments.into_iter().next().unwrap_or_default();
        if let Some(build) = self.table.as_mut() {
            build.current_row.push(pieces);
        }
    }

    fn close_table(&mut self) {
        let Some(build) = self.table.take() else {
            return;
        };
        let line = build.line;
        let laid = table_rows(build, self.columns);
        if !self.rows.is_empty() {
            self.rows.push(separator_row(line));
        }
        self.rows.extend(laid);
    }

    fn close_frame(&mut self, end: TagEnd) {
        if matches!(
            end,
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Image
        ) {
            if let Some(frame) = self.stack.last_mut() {
                frame.emphasis.pop();
            }
        }
        match end {
            TagEnd::List(_) => {
                self.lists.pop();
            }
            TagEnd::DefinitionList => {
                self.definition_lists.pop();
            }
            TagEnd::BlockQuote(_) => {
                self.quotes.pop();
            }
            TagEnd::FootnoteDefinition => {
                self.footnote_defs.pop();
            }
            _ => {}
        }
        if matches!(self.stack.last(), Some(frame) if frame.closes == end) {
            let frame = self.stack.pop().expect("just matched Some above");
            close_frame(&mut self.stack, &mut self.rows, frame, self.columns);
        }
    }

    /// Prose, and an HTML block's own lines fed to the frame `Tag::HtmlBlock`
    /// opened earlier — both plain text with whatever emphasis the open frame
    /// already carries.
    fn push_text(&mut self, text: &str) {
        if let Some(frame) = self.stack.last_mut() {
            let style = *frame.emphasis.last().unwrap();
            push_piece(&mut frame.segments, text, style);
        }
    }

    /// A span of backticks inside prose or a tag inside a sentence — `<br>`,
    /// `<kbd>x</kbd>`, a comment — both set apart with `code` on top of
    /// whatever emphasis already surrounds them, so `` *`x`* `` and
    /// `*<b>x</b>*` both still carry the italic too.
    fn push_code(&mut self, text: &str) {
        if let Some(frame) = self.stack.last_mut() {
            let mut style = *frame.emphasis.last().unwrap();
            style.code = true;
            push_piece(&mut frame.segments, text, style);
        }
    }

    /// A line ending the author asked for, which ends the row without ending
    /// the block: a new segment starts, and wrapping never crosses into it.
    fn hard_break(&mut self) {
        if let Some(frame) = self.stack.last_mut() {
            frame.segments.push(Vec::new());
        }
    }

    /// Fires as the first event inside a task item, before any text —
    /// overriding the `Bullet` `Tag::Item` already set, since a task item is
    /// still an item and needs a depth and ordinal exactly as any other.
    fn task_marker(&mut self, checked: bool) {
        if let Some(frame) = self.stack.last_mut() {
            if let RowKind::List(item) = &mut frame.kind {
                item.marker = Some(Marker::Task(checked));
            }
        }
    }

    /// The reference's own number, the same one its definition's paragraph
    /// carries — plain text, since a reference is prose wearing a number
    /// rather than a construct needing a modifier.
    fn footnote_reference(&mut self, label: &str) {
        if let Some(frame) = self.stack.last_mut() {
            let n = footnote_number(label, &mut self.footnotes);
            let style = *frame.emphasis.last().unwrap();
            push_piece(&mut frame.segments, &format!("[{n}]"), style);
        }
    }

    /// A thematic break belongs to no frame — it interrupts whatever surrounds
    /// it rather than being contained by it — so it is pushed straight onto
    /// the document's rows.
    fn rule(&mut self, line: usize) {
        let follows_list = matches!(self.rows.last().map(|row| row.kind), Some(RowKind::List(_)));
        if !self.rows.is_empty() && !follows_list {
            self.rows.push(separator_row(line));
        }
        self.rows.push(Row {
            kind: RowKind::Rule,
            line,
            pieces: Vec::new(),
            refused: None,
        });
    }
}

/// The document `text` describes, laid out to `columns`.
///
/// `columns` of zero means the terminal has not reported its size yet, which is
/// not a pane one column wide — it is no pane at all. Nothing is wrapped until
/// a width arrives, and the first `Resized` reflows it.
pub fn rows(text: &str, columns: usize) -> Vec<Row> {
    let mut build = Build {
        starts: line_starts(text),
        columns,
        rows: Vec::new(),
        stack: Vec::new(),
        lists: Vec::new(),
        definition_lists: Vec::new(),
        quotes: Vec::new(),
        table: None,
        footnotes: HashMap::new(),
        footnote_defs: Vec::new(),
    };

    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        let line = build.line(range.start);
        match event {
            Event::Start(tag) => build.start(&tag, line),
            Event::End(end) => build.end(end),
            Event::Code(text) | Event::InlineHtml(text) => build.push_code(&text),
            Event::Text(text) | Event::Html(text) => build.push_text(&text),
            // The author's line ending, not the pane's — the row it belongs
            // to is decided by the pane's width, so it becomes a space rather
            // than a break.
            Event::SoftBreak => build.push_text(" "),
            Event::HardBreak => build.hard_break(),
            Event::TaskListMarker(checked) => build.task_marker(checked),
            // Math arrives only under a flag [`FLAGS`] records as off for
            // good, so this cannot fire today and dropping it changes
            // nothing: `$x^2$` reaches the reader as itself. Collecting an
            // `InlineMath`'s contents as prose would turn it into `x2`,
            // which destroys more than it renders — the reason the flag
            // stays off rather than a home ever being built for this.
            Event::InlineMath(_) | Event::DisplayMath(_) => {}
            Event::FootnoteReference(label) => build.footnote_reference(&label),
            Event::Rule => build.rule(line),
        }
    }
    build.rows
}

/// A blank row: the separator between one block and the next, wherever a
/// caller has already decided one belongs — `close_frame`'s "two list rows
/// never separate", `Event::Rule`'s "a rule after a list doesn't either", and
/// `TagEnd::Table`'s "always, since a table is never part of what precedes
/// it" each keep their own guard; only the row itself was duplicated three
/// times over.
fn separator_row(line: usize) -> Row {
    Row {
        kind: RowKind::Paragraph,
        line,
        pieces: Vec::new(),
        refused: None,
    }
}

/// Opens a frame, first flushing the currently open one's pending text into
/// its own rows — a nested frame (a list item's own text, then its nested
/// list) would otherwise sit in the parent's `segments` past the point the
/// child's rows are ready to merge in, reversing their order.
fn push_frame(stack: &mut Vec<Frame>, kind: RowKind, line: usize, closes: TagEnd, columns: usize) {
    if let Some(top) = stack.last_mut() {
        flush(top, columns);
    }
    stack.push(Frame {
        kind,
        line,
        closes,
        segments: vec![Vec::new()],
        emphasis: vec![Emphasis::default()],
        own_rows: Vec::new(),
        language: None,
    });
}

/// Closes a frame: flushes what is still pending, then either merges into
/// the parent frame still open (a nested list closing back into its item) or,
/// once the stack is empty, lands on the document's rows with the blank-row
/// separator applied.
fn close_frame(stack: &mut [Frame], rows: &mut Vec<Row>, mut frame: Frame, columns: usize) {
    flush(&mut frame, columns);
    if let Some(parent) = stack.last_mut() {
        parent.own_rows.extend(frame.own_rows);
        return;
    }
    // A blank row before every block but the first: without it a heading and
    // the paragraph under it run together and the document reads as one,
    // which is the thing a Preview exists to undo. Two list rows in a row are
    // the one exception — sibling items of the same list read as one list,
    // not as a blank line between every bullet.
    let both_list = matches!(rows.last().map(|row| row.kind), Some(RowKind::List(_)))
        && matches!(frame.kind, RowKind::List(_));
    if !rows.is_empty() && !both_list {
        rows.push(separator_row(frame.line));
    }
    rows.extend(frame.own_rows);
}

/// Lays out a frame's pending `segments` into its `own_rows` and resets
/// `segments` to empty. A list item's marker — the bullet, ordinal or
/// checkbox — belongs on the first row the item ever produces and nowhere
/// else, so it is cleared on every row after that: the first flush's first
/// row when `own_rows` is still empty, every row of a later flush (loose
/// paragraph, or text following a nested list) once it is not.
fn flush(frame: &mut Frame, columns: usize) {
    if frame.segments == [Vec::new()] {
        return;
    }
    let mut laid = match frame.kind {
        RowKind::Code => code_rows(
            RowKind::Code,
            frame.line,
            frame.language.as_deref().unwrap_or(""),
            &frame.segments,
        ),
        RowKind::Diagram => diagram_rows(frame.line, columns, &frame.segments),
        // Verbatim, one row per source line — the same shape a code block
        // takes, since a metadata block is preformatted for the same reason
        // a fence is.
        RowKind::Metadata => code_rows(RowKind::Metadata, frame.line, "", &frame.segments),
        _ => laid_out(frame.kind, frame.line, &frame.segments, columns),
    };
    if let RowKind::List(item) = frame.kind {
        let already_marked = !frame.own_rows.is_empty();
        for row in laid.iter_mut().skip(usize::from(!already_marked)) {
            row.kind = RowKind::List(ListItem {
                marker: None,
                ..item
            });
        }
    }
    frame.own_rows.extend(laid);
    frame.segments = vec![Vec::new()];
}

/// Enters a modifier on the currently open frame: pushes a copy of its active
/// `Emphasis` with `set` applied, so the matching close only has to pop
/// rather than know which field to clear. A snapshot per push is what keeps
/// this correct under nesting no matter how deep, with no counter per field
/// to keep in sync.
fn push_emphasis_on_top(stack: &mut [Frame], set: impl FnOnce(&mut Emphasis)) {
    if let Some(frame) = stack.last_mut() {
        let mut top = *frame.emphasis.last().unwrap();
        set(&mut top);
        frame.emphasis.push(top);
    }
}

/// Appends `text` to the open segment, merging into the last piece when its
/// emphasis already matches rather than growing a new one-character piece per
/// word — a row of plain prose still ends up as the one piece it always was.
fn push_piece(segments: &mut [Vec<Piece>], text: &str, emphasis: Emphasis) {
    if text.is_empty() {
        return;
    }
    let current = segments.last_mut().expect("a block always has a segment");
    if let Some(last) = current.last_mut() {
        if last.emphasis == emphasis {
            last.text.push_str(text);
            return;
        }
    }
    current.push(Piece {
        text: text.to_string(),
        emphasis,
        token: None,
    });
}

/// A verbatim block's segments as highlighted rows, one per source line and
/// never wrapped — a line broken at the pane edge is a line the block's own
/// syntax does not permit. `kind` is stamped onto every row this returns,
/// which is what lets a metadata block reuse this rather than duplicating it
/// — `RowKind::Metadata` with `language` empty, exactly as an unnamed fence
/// already is. `language` is the fence's info string, or the empty string
/// for an indented block, an unnamed fence, or a metadata block; either way
/// [`highlight::highlight`] answers with plain tokens rather than failing.
///
/// The block's own trailing newline — the line ending before the closing
/// fence — is stripped first, or it would surface as one extra empty row;
/// `pulldown-cmark` hands the fence's lines with no other separator to split
/// rows on, so a row here is a line of the highlighter's answer.
fn code_rows(kind: RowKind, line: usize, language: &str, segments: &[Vec<Piece>]) -> Vec<Row> {
    let raw: String = segments
        .iter()
        .flatten()
        .map(|piece| piece.text.as_str())
        .collect();
    let source = raw.strip_suffix('\n').unwrap_or(&raw);
    highlight::highlight(language, source)
        .into_iter()
        .map(|tokens| Row {
            kind,
            line,
            pieces: tokens
                .into_iter()
                .map(|token| Piece {
                    text: token.text,
                    emphasis: Emphasis::default(),
                    token: Some(token.kind),
                })
                .collect(),
            refused: None,
        })
        .collect()
}

/// A mermaid fence's segments, rendered as a diagram laid out to `columns` —
/// or, when `mermaid-text` cannot draw it, or draws it wider than `columns`
/// allows, the reason and the fence's raw source as one row of highlighted
/// code. This is 05's fallback shape reached from a render failure instead
/// of an unrecognised language, which is why ticket 07 was blocked on it.
///
/// The fallback source is joined onto a single row rather than split one per
/// source line the way [`code_rows`] does: a diagram whose source could not
/// be drawn is not offering syntax the reader is meant to read line by line,
/// only a reference for what was attempted.
fn diagram_rows(line: usize, columns: usize, segments: &[Vec<Piece>]) -> Vec<Row> {
    let raw: String = segments
        .iter()
        .flatten()
        .map(|piece| piece.text.as_str())
        .collect();
    let source = raw.strip_suffix('\n').unwrap_or(&raw);
    let width = (columns > 0).then_some(columns);
    match mermaid_text::render_with_width(source, width) {
        Ok(diagram) if fits(width, &diagram) => diagram
            .lines()
            .map(|text| Row {
                kind: RowKind::Diagram,
                line,
                pieces: vec![Piece {
                    text: text.to_string(),
                    emphasis: Emphasis::default(),
                    token: None,
                }],
                refused: None,
            })
            .collect(),
        Ok(_) => diagram_fallback(line, source, DiagramRefusal::TooWide),
        Err(error) => diagram_fallback(line, source, DiagramRefusal::from(&error)),
    }
}

/// Whether every line of a rendered diagram fits `width` — `None` (no width
/// reported yet) always fits. Measured in display columns via
/// [`UnicodeWidthStr`], not `chars().count()`: `mermaid-text` compacts to the
/// budget it was given using the same measure internally, and a box-drawing
/// diagram carrying a wide-character label (CJK text in a node) would count
/// short on characters while still overflowing the pane it is measured
/// against.
fn fits(width: Option<usize>, diagram: &str) -> bool {
    width.is_none_or(|w| diagram.lines().all(|l| l.width() <= w))
}

/// The reason and the fence's raw source, as one highlighted [`RowKind::Code`]
/// row — see [`diagram_rows`] for why it is one row rather than one per line.
fn diagram_fallback(line: usize, source: &str, reason: DiagramRefusal) -> Vec<Row> {
    let joined = source.lines().collect::<Vec<_>>().join(" ");
    let pieces = highlight::highlight("mermaid", &joined)
        .into_iter()
        .flatten()
        .map(|token| Piece {
            text: token.text,
            emphasis: Emphasis::default(),
            token: Some(token.kind),
        })
        .collect();
    vec![Row {
        kind: RowKind::Code,
        line,
        pieces,
        refused: Some(reason),
    }]
}

/// A block's segments as the rows they take. Each segment is one hard break's
/// worth of pieces, laid out independently: wrapping never crosses a hard
/// break, because that break is a line ending the author asked for.
fn laid_out(kind: RowKind, line: usize, segments: &[Vec<Piece>], columns: usize) -> Vec<Row> {
    segments
        .iter()
        .flat_map(|pieces| wrapped(pieces, textwrap::Options::new(columns)))
        .map(|pieces| Row {
            kind,
            line,
            pieces,
            refused: None,
        })
        .collect()
}

/// One segment's pieces, split at the same points `textwrap` would break the
/// flattened text at. `textwrap` wraps a string; it has no notion of a piece
/// boundary, so the break is found in the flattened text first and then
/// walked back onto the pieces that produced it — splitting one where a break
/// falls inside it, and carrying its emphasis into both halves.
///
/// The caller passes `textwrap`'s own options rather than a width, because a
/// table cell wants one of them different: it declines to break a word too
/// long for its column, overflowing instead — see [`table_rows`].
pub(crate) fn wrapped(pieces: &[Piece], options: textwrap::Options<'_>) -> Vec<Vec<Piece>> {
    if options.width == 0 {
        return vec![pieces.to_vec()];
    }
    let flat: String = pieces.iter().map(|piece| piece.text.as_str()).collect();
    if flat.is_empty() {
        // An empty segment is a hard break immediately followed by another
        // hard break or the block's end — still a row, just one with nothing
        // on it, the same way a blank source line is still a line.
        return vec![Vec::new()];
    }
    let mut at = 0;
    let spans: Vec<(usize, usize, &Piece)> = pieces
        .iter()
        .map(|piece| {
            let span = (at, at + piece.text.len(), piece);
            at += piece.text.len();
            span
        })
        .collect();
    let mut rows = Vec::new();
    let mut cursor = 0;
    for line in textwrap::wrap(&flat, options) {
        // `textwrap` may trim whitespace at a break rather than returning an
        // exact substring, so the break is located by searching forward
        // rather than assumed to start where the last one ended. A miss is
        // unreachable — every wrapped line is `textwrap`'s own subset of
        // `flat` in order — and it fails loudly rather than quietly dropping
        // a row of the document.
        let offset = flat[cursor..]
            .find(line.as_ref())
            .expect("a wrapped line is a substring of what was wrapped");
        let start = cursor + offset;
        let end = start + line.len();
        cursor = end;
        rows.push(
            spans
                .iter()
                .filter_map(|(piece_start, piece_end, piece)| {
                    let from = start.max(*piece_start);
                    let to = end.min(*piece_end);
                    (from < to).then(|| Piece {
                        text: piece.text[from - piece_start..to - piece_start].to_string(),
                        emphasis: piece.emphasis,
                        token: piece.token,
                    })
                })
                .collect(),
        );
    }
    rows
}

/// A finished table's rows, columns aligned to their widest cell and each
/// column's declared alignment honoured. The header is set apart by marking
/// its pieces bold rather than by drawing a rule of dashes: no ticket asks
/// for one, and inventing a glyph nobody named is exactly what R26.5a's sweep
/// exists to catch. Every row shares the table's own opening line, the same
/// way a fenced code block's rows all carry the fence's line.
///
/// A column too wide for `columns` shrinks, and a cell too wide for the
/// column it lands in wraps *inside* that column — one source row becoming as
/// many display rows as its tallest cell needs, its shorter cells padded down
/// the whole height so no column shifts. Wrapping within a fixed column width
/// is what preserves the alignment a table exists to show; reflowing a
/// laid-out row would destroy it, and this code once refused the first because
/// it had rejected the second — see
/// `docs/adr/0019-a-table-cell-wraps-inside-its-column.md`.
///
/// A word too long for its column is not broken: it overflows, the row renders
/// wider than `columns` and is clipped, exactly as an unwrapped long code line
/// already is. That is recoverable by scrolling sideways; a cut cell is gone
/// from [`Row::text`] altogether. `columns` of zero means the terminal has not
/// reported a size yet, the same convention [`rows`] itself uses, so nothing
/// shrinks and nothing wraps before then.
fn table_rows(build: TableBuild, columns: usize) -> Vec<Row> {
    let column_count = build
        .alignments
        .len()
        .max(build.header.len())
        .max(build.body.iter().map(Vec::len).max().unwrap_or(0));
    if column_count == 0 {
        return Vec::new();
    }
    let cell_width =
        |cell: &[Piece]| -> usize { cell.iter().map(|piece| piece.text.width()).sum() };
    let natural: Vec<usize> = (0..column_count)
        .map(|column| {
            let header_width = build.header.get(column).map_or(0, |cell| cell_width(cell));
            let body_width = build
                .body
                .iter()
                .map(|row| row.get(column).map_or(0, |cell| cell_width(cell)))
                .max()
                .unwrap_or(0);
            header_width.max(body_width).max(1)
        })
        .collect();
    const GAP: usize = 2;
    let widths = shrink_to_fit(&natural, columns, GAP);
    let alignment = |column: usize| {
        build
            .alignments
            .get(column)
            .copied()
            .unwrap_or(Alignment::None)
    };
    let mut rows = Vec::new();
    if !build.header.is_empty() {
        for mut pieces in row_pieces(&build.header, &widths, column_count, GAP, alignment) {
            for piece in &mut pieces {
                piece.emphasis.strong = true;
            }
            rows.push(Row {
                kind: RowKind::Table,
                line: build.line,
                pieces,
                refused: None,
            });
        }
    }
    for row in &build.body {
        for pieces in row_pieces(row, &widths, column_count, GAP, alignment) {
            rows.push(Row {
                kind: RowKind::Table,
                line: build.line,
                pieces,
                refused: None,
            });
        }
    }
    rows
}

/// The narrowest a column is shrunk to — below this a column is a stack of
/// fragments rather than a column, and shrinking further buys nothing anyway;
/// the argument is in `docs/adr/0019-a-table-cell-wraps-inside-its-column.md`.
const MIN_COLUMN: usize = 4;

/// The widths a table's columns actually draw at: `natural` unless the whole
/// row — every column plus the gaps between them — would not fit `columns`,
/// in which case the currently-widest column gives up one column at a time
/// until it does, or until every column has reached [`MIN_COLUMN`] — or its
/// own natural width, if that is narrower still. Shrinking the widest column
/// first is what keeps a table of mostly-short cells and one very long one
/// legible instead of squeezing every column equally.
fn shrink_to_fit(natural: &[usize], columns: usize, gap: usize) -> Vec<usize> {
    let mut widths = natural.to_vec();
    if columns == 0 {
        return widths;
    }
    let total = |widths: &[usize]| -> usize {
        widths.iter().sum::<usize>() + gap * widths.len().saturating_sub(1)
    };
    let floor = |column: usize| natural[column].min(MIN_COLUMN);
    while total(&widths) > columns {
        let Some((index, width)) = widths
            .iter()
            .copied()
            .enumerate()
            .filter(|(column, width)| *width > floor(*column))
            .max_by_key(|(_, width)| *width)
        else {
            break;
        };
        widths[index] = width - 1;
    }
    widths
}

/// One source row as the display rows it takes: each column's cell wrapped to
/// its width, in source order, with a plain gap between columns. The row is as
/// tall as its tallest cell, and a cell with fewer rows than that is padded to
/// the full width on the rows it has nothing on — otherwise every column after
/// a wrapped one would shift left on all but its first row. A row shorter than
/// `column_count` — a ragged table row GFM still permits — reads its missing
/// columns as empty cells rather than shifting the ones it has.
fn row_pieces(
    row: &[Vec<Piece>],
    widths: &[usize],
    column_count: usize,
    gap: usize,
    alignment: impl Fn(usize) -> Alignment,
) -> Vec<Vec<Piece>> {
    let empty = Vec::new();
    let cells: Vec<Vec<Vec<Piece>>> = widths
        .iter()
        .enumerate()
        .take(column_count)
        .map(|(column, width)| {
            let cell = row.get(column).unwrap_or(&empty);
            wrapped(cell, textwrap::Options::new(*width).break_words(false))
        })
        .collect();
    let height = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
    (0..height)
        .map(|display| {
            let mut pieces = Vec::new();
            for (column, cell) in cells.iter().enumerate() {
                let line = cell.get(display).map_or(&empty[..], Vec::as_slice);
                pieces.extend(pad_cell(line, widths[column], alignment(column)));
                if column + 1 < column_count {
                    pieces.push(plain_spaces(gap));
                }
            }
            pieces
        })
        .collect()
}

/// One display row of a cell padded to `width` by its declared alignment.
/// Padding is a plain [`Piece`] with no modifier, appended or prepended around
/// the cell's own pieces, which is what keeps a padded **bold** cell's padding
/// unbold. The alignment applies to every display row a wrapped cell occupies,
/// since it is a property of the column rather than of the first row.
///
/// A row can be wider than `width` — an unbreakable word [`wrapped`] declined
/// to split — in which case it overflows rather than being cut; and it can
/// fall short by more than its text, since a display-column budget does not
/// divide evenly by a double-width glyph. Both are the same arithmetic: pad by
/// whatever is left over, which for the overflowing row is nothing.
fn pad_cell(cell: &[Piece], width: usize, alignment: Alignment) -> Vec<Piece> {
    let natural: usize = cell.iter().map(|piece| piece.text.width()).sum();
    let pad = width.saturating_sub(natural);
    let (left, right) = match alignment {
        Alignment::Right => (pad, 0),
        Alignment::Center => (pad / 2, pad - pad / 2),
        Alignment::None | Alignment::Left => (0, pad),
    };
    let mut pieces = Vec::new();
    if left > 0 {
        pieces.push(plain_spaces(left));
    }
    pieces.extend(cell.iter().cloned());
    if right > 0 {
        pieces.push(plain_spaces(right));
    }
    pieces
}

/// A plain run of `count` spaces, carrying no emphasis and no token — the
/// shape padding and the gap between columns both need.
fn plain_spaces(count: usize) -> Piece {
    Piece {
        text: " ".repeat(count),
        emphasis: Emphasis::default(),
        token: None,
    }
}

/// A footnote's display number: assigned the first time its label is seen,
/// by either a reference or a definition, whichever the parser hands back
/// first. Real documents put the definition after the reference it belongs
/// to, so first-seen order and reference order agree in practice; a
/// definition that never had a reference still gets a number this way
/// instead of one silently missing. `seen.len()` is the next number due —
/// one counter, not two that could drift apart.
fn footnote_number(label: &str, seen: &mut HashMap<String, usize>) -> usize {
    let next = seen.len() + 1;
    *seen.entry(label.to_string()).or_insert(next)
}

/// The byte offset each line starts at, so a block's offset can be turned into
/// a line number without counting newlines from the top of the file per block.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
    starts
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|start| *start <= offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(rows: &[Row]) -> Vec<RowKind> {
        rows.iter().map(|row| row.kind).collect()
    }

    fn texts(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::text).collect()
    }

    /// One markdown sample per construct in the table in
    /// `.scratch/markdown-preview/spec.md`, with the [`RowKind`] the
    /// classification says it becomes. The sweep below holds each to three
    /// mechanical things and to nothing else: it produces a row with text on
    /// it, one of its rows has the kind it was classified as, and it leaks no
    /// markup marker.
    ///
    /// No glyph and no colour is asserted anywhere here, for the reason
    /// `highlighting.feature` asserts token kinds and never colours: a bullet
    /// character is a theme decision exactly as a colour is, and a suite that
    /// goes red when `•` becomes `‣` is a suite people learn to ignore. That
    /// also means the sweep cannot see a heading drawn at one weight or an
    /// emphasis drawn flat — those are ticket 12's, tracked in `spec.md`'s
    /// table rather than here.
    const CONSTRUCTS: [(&str, &str, RowKind); 29] = [
        ("paragraph", "Prose in a paragraph.\n", RowKind::Paragraph),
        (
            "heading",
            "## Install\n",
            RowKind::Heading(HeadingLevel::H2),
        ),
        ("soft break", "one\ntwo\n", RowKind::Paragraph),
        (
            "heading level",
            "# One\n\n### Three\n",
            RowKind::Heading(HeadingLevel::H1),
        ),
        (
            "emphasis",
            "*em* **strong** ~~struck~~\n",
            RowKind::Paragraph,
        ),
        ("inline code", "Call `main` first.\n", RowKind::Paragraph),
        ("hard break", "one  \ntwo\n", RowKind::Paragraph),
        (
            "list",
            "- one\n- two\n  - nested\n",
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Bullet),
            }),
        ),
        (
            "ordered list",
            "1. first\n2. second\n",
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Ordinal(1)),
            }),
        ),
        (
            "task item",
            "- [ ] todo\n- [x] done\n",
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Task(false)),
            }),
        ),
        ("block quote", "> quoted\n", RowKind::Quote(None)),
        (
            "alert",
            "> [!NOTE]\n> mind this\n",
            RowKind::Quote(Some(BlockQuoteKind::Note)),
        ),
        ("table", "| a | b |\n|---|---|\n| 1 | 2 |\n", RowKind::Table),
        (
            "table alignment",
            "| a | b |\n|:--|--:|\n| 1 | 2 |\n",
            RowKind::Table,
        ),
        ("thematic break", "one\n\n***\n\ntwo\n", RowKind::Rule),
        ("fenced code", "```rust\nfn main() {}\n```\n", RowKind::Code),
        ("indented code", "    let x = 1;\n", RowKind::Code),
        (
            "link",
            "See [Varde](https://example.com).\n",
            RowKind::Paragraph,
        ),
        ("image", "![a cat](cat.png)\n", RowKind::Paragraph),
        (
            "raw html",
            "<div>block</div>\n\nInline <b>bold</b>.\n",
            RowKind::Code,
        ),
        (
            "yaml frontmatter",
            "---\ntitle: Varde\n---\n\nProse.\n",
            RowKind::Metadata,
        ),
        (
            "mermaid fence",
            "```mermaid\ngraph TD\n  A --> B\n```\n",
            RowKind::Diagram,
        ),
        (
            "footnote",
            "Text[^1].\n\n[^1]: a note\n",
            RowKind::Paragraph,
        ),
        (
            "definition list",
            "term\n: meaning\n",
            RowKind::List(ListItem {
                depth: 0,
                marker: None,
            }),
        ),
        ("superscript", "x^2^ and H~2~O\n", RowKind::Paragraph),
        ("math", "$x^2$ and $$y = mx + b$$\n", RowKind::Paragraph),
        (
            "heading attributes",
            "## Install {#install}\n",
            RowKind::Heading(HeadingLevel::H2),
        ),
        (
            "toml frontmatter",
            "+++\ntitle = \"Varde\"\n+++\n\nProse.\n",
            RowKind::Metadata,
        ),
        ("wikilink", "See [[Setup]] for more.\n", RowKind::Paragraph),
    ];

    /// The markers a Preview exists to consume. A row still holding one is a
    /// row showing the reader the markup instead of the document. `~~` joined
    /// the list once ticket 12 gave strikethrough a span to carry its
    /// modifier in — before that its tildes were the one leak the other three
    /// would have let through.
    const MARKERS: [&str; 9] = ["##", "**", "- ", "|", "`", "~~", "> ", "[!", "[ ]"];

    /// Constructs the render does not reach yet, each with the ticket that owes
    /// it. Not a progress checklist of the kind `AGENTS.md` bans: it is a list
    /// of constructs deliberately not yet rendered, in the source beside the
    /// code that would render them, and it cannot drift because the two sweeps
    /// below fail the moment an entry is wrong in either direction. The ticket
    /// that lands a construct deletes its entry; when the list is empty every
    /// construct in the table renders as something.
    const UNRENDERED: [(&str, &str); 0] = [];

    /// Whether a construct's sample renders. The three questions are
    /// deliberately mechanical — see [`CONSTRUCTS`] for what that buys and
    /// what it costs. The kind check compares variants, not full values: a
    /// `List` row's exact marker or a `Quote` row's exact alert is what the
    /// dedicated tests below pin, and this sweep only needs to know a row of
    /// the right *kind* exists at all.
    fn unrendered(sample: &str, kind: RowKind) -> Option<String> {
        let rows = rows(sample, 200);
        if !rows.iter().any(|row| !row.text().trim().is_empty()) {
            return Some("renders no row with text on it".to_string());
        }
        if !rows
            .iter()
            .any(|row| std::mem::discriminant(&row.kind) == std::mem::discriminant(&kind))
        {
            return Some(format!("renders no {kind:?} row"));
        }
        // `Code` and `Metadata` rows are verbatim by design — a fence or a
        // frontmatter block is supposed to keep the author's own characters,
        // markers included, so a marker landing inside one is not a leak.
        // Raw HTML is what makes this matter: `</b> ` legitimately contains
        // `"> "`, the blockquote marker, with nothing left unconsumed.
        for marker in MARKERS {
            if let Some(row) = rows
                .iter()
                .filter(|row| !matches!(row.kind, RowKind::Code | RowKind::Metadata))
                .find(|row| row.text().contains(marker))
            {
                return Some(format!("leaks {marker:?} in {:?}", row.text()));
            }
        }
        None
    }

    /// The reason this test exists: four formatting gaps in this feature were
    /// found *by eye*, in a Preview of this repo's own documents, and two of
    /// them were named in R26.5 from the start and owned by no ticket at all.
    /// Nothing failed. R26.5b is that this cannot happen twice.
    #[test]
    fn every_construct_renders_something_or_is_a_recorded_omission() {
        let missing: Vec<String> = CONSTRUCTS
            .iter()
            .filter(|(name, ..)| !UNRENDERED.iter().any(|(owed, _)| owed == name))
            .filter_map(|(name, sample, kind)| {
                unrendered(sample, *kind).map(|why| format!("{name} {why}"))
            })
            .collect();
        assert!(
            missing.is_empty(),
            "these constructs are in neither the render nor the UNRENDERED \
             list: {missing:#?}"
        );
    }

    /// An omissions list nobody prunes is how the contract rots — the same
    /// reason `keys.rs` holds its `UNLISTED` to still naming live bindings. An
    /// entry for a construct that has started rendering excuses nothing, and
    /// one naming a construct the sweep does not drive excuses nothing either.
    #[test]
    fn the_unrendered_list_holds_only_constructs_that_still_do_not_render() {
        for (owed, ticket) in UNRENDERED {
            let (_, sample, kind) = CONSTRUCTS
                .iter()
                .find(|(name, ..)| *name == owed)
                .unwrap_or_else(|| panic!("{owed} is excused but the sweep does not drive it"));
            assert!(
                unrendered(sample, *kind).is_some(),
                "{owed} renders now, so its UNRENDERED entry is stale: {ticket}"
            );
        }
    }

    /// [`FLAGS`] is a hand-written table because a bitflags struct cannot be
    /// matched exhaustively, so this is what `block_kind`'s missing `_ =>` arm
    /// gets for free: a flag `pulldown-cmark` grows fails here until somebody
    /// has decided it, rather than defaulting to off in silence.
    #[test]
    fn every_parser_flag_has_a_decision() {
        let decided = FLAGS
            .iter()
            .fold(Options::empty(), |all, (flag, _, _)| all | *flag);
        assert_eq!(
            decided,
            Options::all(),
            "undecided: {:?}",
            Options::all() - decided
        );
        for (flag, _, reason) in FLAGS {
            assert!(!reason.is_empty(), "{flag:?} is decided with no reason");
        }
    }

    /// The reason this exists rather than trusting `every_parser_flag_has_a_decision`
    /// alone: that test only checks the *reason* is non-empty, so a `bool`
    /// flipped to `true` with its prose left unchanged — turning a construct
    /// on with nowhere to put what comes back — would pass it silently. This
    /// pins the fold `rows()` actually parses with, so flipping one without
    /// also giving its events a home in the same commit fails here first.
    #[test]
    fn strikethrough_tasklists_gfm_tables_yaml_and_toml_metadata_footnotes_definition_lists_and_heading_attributes_are_the_only_flags_on(
    ) {
        assert_eq!(
            options(),
            Options::ENABLE_STRIKETHROUGH
                | Options::ENABLE_TASKLISTS
                | Options::ENABLE_GFM
                | Options::ENABLE_TABLES
                | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
                | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
                | Options::ENABLE_FOOTNOTES
                | Options::ENABLE_DEFINITION_LIST
                | Options::ENABLE_HEADING_ATTRIBUTES
        );
    }

    #[test]
    fn a_heading_loses_its_hashes_and_keeps_its_level() {
        let rows = rows("## Install\n", 40);
        assert_eq!(kinds(&rows), vec![RowKind::Heading(HeadingLevel::H2)]);
        assert_eq!(texts(&rows), vec!["Install".to_string()]);
    }

    #[test]
    fn a_heading_level_is_the_run_of_hashes_that_opened_it() {
        let rows = rows("# One\n\n### Three\n", 40);
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::Heading(HeadingLevel::H1),
                RowKind::Paragraph,
                RowKind::Heading(HeadingLevel::H3),
            ]
        );
    }

    #[test]
    fn emphasis_strong_and_strikethrough_are_consumed_and_carried() {
        let rows = rows("Run **make** to *build* ~~fast~~.\n", 80);
        assert_eq!(texts(&rows), vec!["Run make to build fast.".to_string()]);
        let pieces = &rows[0].pieces;
        let make = pieces.iter().find(|piece| piece.text == "make").unwrap();
        assert!(make.emphasis.strong, "{pieces:?}");
        let build = pieces.iter().find(|piece| piece.text == "build").unwrap();
        assert!(build.emphasis.italic, "{pieces:?}");
        let fast = pieces.iter().find(|piece| piece.text == "fast").unwrap();
        assert!(fast.emphasis.struck, "{pieces:?}");
    }

    /// The one the wrapping rewrite is for: a styled piece long enough to be
    /// split by `textwrap` itself, not just to sit beside a break. `"bbbb
    /// cccc"` is one `Piece` (no event separates the two words inside
    /// `**...**`), and wrapping to 10 columns breaks between them — so both
    /// halves have to come back still `strong`, proving the split carries
    /// emphasis rather than dropping it at the row boundary.
    #[test]
    fn a_styled_piece_split_by_wrapping_carries_its_emphasis_into_both_halves() {
        let rows = rows("aaaa **bbbb cccc** dddd\n", 10);
        assert_eq!(
            texts(&rows),
            vec!["aaaa bbbb".to_string(), "cccc dddd".to_string()]
        );
        let bbbb = rows[0]
            .pieces
            .iter()
            .find(|piece| piece.text == "bbbb")
            .unwrap();
        assert!(bbbb.emphasis.strong, "{:?}", rows[0].pieces);
        let cccc = rows[1]
            .pieces
            .iter()
            .find(|piece| piece.text == "cccc")
            .unwrap();
        assert!(cccc.emphasis.strong, "{:?}", rows[1].pieces);
        let dddd = rows[1]
            .pieces
            .iter()
            .find(|piece| piece.text.contains("dddd"))
            .unwrap();
        assert!(!dddd.emphasis.strong, "{:?}", rows[1].pieces);
    }

    #[test]
    fn inline_code_is_a_piece_of_its_own() {
        let rows = rows("Call `main` first.\n", 80);
        assert_eq!(texts(&rows), vec!["Call main first.".to_string()]);
        let main = rows[0]
            .pieces
            .iter()
            .find(|piece| piece.text == "main")
            .unwrap();
        assert!(main.emphasis.code, "{:?}", rows[0].pieces);
    }

    /// The load-bearing one: every row of a wrapped paragraph names the line the
    /// paragraph started on, which is what the four consumers of the old
    /// `row = line + scroll` identity read instead of deriving it.
    #[test]
    fn every_row_of_a_wrapped_paragraph_carries_the_block_line() {
        let rows = rows("# Setup\n\none two three four five six seven eight\n", 18);
        let paragraph: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Paragraph && !row.text().is_empty())
            .collect();
        assert!(paragraph.len() > 1, "{:?}", texts(&rows));
        assert!(paragraph.iter().all(|row| row.line == 3), "{rows:?}");
    }

    #[test]
    fn a_soft_break_is_a_space_and_not_a_row() {
        let rows = rows("one\ntwo\n", 40);
        assert_eq!(texts(&rows), vec!["one two".to_string()]);
    }

    /// A hard break ends the row without ending the block: two rows, both
    /// still `Paragraph` and both still line 1, unlike a soft break's one row.
    #[test]
    fn a_hard_break_ends_the_row_and_a_soft_break_still_does_not() {
        let rows = rows("one  \ntwo\n", 40);
        assert_eq!(texts(&rows), vec!["one".to_string(), "two".to_string()]);
        assert!(rows.iter().all(|row| row.kind == RowKind::Paragraph));
        assert!(rows.iter().all(|row| row.line == 1), "{rows:?}");
    }

    /// A width of zero is no pane at all, not a pane one column wide: wrapping
    /// to it would put every word on a row of its own before the terminal has
    /// said anything about itself.
    #[test]
    fn no_width_yet_means_no_wrapping_yet() {
        let rows = rows("one two three four five six seven eight\n", 0);
        assert_eq!(
            texts(&rows),
            vec!["one two three four five six seven eight".to_string()]
        );
    }

    #[test]
    fn a_block_after_the_first_is_set_apart_from_it() {
        let rows = rows("# Setup\n\nInstall it.\n", 40);
        assert_eq!(
            texts(&rows),
            vec![
                "Setup".to_string(),
                String::new(),
                "Install it.".to_string()
            ]
        );
        assert_eq!(
            rows.iter().map(|row| row.line).collect::<Vec<_>>(),
            vec![1, 3, 3]
        );
        // The separator is prose whitespace, not a second heading: a blank row
        // that took the following block's kind would be counted by every
        // "there is 1 <kind> row" assertion the later tickets make.
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::Heading(HeadingLevel::H1),
                RowKind::Paragraph,
                RowKind::Paragraph
            ]
        );
    }

    #[test]
    fn markdown_is_the_two_extensions_and_nothing_else() {
        for file in ["README.md", "NOTES.markdown", "README.MD"] {
            assert!(is_markdown(Path::new(file)), "{file}");
        }
        for file in ["guide.mdx", "src/main.rs", "Makefile"] {
            assert!(!is_markdown(Path::new(file)), "{file}");
        }
    }

    #[test]
    fn bulleted_items_are_list_rows_marked_bullet_at_depth_one() {
        let rows = rows("- one\n- two\n", 80);
        assert_eq!(texts(&rows), vec!["one".to_string(), "two".to_string()]);
        for row in &rows {
            assert_eq!(
                row.kind,
                RowKind::List(ListItem {
                    depth: 1,
                    marker: Some(Marker::Bullet)
                }),
                "{rows:?}"
            );
        }
    }

    #[test]
    fn ordered_items_keep_their_ordinal() {
        // Starts at 3, not 1: a count from the item's position rather than a
        // read of the author's own numbering would pass a list starting at 1
        // and fail only here.
        let rows = rows("3. first\n4. second\n", 80);
        let ordinals: Vec<u64> = rows
            .iter()
            .map(|row| match row.kind {
                RowKind::List(ListItem {
                    marker: Some(Marker::Ordinal(n)),
                    ..
                }) => n,
                other => panic!("expected an ordinal, got {other:?}"),
            })
            .collect();
        assert_eq!(ordinals, vec![3, 4]);
    }

    /// The nested list ticket 03's own sample drives — a sibling item, then a
    /// deeper one nested under it. Order matters as much as depth: "two"'s
    /// own text is pending when its nested list opens, and has to land
    /// *before* "nested" rather than after it.
    #[test]
    fn a_nested_list_item_is_indented_deeper_than_its_parent() {
        let rows = rows("- one\n- two\n  - nested\n", 80);
        assert_eq!(
            texts(&rows),
            vec!["one".to_string(), "two".to_string(), "nested".to_string()]
        );
        let depths: Vec<usize> = rows
            .iter()
            .map(|row| match row.kind {
                RowKind::List(ListItem { depth, .. }) => depth,
                other => panic!("expected a list row, got {other:?}"),
            })
            .collect();
        assert_eq!(depths, vec![1, 1, 2]);
    }

    #[test]
    fn task_items_are_ticked_distinguishably_from_unticked() {
        let rows = rows("- [ ] todo\n- [x] done\n", 80);
        assert_eq!(texts(&rows), vec!["todo".to_string(), "done".to_string()]);
        assert_eq!(
            rows[0].kind,
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Task(false))
            })
        );
        assert_eq!(
            rows[1].kind,
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Task(true))
            })
        );
    }

    /// The marker belongs to the item, not to every row wrapping produces —
    /// a bullet repeated down two wrapped rows would read as two items.
    #[test]
    fn a_wrapped_list_item_carries_its_marker_only_once() {
        let rows = rows("- aaaa bbbb cccc dddd\n", 10);
        assert!(rows.len() > 1, "{rows:?}");
        assert_eq!(
            rows[0].kind,
            RowKind::List(ListItem {
                depth: 1,
                marker: Some(Marker::Bullet)
            })
        );
        for row in &rows[1..] {
            assert_eq!(
                row.kind,
                RowKind::List(ListItem {
                    depth: 1,
                    marker: None
                }),
                "{rows:?}"
            );
        }
    }

    #[test]
    fn a_block_quote_is_set_apart_from_the_prose_around_it() {
        let rows = rows("Intro.\n\n> quoted\n", 80);
        assert_eq!(
            texts(&rows),
            vec!["Intro.".to_string(), String::new(), "quoted".to_string()]
        );
        assert_eq!(
            kinds(&rows),
            vec![RowKind::Paragraph, RowKind::Paragraph, RowKind::Quote(None)]
        );
    }

    /// Each of GFM's five alert kinds, distinguishable from a plain quote and
    /// from each other, with its `[!KIND]` marker consumed rather than left
    /// as the first line of the quote's prose.
    #[test]
    fn every_gfm_alert_kind_is_distinguishable_and_its_marker_is_consumed() {
        let samples = [
            ("> [!NOTE]\n> mind this\n", BlockQuoteKind::Note),
            ("> [!TIP]\n> mind this\n", BlockQuoteKind::Tip),
            ("> [!IMPORTANT]\n> mind this\n", BlockQuoteKind::Important),
            ("> [!WARNING]\n> mind this\n", BlockQuoteKind::Warning),
            ("> [!CAUTION]\n> mind this\n", BlockQuoteKind::Caution),
        ];
        for (sample, kind) in samples {
            let rows = rows(sample, 80);
            assert!(
                rows.iter()
                    .any(|row| row.kind == RowKind::Quote(Some(kind))),
                "{sample:?} -> {rows:?}"
            );
            assert!(
                rows.iter().all(|row| !row.text().contains("[!")),
                "{sample:?} -> {rows:?}"
            );
        }
    }

    #[test]
    fn a_thematic_break_is_a_rule_row_set_apart_from_the_prose_around_it() {
        let rows = rows("one\n\n***\n\ntwo\n", 80);
        assert_eq!(
            texts(&rows),
            vec![
                "one".to_string(),
                String::new(),
                String::new(),
                String::new(),
                "two".to_string(),
            ]
        );
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::Paragraph,
                RowKind::Paragraph,
                RowKind::Rule,
                RowKind::Paragraph,
                RowKind::Paragraph,
            ]
        );
    }

    #[test]
    fn every_list_and_quote_row_carries_the_source_line_of_its_block() {
        let rows = rows("- one\n- two\n  - nested\n\n> quoted\n", 80);
        let lines: Vec<usize> = rows.iter().map(|row| row.line).collect();
        assert_eq!(texts(&rows).len(), lines.len());
        let one = &rows[texts(&rows).iter().position(|t| t == "one").unwrap()];
        assert_eq!(one.line, 1);
        let two = &rows[texts(&rows).iter().position(|t| t == "two").unwrap()];
        assert_eq!(two.line, 2);
        let nested = &rows[texts(&rows).iter().position(|t| t == "nested").unwrap()];
        assert_eq!(nested.line, 3);
        let quoted = &rows[texts(&rows).iter().position(|t| t == "quoted").unwrap()];
        assert_eq!(quoted.line, 5);
    }

    #[test]
    fn a_fenced_code_block_is_highlighted_in_its_language_and_unwrapped() {
        let rows = rows("```rust\nfn main() {}\n```\n", 5);
        let code: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Code)
            .collect();
        assert_eq!(code.len(), 1, "{rows:?}");
        assert_eq!(code[0].text(), "fn main() {}");
        let keyword = code[0]
            .pieces
            .iter()
            .find(|piece| piece.text == "fn")
            .unwrap_or_else(|| panic!("{:?}", code[0].pieces));
        assert_eq!(keyword.token, Some(highlight::Kind::Keyword));
    }

    #[test]
    fn a_fence_naming_no_language_is_still_one_unwrapped_plain_row() {
        let rows = rows(
            "```\na line that is far longer than the pane is wide\n```\n",
            5,
        );
        let code: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Code)
            .collect();
        assert_eq!(code.len(), 1, "{rows:?}");
        assert!(
            code[0]
                .pieces
                .iter()
                .all(|piece| piece.token == Some(highlight::Kind::Plain)),
            "{:?}",
            code[0].pieces
        );
    }

    #[test]
    fn a_fence_naming_an_unknown_language_renders_plain_rather_than_failing() {
        let rows = rows("```not-a-real-language\nx\n```\n", 40);
        let code: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Code)
            .collect();
        assert_eq!(code.len(), 1, "{rows:?}");
        assert_eq!(code[0].text(), "x");
    }

    #[test]
    fn an_indented_code_block_renders_the_same_way_as_a_fenced_one() {
        let rows = rows("    let x = 1;\n", 40);
        let code: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Code)
            .collect();
        assert_eq!(code.len(), 1, "{rows:?}");
        assert_eq!(code[0].text(), "let x = 1;");
    }

    #[test]
    fn a_multi_line_fence_keeps_one_row_per_source_line_and_no_wider_pane_would_merge_them() {
        let rows = rows("```\nline one\nline two\n```\n", 5);
        let code: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Code)
            .collect();
        assert_eq!(code.len(), 2, "{rows:?}");
        assert_eq!(code[0].text(), "line one");
        assert_eq!(code[1].text(), "line two");
    }

    #[test]
    fn fence_delimiters_contribute_no_row_and_the_code_rows_carry_the_fence_line() {
        let rows = rows("intro\n\n```rust\nfn main() {}\n```\n", 80);
        assert!(
            !rows.iter().any(|row| row.text().contains("```")),
            "{rows:?}"
        );
        let code = rows
            .iter()
            .find(|row| row.kind == RowKind::Code)
            .unwrap_or_else(|| panic!("{rows:?}"));
        assert_eq!(code.line, 3, "{rows:?}");
    }

    #[test]
    fn a_graph_fence_renders_as_a_diagram_holding_its_node_labels() {
        let rows = rows("```mermaid\ngraph LR; A[Build] --> B[Test]\n```\n", 76);
        assert!(!rows.is_empty(), "no rows");
        assert!(
            rows.iter().all(|row| row.kind == RowKind::Diagram),
            "{rows:?}"
        );
        assert!(rows.iter().all(|row| row.line == 1), "{rows:?}");
        let text: String = rows.iter().map(Row::text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Build"), "{text}");
        assert!(text.contains("Test"), "{text}");
    }

    /// A narrower pane re-lays the same source out differently — proof the
    /// diagram is not memoised past the width it was built for.
    #[test]
    fn a_diagram_relayouts_when_the_pane_resizes() {
        let source = "```mermaid\ngraph LR; A[Build] --> B[Test] --> C[Deploy]\n```\n";
        let wide = rows(source, 76);
        let narrow = rows(source, 20);
        assert_ne!(texts(&wide), texts(&narrow));
    }

    #[test]
    fn an_unsupported_diagram_type_is_refused_and_shown_as_code() {
        let rows = rows("```mermaid\nC4Context\n  title System\n```\n", 76);
        let code = rows
            .iter()
            .find(|row| row.kind == RowKind::Code)
            .unwrap_or_else(|| panic!("no code row in {rows:?}"));
        assert_eq!(code.refused, Some(DiagramRefusal::UnsupportedDiagram));
        assert_eq!(code.line, 1);
        assert!(code.text().contains("C4Context"), "{:?}", code.text());
    }

    #[test]
    fn a_malformed_diagram_is_refused_and_shown_as_code() {
        let rows = rows("```mermaid\npie\n  bad\n```\n", 76);
        let code = rows
            .iter()
            .find(|row| row.kind == RowKind::Code)
            .unwrap_or_else(|| panic!("no code row in {rows:?}"));
        assert_eq!(code.refused, Some(DiagramRefusal::MalformedDiagram));
        assert!(code.text().contains("bad"), "{:?}", code.text());
    }

    #[test]
    fn a_diagram_wider_than_the_pane_is_refused_and_shown_as_code() {
        let source = "```mermaid\ngraph LR; A[LongLabelHere] --> B[AnotherLongLabel] --> \
                       C[YetAnotherLabel] --> D[MoreLabelText]\n```\n";
        let rows = rows(source, 10);
        let code = rows
            .iter()
            .find(|row| row.kind == RowKind::Code)
            .unwrap_or_else(|| panic!("no code row in {rows:?}"));
        assert_eq!(code.refused, Some(DiagramRefusal::TooWide));
    }

    #[test]
    fn a_refusal_reason_is_the_words_named_here_not_the_crates() {
        assert_eq!(
            DiagramRefusal::UnsupportedDiagram.as_str(),
            "unsupported-diagram"
        );
        assert_eq!(
            DiagramRefusal::MalformedDiagram.as_str(),
            "malformed-diagram"
        );
        assert_eq!(DiagramRefusal::TooWide.as_str(), "diagram-too-wide");
    }

    #[test]
    fn a_table_aligns_columns_to_their_widest_cell() {
        let rows = rows("| a | bb |\n|---|---|\n| ccc | d |\n", 80);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert_eq!(table, vec!["a    bb".to_string(), "ccc  d ".to_string()]);
    }

    /// One column of each declared alignment, each padded against a body row
    /// wider than its own header — proving left pads on the right, right
    /// pads on the left, and centre splits the padding between both.
    #[test]
    fn each_columns_declared_alignment_is_honoured() {
        let rows = rows("| a | b | c |\n|:--|--:|:-:|\n| 11 | 222 | 3333 |\n", 80);
        let header = rows
            .iter()
            .find(|row| row.kind == RowKind::Table)
            .unwrap_or_else(|| panic!("{rows:?}"));
        assert_eq!(header.text(), "a     b   c  ");
    }

    /// Bold rather than a dashed rule — see [`table_rows`] for why no ticket
    /// asks for the glyph a rule would invent.
    #[test]
    fn a_tables_header_row_is_bold_and_the_body_is_not() {
        let rows = rows("| a |\n|---|\n| b |\n", 80);
        let table: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .collect();
        assert_eq!(table.len(), 2, "{rows:?}");
        assert!(
            table[0].pieces.iter().all(|piece| piece.emphasis.strong),
            "{:?}",
            table[0].pieces
        );
        assert!(
            !table[1].pieces.iter().any(|piece| piece.emphasis.strong),
            "{:?}",
            table[1].pieces
        );
    }

    /// The whole of every cell survives a pane too narrow for the table: the
    /// text moves onto further display rows instead of being cut away, which
    /// is what makes it reachable at all — a truncated cell is gone from
    /// `Row::text()` and no scroll can bring it back.
    #[test]
    fn a_table_wider_than_the_pane_wraps_its_cells_rather_than_dropping_text() {
        let rows = rows("| alpha beta | gamma delta |\n|---|---|\n| c | d |\n", 16);
        let table: String = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect::<Vec<_>>()
            .join("\n");
        for word in ["alpha", "beta", "gamma", "delta", "c", "d"] {
            assert!(table.contains(word), "{word:?} lost in {table:?}");
        }
    }

    /// A source row is as tall as its tallest cell, and its shorter cells are
    /// padded down that whole height — otherwise the column after a wrapped
    /// one would shift left on every row but the first.
    #[test]
    fn a_wrapped_source_rows_columns_stay_aligned_down_its_whole_height() {
        let rows = rows("| one two three | x |\n|---|---|\n| a | b |\n", 12);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert!(table.len() > 2, "nothing wrapped: {table:?}");
        let width = table[0].width();
        assert!(
            table.iter().all(|row| row.width() == width),
            "ragged: {table:?}"
        );
        let column = table[0].find('x').unwrap_or_else(|| panic!("{table:?}"));
        assert_eq!(
            table[1].char_indices().nth(column).map(|(_, ch)| ch),
            Some(' '),
            "second column moved: {table:?}"
        );
    }

    /// Alignment is a property of the column, so it applies to a wrapped
    /// cell's continuation rows exactly as it does to its first.
    #[test]
    fn a_wrapped_cells_alignment_is_honoured_on_every_display_row() {
        let rows = rows("| h |\n|--:|\n| one two |\n", 5);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert!(table.len() > 2, "nothing wrapped: {table:?}");
        for row in &table[1..] {
            assert!(row.starts_with(' '), "not right-aligned: {row:?}");
            assert!(!row.ends_with(' '), "not right-aligned: {row:?}");
        }
    }

    /// The regression this change is likeliest to cause: a table the pane has
    /// room for is laid out exactly as it was before wrapping existed.
    #[test]
    fn a_table_that_fits_the_pane_is_one_display_row_per_source_row() {
        let rows = rows("| a | bb |\n|---|---|\n| ccc | d |\n| e | ff |\n", 80);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert_eq!(
            table,
            vec![
                "a    bb".to_string(),
                "ccc  d ".to_string(),
                "e    ff".to_string()
            ]
        );
    }

    /// `columns` of zero is a size the terminal has not reported yet, so
    /// nothing shrinks and nothing wraps — the same convention [`rows`] uses.
    #[test]
    fn a_table_laid_out_against_no_reported_size_wraps_nothing() {
        let rows = rows("| alpha beta | gamma |\n|---|---|\n| c | d |\n", 0);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert_eq!(
            table,
            vec![
                "alpha beta  gamma".to_string(),
                "c           d    ".to_string()
            ]
        );
    }

    /// A token with nowhere to break is the one case wrapping cannot help.
    /// It overflows its column rather than being cut: an overflowing row is
    /// clipped at render and recoverable by scrolling sideways, where a cut
    /// one is gone for good.
    #[test]
    fn an_unbreakable_token_wider_than_its_column_overflows_rather_than_truncating() {
        let rows = rows("| aaaaaaaaaaaaaaaaaaaa |\n|---|\n| b |\n", 6);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert!(
            table.iter().any(|row| row.contains("aaaaaaaaaaaaaaaaaaaa")),
            "{table:?}"
        );
    }

    /// A pane too narrow for even the floor stops shrinking there rather than
    /// squeezing on down to a column of single letters: the cell wraps at four
    /// and the row overflows, which is the trade [`MIN_COLUMN`] names.
    #[test]
    fn a_column_stops_shrinking_at_the_narrowest_width_still_worth_wrapping_to() {
        let rows = rows("| ab cd efg | cc |\n|---|---|\n| x | y |\n", 3);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert_eq!(table[0], "ab    cc", "{table:?}");
        assert!(
            table.iter().all(|row| row.width() == 8),
            "ragged: {table:?}"
        );
    }

    /// A wrapped cell keeps its emphasis on every row it occupies, exactly as
    /// a wrapped paragraph's split piece does.
    #[test]
    fn a_wrapped_cell_carries_its_emphasis_onto_its_continuation_rows() {
        let rows = rows("| **one two** |\n|---|\n| x |\n", 5);
        let table: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .collect();
        assert!(table.len() > 2, "nothing wrapped: {rows:?}");
        let carried = table[1]
            .pieces
            .iter()
            .find(|piece| piece.text.trim() == "two")
            .unwrap_or_else(|| panic!("{:?}", table[1].pieces));
        assert!(carried.emphasis.strong, "{:?}", table[1].pieces);
    }

    /// Two CJK glyphs are two display columns wide each — four total — same
    /// as `aaaa`, so both columns land at the same width instead of one
    /// counted short on characters and overflowing the pane it was measured
    /// against.
    #[test]
    fn column_widths_are_measured_in_display_columns_not_characters() {
        let rows = rows("| 中文 |\n|---|\n| aaaa |\n", 80);
        let table: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .collect();
        assert_eq!(table.len(), 2, "{rows:?}");
        assert_eq!(table[0].text().width(), table[1].text().width(), "{rows:?}");
    }

    /// A display-column budget does not always divide evenly by a
    /// double-width glyph, so a wrapped CJK cell's row can land one column
    /// short of its column's width. Left unpadded, that shortfall drags every
    /// column after it one to the left on just that row — comparing every
    /// display row's total width against the others is what catches the drift.
    #[test]
    fn a_wrapped_wide_glyph_cells_rows_are_padded_back_up_to_its_column_width() {
        let rows = rows("| 中文文 | b |\n|---|---|\n| x | y |\n", 8);
        let table: Vec<String> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .map(Row::text)
            .collect();
        assert!(table.len() > 2, "nothing wrapped: {table:?}");
        let width = table[0].width();
        assert!(
            table.iter().all(|row| row.width() == width),
            "ragged: {table:?}"
        );
    }

    #[test]
    fn a_tables_rows_carry_the_source_line_the_table_started_on() {
        let rows = rows("intro\n\n| a |\n|---|\n| b |\n| c |\n", 80);
        let table: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Table)
            .collect();
        assert_eq!(table.len(), 3, "{rows:?}");
        assert!(table.iter().all(|row| row.line == 3), "{rows:?}");
    }

    #[test]
    fn no_row_anywhere_holds_a_list_quote_alert_or_rule_marker() {
        let rows = rows(
            "- [ ] todo\n- [x] done\n  - nested\n\n> [!NOTE]\n> mind this\n\n***\n",
            80,
        );
        for marker in ["- ", "> ", "[!", "[ ]", "[x]"] {
            assert!(
                rows.iter().all(|row| !row.text().contains(marker)),
                "{marker:?} leaked in {rows:?}"
            );
        }
    }

    /// Every spelling that arrives as `Tag::Link` with a different
    /// `LinkType` — inline, full reference, collapsed reference, shortcut
    /// reference — and the code never inspects which one it is: `Tag::Link`
    /// is inline in [`block_kind`] regardless, so the text between the tags
    /// is what shows and the `dest_url` field is never touched. A rule that
    /// only handled the inline spelling would leave the rest showing their
    /// URLs; this pins that no spelling is special-cased into that gap.
    #[test]
    fn every_link_spelling_hides_its_url() {
        let rows = rows(
            "[inline](https://example.com/inline) and [full][r] and \
             [coll][] and [short].\n\n\
             [r]: https://example.com/ref\n\
             [coll]: https://example.com/coll\n\
             [short]: https://example.com/short\n",
            200,
        );
        // Exact equality rather than `contains` — "ref" or "coll" would also
        // be true as a substring of the URL each is meant to prove is gone,
        // which would pass even if the URL leaked right next to it.
        assert_eq!(
            texts(&rows),
            vec!["inline and full and coll and short.".to_string()]
        );
    }

    /// An autolink's visible text *is* its target — `<https://…>` and a bare
    /// GFM autolink both show the author's own literal characters, so there
    /// is nothing to hide, unlike every other spelling above. A bare URL with
    /// no brackets is not one of these: `pulldown-cmark` 0.13.4's `ENABLE_GFM`
    /// only turns on `[!NOTE]`-style alert parsing (see its own doc comment
    /// on `Tag::BlockQuote`), not GitHub's separate "extended autolinks" —
    /// there is no bare-URL `LinkType` this crate can produce for any flag
    /// combination to leave un-driven, so `https://example.com/bare` below
    /// stays plain `Event::Text`, same as any other word in the sentence.
    #[test]
    fn an_autolink_shows_its_url_because_that_is_its_own_text() {
        let rows = rows("See <https://example.com/auto> for more.\n", 200);
        let text = texts(&rows).join(" ");
        assert!(text.contains("https://example.com/auto"), "{text}");
    }

    #[test]
    fn an_image_renders_its_alt_text_marked_as_an_image() {
        let rows = rows("![a cat](cat.png)\n", 200);
        assert_eq!(texts(&rows), vec!["a cat".to_string()]);
        let piece = rows[0]
            .pieces
            .iter()
            .find(|piece| piece.text.contains("cat"))
            .unwrap();
        assert!(piece.emphasis.image, "{:?}", rows[0].pieces);
        assert!(!rows[0].text().contains("cat.png"), "{:?}", rows[0]);
    }

    #[test]
    fn inline_html_is_shown_literally_and_set_apart() {
        let rows = rows("Press <kbd>x</kbd> to exit.\n", 200);
        assert_eq!(
            texts(&rows),
            vec!["Press <kbd>x</kbd> to exit.".to_string()]
        );
        let tag = rows[0]
            .pieces
            .iter()
            .find(|piece| piece.text == "<kbd>")
            .unwrap();
        assert!(tag.emphasis.code, "{:?}", rows[0].pieces);
    }

    #[test]
    fn a_raw_html_block_is_shown_literally_and_unwrapped() {
        let rows = rows("<div>\n  <p>hi</p>\n</div>\n", 10);
        assert!(
            kinds(&rows).iter().all(|kind| *kind == RowKind::Code),
            "{rows:?}"
        );
        let text = texts(&rows).join("\n");
        assert!(text.contains("<div>"), "{text}");
        assert!(text.contains("<p>hi</p>"), "{text}");
        assert!(text.contains("</div>"), "{text}");
    }

    #[test]
    fn yaml_frontmatter_renders_as_metadata_rows_set_apart() {
        let rows = rows("---\ntitle: Setup\nauthor: me\n---\n\nProse.\n", 80);
        assert_eq!(kinds(&rows)[0], RowKind::Metadata);
        assert_eq!(*kinds(&rows).last().unwrap(), RowKind::Paragraph);
        let text = texts(&rows).join("\n");
        assert!(text.contains("title: Setup"), "{text}");
        assert!(text.contains("author: me"), "{text}");
        assert!(!text.contains("---"), "{text}");
    }

    /// `pulldown-cmark` itself refuses to open a metadata block that never
    /// finds a closing `---`, falling back to ordinary parsing — this pins
    /// that the fallback keeps every row after it rather than swallowing the
    /// rest of the document into an unclosed block.
    #[test]
    fn frontmatter_with_no_closing_marker_does_not_swallow_the_document() {
        let rows = rows("---\ntitle: Setup\n\nStill here.\n", 80);
        assert!(
            !rows.iter().any(|row| row.kind == RowKind::Metadata),
            "{rows:?}"
        );
        let text = texts(&rows).join(" ");
        assert!(text.contains("Still here."), "{text}");
    }

    #[test]
    fn toml_frontmatter_renders_as_metadata_rows_the_same_as_yamls() {
        let rows = rows("+++\ntitle = \"Setup\"\n+++\n\nProse.\n", 80);
        assert_eq!(kinds(&rows)[0], RowKind::Metadata);
        assert_eq!(*kinds(&rows).last().unwrap(), RowKind::Paragraph);
        let text = texts(&rows).join("\n");
        assert!(text.contains("title = \"Setup\""), "{text}");
        assert!(!text.contains("+++"), "{text}");
    }

    #[test]
    fn a_heading_attribute_is_consumed_rather_than_shown_as_prose() {
        let rows = rows("## Install {#install}\n", 80);
        assert_eq!(kinds(&rows), vec![RowKind::Heading(HeadingLevel::H2)]);
        assert_eq!(texts(&rows), vec!["Install".to_string()]);
    }

    #[test]
    fn a_footnote_reference_becomes_its_number_and_the_definition_carries_the_same_one() {
        let rows = rows("Text[^1].\n\n[^1]: a note\n", 80);
        let text = texts(&rows).join("\n");
        assert!(text.contains("Text[1]."), "{text}");
        assert!(text.contains("[1] a note"), "{text}");
        assert!(!text.contains("[^1]"), "{text}");
    }

    /// Numbering follows the order references appear in prose, not the order
    /// definitions were written — the reader meets `[1]` before `[2]` in
    /// text even though this document defines them the other way round.
    #[test]
    fn footnotes_are_numbered_in_reference_order_not_definition_order() {
        let rows = rows("One[^b] and two[^a].\n\n[^a]: second label, used first\n[^b]: first label, used second\n", 80);
        let text = texts(&rows).join("\n");
        assert!(text.contains("One[1]"), "{text}");
        assert!(text.contains("two[2]"), "{text}");
        assert!(text.contains("[1] first label, used second"), "{text}");
        assert!(text.contains("[2] second label, used first"), "{text}");
    }

    #[test]
    fn a_definition_lists_term_and_definition_are_list_rows_the_definition_indented_deeper() {
        let rows = rows("term\n: meaning\n", 80);
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::List(ListItem {
                    depth: 0,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 1,
                    marker: None
                }),
            ]
        );
        assert_eq!(
            texts(&rows),
            vec!["term".to_string(), "meaning".to_string()]
        );
    }

    /// A loose definition — one blank line between the term and `:` — wraps
    /// its content in a real `Tag::Paragraph`, the same way a loose list item
    /// does. Pinned because the paragraph-opening arm must treat that
    /// wrapper as transparent or the definition renders as an ordinary
    /// paragraph at depth 0, indistinguishable from prose.
    #[test]
    fn a_loose_definitions_paragraph_still_indents_under_its_term() {
        let rows = rows(
            "Apple\n\n:   Pomaceous fruit.\n\nOrange\n\n:   Citrus fruit.\n",
            80,
        );
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::List(ListItem {
                    depth: 0,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 1,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 0,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 1,
                    marker: None
                }),
            ]
        );
        assert_eq!(
            texts(&rows),
            vec![
                "Apple".to_string(),
                "Pomaceous fruit.".to_string(),
                "Orange".to_string(),
                "Citrus fruit.".to_string(),
            ]
        );
    }

    /// A definition list nested inside another's definition must indent
    /// deeper than its own outer term — pinned because depth used to be
    /// derived from the tag alone (0 for a title, 1 for a definition,
    /// regardless of nesting), which drew every nested term as a sibling of
    /// the outer one.
    #[test]
    fn a_nested_definition_list_indents_deeper_than_its_outer_term() {
        let rows = rows("outer\n: inner term\n  : inner def\n", 80);
        assert_eq!(
            kinds(&rows),
            vec![
                RowKind::List(ListItem {
                    depth: 0,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 2,
                    marker: None
                }),
                RowKind::List(ListItem {
                    depth: 3,
                    marker: None
                }),
            ]
        );
        assert_eq!(
            texts(&rows),
            vec![
                "outer".to_string(),
                "inner term".to_string(),
                "inner def".to_string(),
            ]
        );
    }

    #[test]
    fn superscript_subscript_and_math_flags_stay_off_so_their_characters_survive() {
        let superscript = rows("x^2^ and H~2~O\n", 80);
        let text = texts(&superscript).join(" ");
        assert!(text.contains("x^2^"), "{text}");
        assert!(text.contains("H~2~O"), "{text}");

        let math = rows("$x^2$ and $$y = mx + b$$\n", 80);
        let text = texts(&math).join(" ");
        assert!(text.contains("$x^2$"), "{text}");
        assert!(text.contains("$$y = mx + b$$"), "{text}");
    }

    /// `ENABLE_STRIKETHROUGH` (ticket 12) strikes a single pair of tildes
    /// too, and it cannot tell a struck word from a subscript by the flag
    /// alone — only CommonMark's flanking rule decides. `H~2~O`'s tildes
    /// touch a letter on both sides and stay literal; a tilde run set off by
    /// whitespace is read as strikethrough regardless, which is a known
    /// interaction recorded on `ENABLE_SUBSCRIPT`, not a regression this
    /// ticket owns.
    #[test]
    fn a_whitespace_flanked_subscript_is_struck_because_strikethrough_claims_it_first() {
        let rows = rows("log ~2~ n is fine\n", 80);
        let text = texts(&rows).join(" ");
        assert!(text.contains("log 2 n is fine"), "{text}");
    }

    #[test]
    fn a_wikilink_stays_its_own_brackets_rather_than_becoming_a_link() {
        let rows = rows("See [[Setup]] for more.\n", 80);
        let text = texts(&rows).join(" ");
        assert!(text.contains("[[Setup]]"), "{text}");
    }
}
