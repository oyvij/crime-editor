//! Turning a mouse event into events.
//!
//! Pure, like [`crate::keys`]. The one thing it cannot do is read characters off
//! the terminal's screen, so a drag that selects text comes back as a
//! [`Selection`] request for the edge to fulfil.

use crate::layout::{self, Area, Layout};
use crate::{tree, Direction, Event, Modal, Pane, Place, State};
use terminput::KeyModifiers;

/// Which mouse-report encoding the program in a hosted pane asked for, as the
/// terminal model in front of its pty reports it. A report in any other
/// encoding is text the child cannot parse, and it lands in its prompt as
/// literal characters — which is the phantom text CRIME used to leave behind by
/// sending SGR to every child that asked for the mouse at all.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    None,
    /// `ESC [ M` and one byte per field, each offset by 32.
    Legacy,
    /// `ESC [ <` and decimal fields, with no coordinate limit.
    Sgr,
}

/// What the child is being told happened. A click is press *and* release
/// together: a child left holding a button reads the next move as a drag of its
/// own. The wheel has no release — xterm never sends one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Click,
    Wheel(Direction),
}

/// The largest coordinate the legacy encoding can express: its fields are one
/// byte offset by 32, so 223 is the last cell that has a representation at all.
const LEGACY_LIMIT: usize = 223;

/// The bytes a child expecting `encoding` reads as `gesture` on the cell `at` of
/// its own grid, or nothing when it cannot be told: the child asked for no
/// mouse, or the legacy encoding has no byte for that cell. Declining is
/// deliberate — a wrapped coordinate names a different cell, and a child acting
/// on the wrong cell is worse than one that heard nothing.
pub fn report(encoding: Encoding, gesture: Gesture, at: Place) -> Option<Vec<u8>> {
    // xterm's wheel buttons, exhaustive over the directions rather than
    // catch-all: a sideways swipe answered with 65 is a child told to scroll
    // down, which is the substitution this function exists to refuse.
    let button: u8 = match gesture {
        Gesture::Click => 0,
        Gesture::Wheel(Direction::Up) => 64,
        Gesture::Wheel(Direction::Down) => 65,
        Gesture::Wheel(Direction::Left) => 66,
        Gesture::Wheel(Direction::Right) => 67,
    };
    let released = matches!(gesture, Gesture::Click);
    match encoding {
        Encoding::None => None,
        Encoding::Sgr => {
            let (column, line) = (at.column, at.line);
            let mut bytes = format!("\x1b[<{button};{column};{line}M").into_bytes();
            if released {
                bytes.extend_from_slice(format!("\x1b[<{button};{column};{line}m").as_bytes());
            }
            Some(bytes)
        }
        Encoding::Legacy => {
            let (column, line) = (offset(at.column)?, offset(at.line)?);
            let mut bytes = vec![0x1b, b'[', b'M', 32 + button, column, line];
            if released {
                // The legacy encoding has no separate release: button 3 is it.
                bytes.extend_from_slice(&[0x1b, b'[', b'M', 32 + 3, column, line]);
            }
            Some(bytes)
        }
    }
}

fn offset(coordinate: usize) -> Option<u8> {
    match (1..=LEGACY_LIMIT).contains(&coordinate) {
        true => Some(32 + coordinate as u8),
        false => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    LeftDown,
    LeftDrag,
    LeftUp,
    RightDown,
    ScrollUp,
    ScrollDown,
    /// A trackpad swipe or a tilt wheel. Named rather than folded into the two
    /// above: the surfaces that answer it have an offset of their own, and the
    /// conversion in `main.rs` used to drop every sideways report on a
    /// catch-all arm, which is the shape AGENTS.md forbids for keys and forbids
    /// here for the same reason.
    ScrollLeft,
    ScrollRight,
    /// The pointer moving with nothing held down, reported only because the
    /// edge asks the terminal for motion. It presses nothing: it is where the
    /// pointer comes to rest — the one gesture nobody makes — and it is what
    /// turns a name under a held jump modifier into a link and takes the link
    /// away again.
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pub kind: Kind,
    pub column: u16,
    pub row: u16,
    /// What was held while it happened, as the terminal reported it. The whole
    /// set rather than a flag: the same lossless-type reason `keys` takes a
    /// `KeyEvent`, and the edge is not the place to decide which modifier
    /// means what.
    pub modifiers: KeyModifiers,
}

/// Whether the modifier that turns a click into a jump is held. Cmd is the
/// gesture — VS Code's on macOS — and Ctrl is the same gesture everywhere
/// else, and both are accepted because the mouse protocol has bits for shift,
/// alt and ctrl and none for Super: a Cmd+click arrives with no modifier at
/// all on most terminals, so binding Cmd alone would be a gesture nobody's
/// terminal can send. `gd` remains the way to reach it with no modifier.
fn jumping(modifiers: KeyModifiers) -> bool {
    modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::CTRL)
}

/// What a drag covers, for the edge to turn into text: a span in the pane's own
/// text coordinates — a buffer's lines and columns, or cells in a pty's visible
/// grid. Ordered, so dragging upward covers what dragging downward covers, and
/// the edge only has to read characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub pane: Pane,
    pub from: Place,
    pub to: Place,
}

/// Which pane edge a drag has hold of. Two handles now, so which one is not a
/// pair of booleans that could both be true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    /// The tree's right border, which moves the tree/editor boundary.
    Tree,
    /// The AI pane's left border, which moves the editor/AI boundary.
    Ai,
}

/// Where a drag began, and which divider it grabbed. Edge state, but the
/// routing needs it, so it lives with the routing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    pub dragging: Option<Divider>,
    pub drag_from: Option<(u16, u16)>,
    /// Whether the pointer moved while the button was down. A child's click is
    /// sent on release and only if it did not, so dragging to select text never
    /// presses whatever the drag started on.
    pub dragged: bool,
    /// When the report being routed arrived, off the edge's clock. Told rather
    /// than remembered, the way `keys::on_key_event` is told the same
    /// millisecond: only the edge can read a clock, and a test supplies
    /// whatever it needs so no timing test ever waits.
    pub at_ms: u64,
    /// Where and when the last press landed, so a second press on the same cell
    /// inside the double-tap window is one gesture rather than two.
    last_press: Option<(u16, u16, u64)>,
}

// No `Eq`: an `Event` carries a speed, which is a float.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Outcome {
    pub events: Vec<Event>,
    pub select: Option<Selection>,
    /// A click with the jump modifier held on a pty's cell: the edge reads the
    /// row it is on and hands it back as `Event::ClickLink`, because whether
    /// there is a URL under the pointer is written on the child's grid.
    pub link: Option<(Pane, Place)>,
}

impl Outcome {
    fn of(events: Vec<Event>) -> Self {
        Self {
            events,
            select: None,
            link: None,
        }
    }
}

pub fn on_mouse(state: &State, panes: &Layout, pointer: &mut Pointer, input: Input) -> Outcome {
    // Before any pane is chosen, because where the pointer *is* is not a press
    // in a pane: a move over anything but the editor's text is the pointer
    // leaving, which is what takes a box it rested for down.
    if input.kind == Kind::Moved {
        let mut events = vec![Event::PointerMoved(resting(state, panes, input))];
        events.extend(hovered(state, panes, input));
        let icon = action_under(state, panes, input);
        if icon != state.hovered_action {
            events.push(Event::HoverAction(icon));
        }
        // Against the strip the renderer draws, so what lights is what the
        // pointer is on. A zero-width strip answers `false` for every column,
        // so a hidden mirror needs no arm of its own here.
        let on_minimap = crate::minimap::strip(state, panes.editor).holds(input.column, input.row);
        if on_minimap != state.hovered_minimap {
            events.push(Event::HoverMinimap(on_minimap));
        }
        return Outcome::of(events);
    }
    // The results box covers every pane, so a press in one is a press on
    // something nobody can see: a click in the tree behind it opened whatever
    // file was under the box, and a drag picked text out of a pane the box was
    // drawn over. What the box itself does with a press is all that is left,
    // and the wheel is the exception that goes on — `update` gives it to the
    // box, for the same reason the box has the keyboard. Nothing is left
    // holding a divider or a half-finished drag either.
    if let Some(search) = state.search.as_ref() {
        if !matches!(
            input.kind,
            Kind::ScrollUp | Kind::ScrollDown | Kind::ScrollLeft | Kind::ScrollRight
        ) {
            *pointer = Pointer::default();
            return Outcome::of(result_click(state, search, input));
        }
    }
    if let Some(outcome) = divider_drag(panes, pointer, input) {
        return outcome;
    }
    let Some(pane) = layout::pane_at(panes, input.column, input.row) else {
        return Outcome::default();
    };
    in_pane(state, panes, pointer, pane, input)
}

/// The hit a press in the results box lands on. Nothing for a file heading, for
/// the query above the list, or for the borders around it: the nearest hit is a
/// different file opened by the `Enter` that follows. The row is read against
/// `search.scroll` — the very field the clamp and the renderer read — so the
/// hit a click marks is the one under the pointer wherever the wheel has left
/// the list.
fn result_click(state: &State, search: &crate::Search, input: Input) -> Vec<Event> {
    let area = layout::search_box(state.screen_width, state.screen_height);
    let top = area.y + 1 + layout::SEARCH_HEADER;
    let offset = match input.kind {
        Kind::LeftDown if input.row >= top && area.holds(input.column, input.row) => {
            usize::from(input.row - top)
        }
        _ => return vec![],
    };
    // The box draws only as many rows as the layout leaves it, so a press below
    // the last of them is on the bottom border — inside the rectangle, and part
    // of no row.
    if offset >= layout::search_hit_rows(state.screen_width, state.screen_height) {
        return vec![];
    }
    match crate::search::rows(&search.results).get(offset + search.scroll) {
        Some(crate::search::Row::Hit(index)) => vec![Event::SelectHit(*index)],
        _ => vec![],
    }
}

/// A pane's border is the handle. Checked before anything else, because those
/// columns belong to a pane and would otherwise read as clicking a row. Only
/// where the border is, though: the terminal spans every column underneath, so
/// a handle that ignored the row would eat two of its columns.
fn divider_drag(panes: &Layout, pointer: &mut Pointer, input: Input) -> Option<Outcome> {
    let tree_edge = panes.tree.x + panes.tree.width.saturating_sub(1);
    let ai_edge = panes.ai.x;
    // How wide the screen is. Read off the AI pane rather than the terminal,
    // which spans it only while the AI pane stops above it — a tall one takes
    // those columns, and clamping against what was left would shrink both
    // handles' range every time somebody asked for it.
    let screen = panes.ai.right();
    // The AI pane's border runs as far down as the pane does, which in the
    // tall shape is past the terminal's first row.
    let beside_tree = input.row < panes.terminal.y;
    let beside_ai = input.row < panes.ai.bottom();
    match input.kind {
        Kind::LeftDown if beside_tree && input.column.abs_diff(tree_edge) <= 1 => {
            pointer.dragging = Some(Divider::Tree);
            Some(Outcome::default())
        }
        Kind::LeftDown if beside_ai && input.column.abs_diff(ai_edge) <= 1 => {
            pointer.dragging = Some(Divider::Ai);
            Some(Outcome::default())
        }
        Kind::LeftDrag if pointer.dragging == Some(Divider::Tree) => {
            let width = input.column.saturating_sub(panes.tree.x) + 1;
            let most = screen.saturating_sub(30).max(12);
            Some(Outcome::of(vec![Event::DragDivider(u32::from(
                width.clamp(12, most),
            ))]))
        }
        Kind::LeftDrag if pointer.dragging == Some(Divider::Ai) => {
            let width = panes.ai.right().saturating_sub(input.column);
            // The editor keeps its 20 columns, which is the same floor the
            // layout enforces — clamping here as well is what stops a drag
            // past it from being remembered as a width nobody can see. The
            // step-menu's width is taken out of that floor too, the same
            // way the layout takes it out of the editor's.
            let most = screen
                .saturating_sub(panes.tree.width + panes.step_menu.width + 20)
                .max(12);
            Some(Outcome::of(vec![Event::DragAiDivider(u32::from(
                width.clamp(12, most),
            ))]))
        }
        Kind::LeftUp => {
            pointer.dragging = None;
            None
        }
        _ => None,
    }
}

fn in_pane(
    state: &State,
    panes: &Layout,
    pointer: &mut Pointer,
    pane: Pane,
    input: Input,
) -> Outcome {
    match input.kind {
        Kind::LeftDown => {
            // Where the button went down is where a selection starts. Waiting
            // for the first drag report anchors it a character late.
            pointer.drag_from = Some((input.column, input.row));
            pointer.dragged = false;
            let doubled = pointer.last_press.is_some_and(|(column, row, ms)| {
                (column, row) == (input.column, input.row)
                    && pointer.at_ms.saturating_sub(ms) <= state.double_tap_ms
            });
            pointer.last_press = Some((input.column, input.row, pointer.at_ms));
            let mut events = pressed(state, panes, pane, input);
            // The place the press already named, rather than a hit-test of its
            // own: a double-click picks a word wherever a single click takes
            // the caret, and two hit-tests are how a click and what it selects
            // come to disagree about the cell under the pointer.
            if let (true, [Event::ClickText(at)]) = (doubled, events.as_slice()) {
                events.push(Event::DoubleClickText(*at));
            }
            Outcome::of(events)
        }
        Kind::RightDown => Outcome::of(vec![Event::RightClick(pane)]),
        Kind::ScrollUp => Outcome::of(vec![Event::Scroll {
            pane,
            direction: Direction::Up,
            at: place_in(state, panes, pane, (input.column, input.row)),
        }]),
        Kind::ScrollDown => Outcome::of(vec![Event::Scroll {
            pane,
            direction: Direction::Down,
            at: place_in(state, panes, pane, (input.column, input.row)),
        }]),
        Kind::ScrollLeft => Outcome::of(vec![Event::Scroll {
            pane,
            direction: Direction::Left,
            at: place_in(state, panes, pane, (input.column, input.row)),
        }]),
        Kind::ScrollRight => Outcome::of(vec![Event::Scroll {
            pane,
            direction: Direction::Right,
            at: place_in(state, panes, pane, (input.column, input.row)),
        }]),
        Kind::LeftUp => {
            // A press and a release with nothing in between is a click, and a
            // click is the only thing a child gets: CRIME owns drags in its
            // panes, so forwarding the press as it happened would activate
            // whatever a text selection started on.
            let tapped = !pointer.dragged;
            pointer.drag_from = None;
            pointer.dragged = false;
            let at = place_in(state, panes, pane, (input.column, input.row));
            // The jump modifier's click on a pty is a question about the text
            // under it, not a click for the child — the same gesture that
            // follows a name to its definition in the editor.
            let hosted = matches!(pane, Pane::Terminal | Pane::Ai);
            match (tapped, hosted && jumping(input.modifiers)) {
                (true, true) => Outcome {
                    events: Vec::new(),
                    select: None,
                    link: Some((pane, at)),
                },
                (true, false) => Outcome::of(vec![Event::ClickThrough { pane, at }]),
                (false, _) => Outcome::default(),
            }
        }
        Kind::LeftDrag => {
            pointer.dragged = true;
            dragged(state, panes, pointer, pane, input)
        }
        // Answered in `on_mouse`, before a pane was chosen: where the pointer
        // is is not a press in a pane.
        Kind::Moved => Outcome::default(),
    }
}

/// The link the pointer is on, as the change to what the state already names:
/// nothing to report while it stays on the same place, because a pointer
/// crossing a pane sends a report per cell and each one that reached `update`
/// would be a frame.
fn hovered(state: &State, panes: &Layout, input: Input) -> Vec<Event> {
    // The editor's own text only, and the border row the Transport lives on is
    // not text — the same rows `pressed` reads as places, and the same ones
    // `resting` answers for.
    let at = match jumping(input.modifiers) {
        true => resting(state, panes, input),
        false => None,
    };
    match at == state.link {
        true => vec![],
        false => vec![Event::HoverLink(at)],
    }
}

fn pressed(state: &State, panes: &Layout, pane: Pane, input: Input) -> Vec<Event> {
    if let Some(key) = palette_entry_at(state, panes, input.column, input.row) {
        return vec![Event::ClickPaletteEntry(key)];
    }
    if let Some(index) = buffer_dot_at(state, panes, input.column, input.row) {
        return match state.buffers.keys().nth(index) {
            Some(path) => vec![Event::ShowBuffer(path.clone())],
            None => vec![],
        };
    }
    let row_index = row_index(state, panes, input.row);
    match (pane, action_at(state, panes, input.column, input.row)) {
        (Pane::Tree, Some(action)) => vec![Event::RowAction(action)],
        // The Transport lives on the editor's top border, so a click on that
        // row is tested against it before it is read as a place in the text —
        // the border row is not one.
        (Pane::Editor, _) if input.row == panes.editor.y => {
            match transport_at(state, panes, input.column) {
                Some(action) => vec![Event::PaneAction(action)],
                None => vec![Event::ClickPane(Pane::Editor)],
            }
        }
        // The mirror's columns are the editor pane's, so a press in them is
        // tested before it is read as a place in the text: travelling a file
        // is not picking a character out of it.
        (Pane::Editor, _)
            if crate::minimap::strip(state, panes.editor).holds(input.column, input.row) =>
        {
            vec![Event::DragMinimap(minimap_row(
                crate::minimap::strip(state, panes.editor),
                input.row,
            ))]
        }
        // The toggle in the gutter and the dots at the end of a folded line are
        // one affordance drawn in two places, so a press on either is one
        // event. The caret lands first, because the block toggled is the block
        // the cursor is in — no second place has to be carried with it, the
        // same shape the jump modifier's click has.
        (Pane::Editor, _) if fold_toggle_at(state, panes, input) => {
            let at = place_in(state, panes, pane, (input.column, input.row));
            vec![Event::ClickText(at), Event::ToggleFold { all: false }]
        }
        // A diff is read-only and has no cursor to place.
        (Pane::Editor, _) if state.diff.is_none() && state.current_buffer.is_some() => {
            let at = place_in(state, panes, pane, (input.column, input.row));
            let mut events = vec![Event::ClickText(at)];
            // A click with the jump modifier held is `gd` with the pointer.
            // The caret lands first, so the question is about the name that
            // was clicked and no second place has to be carried with it.
            if jumping(input.modifiers) {
                events.push(Event::AskDefinition);
            }
            events
        }
        // visible_rows, not rows: in Review view the pane lists changed files,
        // and in Edit view it may be filtered.
        (Pane::Tree, None) => match tree::visible_rows(state).get(row_index) {
            Some(row) => vec![Event::ClickRow(row.path.clone())],
            None => vec![Event::ClickPane(pane)],
        },
        // A row of the Risk list, through its own scroll offset — the pane has
        // rows of its own, so hit-testing it against the tree's offset would
        // name whichever row the tree happened to be scrolled to.
        (Pane::Risk, _) => pressed_in_risk(state, panes, input),
        (Pane::Buffers, _) => pressed_in_buffers(state, panes, input),
        (Pane::History, _) => pressed_in_history(state, panes, input),
        // Which of the strip's shells was pressed, so a click in a split is
        // the keyboard moving to it — the one gesture that tells them apart.
        (Pane::Terminal, _) => vec![Event::FocusSplit(crate::layout::split_at(
            panes.terminal,
            state.terminals.len(),
            input.column,
        ))],
        _ => vec![Event::ClickPane(pane)],
    }
}

/// A row of the Risk list, through its own scroll offset — the pane has rows of
/// its own, so hit-testing it against the tree's offset would name whichever row
/// the tree happened to be scrolled to.
fn pressed_in_risk(state: &State, panes: &Layout, input: Input) -> Vec<Event> {
    // The pane's own actions live on its top border, where the figure is — so a
    // click on that row is tested against them before it is read as a row,
    // which the border row is not.
    if input.row == panes.corner.y {
        return match pane_action_at(state, panes, input.column) {
            Some(action) => vec![Event::PaneAction(action)],
            None => vec![Event::ClickPane(Pane::Risk)],
        };
    }
    // The bottom border is chrome too, and a scrolled or overlong list has a
    // Function at its index: reading it as a row opens one nowhere near the
    // pointer, and its icon columns would arm that row's action.
    if input.row >= panes.corner.bottom().saturating_sub(1) {
        return vec![Event::ClickPane(Pane::Risk)];
    }
    let index = list_row(panes.corner, input.row, state.risk_scroll);
    if let Some(action) = risk_action_at(state, panes, input.column, index) {
        return vec![Event::RowAction(action)];
    }
    match crate::risk::list(state).len() > index {
        true => vec![Event::ClickRiskRow(index)],
        false => vec![Event::ClickPane(Pane::Risk)],
    }
}

/// A row of the Buffers pane, through its own scroll offset. No action icons to
/// test past and no pane actions on its top border — the pane has no actions —
/// so both borders are chrome and every cell between them is either a buffer or
/// nothing. Both are tested: an offset index is bounded by the list, but the
/// bottom border sits a row past the last one a scrolled list draws, so reading
/// it as a row names a buffer nowhere near the pointer.
fn pressed_in_buffers(state: &State, panes: &Layout, input: Input) -> Vec<Event> {
    let index = list_row(panes.corner, input.row, state.buffers_scroll);
    let inside = input.row > panes.corner.y && input.row < panes.corner.bottom().saturating_sub(1);
    match inside && state.buffers.len() > index {
        true => vec![Event::ClickBufferRow(index)],
        false => vec![Event::ClickPane(Pane::Buffers)],
    }
}

/// A row of the Cursor history pane, through its own scroll offset. Both
/// borders are chrome — the pane has no actions of its own on the top one — and
/// the row's own icon is tested before the row, exactly as the Risk list's is:
/// the icon columns are the row's last two, and reading them as the row would
/// go there twice over.
fn pressed_in_history(state: &State, panes: &Layout, input: Input) -> Vec<Event> {
    if input.row <= panes.corner.y || input.row >= panes.corner.bottom().saturating_sub(1) {
        return vec![Event::ClickPane(Pane::History)];
    }
    let index = list_row(panes.corner, input.row, state.history_scroll);
    if let Some(action) = history_action_at(state, panes, input.column, index) {
        return vec![Event::RowAction(action)];
    }
    match crate::history::list(state).len() > index {
        true => vec![Event::ClickHistoryRow(index)],
        false => vec![Event::ClickPane(Pane::History)],
    }
}

fn dragged(
    state: &State,
    panes: &Layout,
    pointer: &mut Pointer,
    pane: Pane,
    input: Input,
) -> Outcome {
    let from = *pointer.drag_from.get_or_insert((input.column, input.row));
    // By pane, exhaustively: a fall-through arm here handed every pane nobody
    // had thought about a span of characters to read, which is how the Buffers
    // pane arrived copying the shell's grid.
    match pane {
        // A filename is not text you copy character by character, so a drag in
        // the tree moves the row selection.
        Pane::Tree => {
            let index = row_index(state, panes, input.row);
            Outcome::of(match tree::visible_rows(state).get(index) {
                Some(row) => vec![Event::DragRow(row.path.clone())],
                None => vec![],
            })
        }
        // The corner's panes hold a Row selection, which names something to go
        // to rather than text somebody picked: there is nothing in them to
        // copy, and nothing to copy is not the same as copying whatever the
        // pane behind them holds.
        Pane::Risk | Pane::Buffers | Pane::History => Outcome::default(),
        Pane::Editor => {
            // A drag that began in the mirror stays a travel however far the
            // pointer wanders, and a selection that wanders into the mirror
            // stays a selection: the gesture is decided by where the button
            // went down. Deciding it by where the pointer is now would have
            // every travel start picking text the moment it left the strip.
            let strip = crate::minimap::strip(state, panes.editor);
            if strip.holds(from.0, from.1) {
                return Outcome::of(vec![Event::DragMinimap(minimap_row(strip, input.row))]);
            }
            // Dragging the diff's gutter picks the lines a comment covers — the
            // mouse equivalent of V then c.
            if state.diff.is_some() && input.column < panes.editor.x + 1 + layout::GUTTER {
                return Outcome::of(match gutter_range(state, panes, from.1, input.row) {
                    Some((file, from_line, to_line)) => vec![Event::DragGutter {
                        file,
                        from_line,
                        to_line,
                    }],
                    None => vec![],
                });
            }
            // The editor needs nothing read off a screen: the span names lines
            // and columns of a buffer this crate already holds, so the drag
            // finishes here.
            match state.current_buffer {
                Some(_) => {
                    let (from, to) = span(state, panes, pane, from, input);
                    Outcome::of(vec![Event::DragText { from, to }])
                }
                None => Outcome::default(),
            }
        }
        // A pty's cells are the child's, so this half of the drag is the edge's
        // to finish.
        Pane::Terminal | Pane::Ai => {
            let (from, to) = span(state, panes, pane, from, input);
            Outcome {
                events: Vec::new(),
                select: Some(Selection { pane, from, to }),
                link: None,
            }
        }
    }
}

/// Which row of the mirror the pointer is holding, 0-based. Clamped to the
/// strip's own rows, so a drag that runs off the pane's edge travels to the end
/// rather than stopping where the rectangle does — the row, not the line drawn
/// on it, for the reason `minimap::travel` gives.
///
/// Takes the strip rather than hit-testing it a second time: both callers have
/// already asked whether the press is in it, and a hidden strip is zero-width,
/// which `Area::holds` answers `false` for — so there is no second answer here
/// for "the mirror is not showing", and nothing goes red if one is written.
fn minimap_row(strip: Area, row: u16) -> u32 {
    u32::from(row.clamp(strip.y, strip.bottom().saturating_sub(1)) - strip.y)
}

/// The span a drag covers, ordered so `from` is the earlier place however the
/// pointer travelled — a drag upwards names the same characters as the drag
/// back down over them.
fn span(
    state: &State,
    panes: &Layout,
    pane: Pane,
    from: (u16, u16),
    input: Input,
) -> (Place, Place) {
    let (start, end) = (
        place_in(state, panes, pane, from),
        place_in(state, panes, pane, (input.column, input.row)),
    );
    match (start.line, start.column) <= (end.line, end.column) {
        true => (start, end),
        false => (end, start),
    }
}

/// Which place in the buffer the pointer is over, or nothing at all when it is
/// over anything else. Nothing is what the dwell asks about, so it is also what
/// says the pointer has left: the border row the Transport lives on, another
/// pane, the gap between them, a diff, a Preview and an empty editor are one
/// answer — there is no symbol under it.
fn resting(state: &State, panes: &Layout, input: Input) -> Option<Place> {
    if state.diff.is_some() || state.current_buffer.is_none() || crate::previewing(state) {
        return None;
    }
    if layout::pane_at(panes, input.column, input.row) != Some(Pane::Editor)
        || input.row <= panes.editor.y
    {
        return None;
    }
    Some(place_in(
        state,
        panes,
        Pane::Editor,
        (input.column, input.row),
    ))
}

/// Where a screen position sits in the pane's own text, 1-based like the
/// cursor. The editor's text starts past the line-number gutter, its first row
/// is whatever it is scrolled to and its first column whatever it is scrolled
/// sideways to; a pty's grid starts at the border and is never scrolled past,
/// because the span names cells of the screen it is showing and there is no
/// history behind a full-screen program (ADR-0002).
///
/// Exhaustive on purpose: the AI pane used to fall into the terminal's arm, so
/// every drag in it asked for a span of the wrong rectangle.
fn place_in(state: &State, panes: &Layout, pane: Pane, (column, row): (u16, u16)) -> Place {
    let (area, gutter, scroll, sideways) = match pane {
        Pane::Editor => (
            panes.editor,
            // A Preview draws no line-number gutter, so its rows start five
            // columns to the left of Source's — the same reason `ui` and this
            // hit-test both ask `layout::gutter` rather than each assuming
            // `GUTTER`.
            crate::gutter(state),
            state.editor_scroll,
            state.editor_hscroll,
        ),
        Pane::Ai => (panes.ai, 0, 0, 0),
        // The split with the keyboard, which the press that began any drag
        // here has already chosen.
        Pane::Terminal => (
            crate::layout::split(panes.terminal, state.terminals.len(), state.split()),
            0,
            0,
            0,
        ),
        // A pane holding rows reaches this only for the wheel's `at`, which
        // nothing but a pty reads: `dragged` answers by pane before a span is
        // ever built out of one. Each still answers in its own rectangle —
        // pointing the corner's panes at the shell's is what made a drag in
        // them a span of characters nobody had pointed at.
        Pane::Tree => (panes.tree, 0, 0, 0),
        Pane::Risk | Pane::Buffers | Pane::History => (panes.corner, 0, 0, 0),
    };
    Place {
        // Through `line_at_row`, not straight off the row: Story view draws
        // comment rows between the lines, so the inverse of what the caret and
        // the scroll clamp use. The identity in every other view.
        line: crate::story::line_at_row(
            state,
            row.saturating_sub(area.y + 1) as usize + 1 + scroll,
        ),
        column: column.saturating_sub(area.x + 1 + gutter) as usize + 1 + sideways,
    }
}

/// Which palette entry sits under the pointer, if the palette is open — named
/// by the key it offers, so a click is the keystroke and not a second mapping
/// beside it. Rows carrying no key (a heading, a gap, the cancel line) answer
/// nothing.
pub fn palette_entry_at(state: &State, panes: &Layout, column: u16, row: u16) -> Option<char> {
    if state.modal != Modal::Palette {
        return None;
    }
    // The screen, which `ui` centres the box against and sizes the list to.
    // Read off the panes rather than taken as an argument: `tree` and
    // `terminal` tile the screen's height between them by construction.
    let height = panes.tree.height + panes.terminal.height;
    let rows = crate::palette_rows(height);
    let widest = rows
        .iter()
        .map(|(_, line)| line.chars().count() as u16)
        .max()
        .unwrap_or(0);
    // The screen, which `ui` centres the box against. Read off the AI pane
    // rather than the terminal: the two were the same number until the AI pane
    // could take the terminal's columns, and a box hit-tested against a
    // narrower screen than it was drawn on is offset from itself.
    let width = panes.ai.right();
    let box_area = layout::overlay(width, height, rows.len() as u16, widest);
    if !box_area.holds(column, row) {
        return None;
    }
    let index = row.checked_sub(box_area.y + 1)? as usize;
    rows.get(index)?.0
}

/// The label the editor's bottom edge ends with, and therefore where the buffer
/// dots sit. Shared so the hit-test and the rendering agree.
pub fn position_label(line: usize, column: usize) -> String {
    format!(" {line}:{column} ")
}

/// Which buffer dot is under the pointer.
pub fn buffer_dot_at(state: &State, panes: &Layout, column: u16, row: u16) -> Option<usize> {
    if row != panes.editor.bottom().saturating_sub(1) || state.buffers.len() < 2 {
        return None;
    }
    let shown = state
        .current_buffer
        .as_ref()
        .and_then(|path| state.buffers.get(path))
        .map(|buffer| position_label(buffer.line, buffer.column))?;
    let strip = state.buffers.len() as u16 * 2 - 1;
    let start = panes
        .editor
        .right()
        .saturating_sub(1 + shown.len() as u16 + strip);
    let index = (column.checked_sub(start)? / 2) as usize;
    (index < state.buffers.len()).then_some(index)
}

/// Which row of a list a screen row names, through that list's own scroll
/// offset: a pane's first row is not its list's first row once it has been
/// scrolled. The area and the offset are arguments because the surviving
/// difference between the four lists is exactly those two — each pane keeps its
/// own origin and its own offset, and the corner's three share a rectangle and
/// nothing else, so a shared offset would name whichever row another list was
/// left scrolled to. What must not differ is the arithmetic: the tree's action
/// hit-test used to derive its own without the offset, so a click on an icon
/// armed the action of a row however far the tree was scrolled.
fn list_row(area: Area, row: u16, scroll: usize) -> usize {
    row.saturating_sub(area.y + 1) as usize + scroll
}

/// Which of a row's action icons sits under the pointer. Two columns each, hard
/// against the pane's right-hand border, which is where `ui` right-aligns them
/// — the four hit-tests that ask this had a copy each, and the fourth copy is
/// what the rule of three forbids. An empty list answers nothing, since there
/// is no column an icon could be in.
fn icon_at(actions: &[&'static str], area: Area, column: u16) -> Option<&'static str> {
    let start = (area.x + area.width.saturating_sub(1)).checked_sub(2 * actions.len() as u16)?;
    actions
        .get(column.checked_sub(start)? as usize / 2)
        .copied()
}

/// Which row of the tree's list a screen row names. Named for its three
/// callers — the row hit-test, the drag and the action hit-test — which read
/// one answer rather than each deriving an offset.
fn row_index(state: &State, panes: &Layout, row: u16) -> usize {
    list_row(panes.tree, row, state.tree_scroll)
}

/// Which Transport control, if any, sits under the pointer — on the editor's
/// top border, at the columns `ui` draws them: the same list feeds the glyph,
/// the click and the key, and `layout::strip_at` is the one place their
/// columns are worked out.
fn transport_at(state: &State, panes: &Layout, column: u16) -> Option<&'static str> {
    let controls = crate::reading::transport(state);
    let labels: Vec<String> = controls.iter().map(|(_, glyph)| glyph.clone()).collect();
    let at = crate::layout::strip_at(panes.editor, &labels, column)?;
    Some(controls[at].0)
}

/// Whether a press lands on a fold's affordance: the toggle in the gutter's
/// pad, or the dots a folded line carries at the end of its text.
///
/// The pad rather than the one column `layout::TOGGLE_COLUMN` names — a
/// one-column target is a target a pointer misses, and the column beside it is
/// air the line does not otherwise use. Only on a line that has a toggle at
/// all: everywhere else those columns are the gutter, and a gutter click puts
/// the caret at the start of the line the way it always did.
fn fold_toggle_at(state: &State, panes: &Layout, input: Input) -> bool {
    if state.diff.is_some() || crate::previewing(state) {
        return false;
    }
    let at = place_in(state, panes, Pane::Editor, (input.column, input.row));
    let Some(toggle) = crate::fold::toggles(state).get(&at.line).copied() else {
        return false;
    };
    let pad = (panes.editor.x + 1 + layout::TOGGLE_COLUMN..=panes.editor.x + layout::GUTTER)
        .contains(&input.column);
    pad || (toggle == crate::fold::Toggle::Folded
        && crate::current_buffer(state)
            .is_some_and(|buffer| buffer.fold_dots_at(at.line, at.column)))
}

/// Which clickable icon the pointer is over, wherever it is: a tree row's
/// actions, the Risk list's and the ones on its border, the Cursor history's,
/// and the Transport on the editor's. One dispatcher rather than a hover test
/// per strip, and it reuses the very hit-tests the press uses — an icon that
/// lights up somewhere the click does not land is worse than one that never
/// lights up at all.
///
/// The row guards are `pressed`'s: a border row is chrome, not a row, and
/// reading one as a row names an entry nowhere near the pointer.
fn action_under(state: &State, panes: &Layout, input: Input) -> Option<&'static str> {
    let corner_row =
        input.row > panes.corner.y && input.row < panes.corner.bottom().saturating_sub(1);
    match layout::pane_at(panes, input.column, input.row)? {
        Pane::Tree => action_at(state, panes, input.column, input.row),
        Pane::Editor if input.row == panes.editor.y => transport_at(state, panes, input.column),
        Pane::Risk if input.row == panes.corner.y => pane_action_at(state, panes, input.column),
        Pane::Risk if corner_row => {
            let index = list_row(panes.corner, input.row, state.risk_scroll);
            risk_action_at(state, panes, input.column, index)
        }
        Pane::History if corner_row => {
            let index = list_row(panes.corner, input.row, state.history_scroll);
            history_action_at(state, panes, input.column, index)
        }
        _ => None,
    }
}

/// Which action icon, if any, sits under the pointer.
pub fn action_at(state: &State, panes: &Layout, column: u16, row: u16) -> Option<&'static str> {
    let clicked = tree::visible_rows(state)
        .into_iter()
        .nth(row_index(state, panes, row))?;
    icon_at(&tree::row_actions(state, &clicked.path), panes.tree, column)
}

/// Which action icon of the Cursor history pane, if any, sits under the
/// pointer. Only the row the keyboard is on, exactly as the Risk list's is:
/// that is the row the pane draws icons on.
fn history_action_at(
    state: &State,
    panes: &Layout,
    column: u16,
    index: usize,
) -> Option<&'static str> {
    if index != state.history_selection {
        return None;
    }
    icon_at(&crate::history::row_actions(state), panes.corner, column)
}

/// Which action icon of the Risk list, if any, sits under the pointer. Only the
/// row the keyboard is on: that is the row the pane draws icons on, and the
/// renderer and this read the same arithmetic for the reason the tree and its
/// hit-test do.
fn risk_action_at(
    state: &State,
    panes: &Layout,
    column: u16,
    index: usize,
) -> Option<&'static str> {
    if index != state.risk_selection {
        return None;
    }
    icon_at(&crate::risk::row_actions(state), panes.corner, column)
}

/// Which of the Risk pane's own action icons sits under the pointer, on the
/// pane's top border — the same columns a row's icons occupy, because `ui`
/// draws both from `risk::pane_actions` at the same offsets.
fn pane_action_at(state: &State, panes: &Layout, column: u16) -> Option<&'static str> {
    icon_at(&crate::risk::pane_actions(state), panes.corner, column)
}

/// The new-file line numbers two gutter rows span.
pub fn gutter_range(
    state: &State,
    panes: &Layout,
    from_row: u16,
    to_row: u16,
) -> Option<(String, u32, u32)> {
    let diff = state.diff.as_ref()?;
    let top = panes.editor.y + 1;
    let first = from_row.checked_sub(top)? as usize;
    let last = to_row.checked_sub(top)? as usize;
    let (lo, hi) = (first.min(last), first.max(last));
    let numbers: Vec<u32> = diff
        .get(lo..=hi.min(diff.len().saturating_sub(1)))?
        .iter()
        .filter_map(|line| line.new_line.map(|n| n as u32))
        .collect();
    Some((
        state.diff_file.clone()?,
        *numbers.first()?,
        *numbers.last()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        on_mouse, palette_entry_at, place_in, report, Divider, Encoding, Gesture, Input,
        KeyModifiers, Kind, Outcome, Pointer,
    };
    use crate::layout::panes;
    use crate::layout::{AiPane, Shapes};
    use crate::tree::Entry;
    use crate::Direction;
    use crate::{Event, Modal, Pane, Place, State, View};
    use std::path::PathBuf;

    fn workspace() -> State {
        let mut state = State {
            root: PathBuf::from("/w"),
            tree_divider: 30,
            ..State::default()
        };
        state.contents.insert(
            PathBuf::from("/w"),
            vec![
                Entry {
                    name: "src".to_string(),
                    is_dir: true,
                },
                Entry {
                    name: "a.rs".to_string(),
                    is_dir: false,
                },
            ],
        );
        state
    }

    fn click(state: &State, column: u16, row: u16) -> Vec<Event> {
        on_mouse(
            state,
            &panes(120, 26, 30, None, 0, 0, Shapes::default()),
            &mut Pointer::default(),
            Input {
                kind: Kind::LeftDown,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
        )
        .events
    }

    /// The cell the encoding tests report on, so each expectation reads as the
    /// bytes for one known cell.
    const CELL: Place = Place { line: 5, column: 9 };

    #[test]
    fn a_child_asking_for_sgr_gets_sgr() {
        assert_eq!(
            report(Encoding::Sgr, Gesture::Click, CELL),
            Some(b"\x1b[<0;9;5M\x1b[<0;9;5m".to_vec())
        );
        assert_eq!(
            report(Encoding::Sgr, Gesture::Wheel(Direction::Up), CELL),
            Some(b"\x1b[<64;9;5M".to_vec())
        );
        assert_eq!(
            report(Encoding::Sgr, Gesture::Wheel(Direction::Down), CELL),
            Some(b"\x1b[<65;9;5M".to_vec())
        );
    }

    // Four buttons, four numbers. A sideways swipe answered with 65 is a child
    // scrolled down by a gesture nobody made sideways, which is the
    // substitution `report` refuses on coordinates and must refuse on buttons.
    #[test]
    fn a_sideways_wheel_is_its_own_button() {
        assert_eq!(
            report(Encoding::Sgr, Gesture::Wheel(Direction::Left), CELL),
            Some(b"\x1b[<66;9;5M".to_vec())
        );
        assert_eq!(
            report(Encoding::Sgr, Gesture::Wheel(Direction::Right), CELL),
            Some(b"\x1b[<67;9;5M".to_vec())
        );
        assert_eq!(
            report(Encoding::Legacy, Gesture::Wheel(Direction::Left), CELL),
            Some(vec![0x1b, b'[', b'M', 32 + 66, 32 + 9, 32 + 5])
        );
        assert_eq!(
            report(Encoding::Legacy, Gesture::Wheel(Direction::Right), CELL),
            Some(vec![0x1b, b'[', b'M', 32 + 67, 32 + 9, 32 + 5])
        );
    }

    // The phantom text: a child that enabled legacy tracking cannot parse an
    // SGR report, so it read one as characters typed at its prompt.
    #[test]
    fn a_child_asking_for_legacy_gets_legacy() {
        assert_eq!(
            report(Encoding::Legacy, Gesture::Click, CELL),
            Some(vec![
                0x1b,
                b'[',
                b'M',
                32,
                32 + 9,
                32 + 5,
                0x1b,
                b'[',
                b'M',
                32 + 3,
                32 + 9,
                32 + 5
            ])
        );
        assert_eq!(
            report(Encoding::Legacy, Gesture::Wheel(Direction::Up), CELL),
            Some(vec![0x1b, b'[', b'M', 32 + 64, 32 + 9, 32 + 5])
        );
        assert_eq!(
            report(Encoding::Legacy, Gesture::Wheel(Direction::Down), CELL),
            Some(vec![0x1b, b'[', b'M', 32 + 65, 32 + 9, 32 + 5])
        );
    }

    #[test]
    fn a_child_that_asked_for_no_mouse_is_told_nothing() {
        assert_eq!(report(Encoding::None, Gesture::Click, CELL), None);
        assert_eq!(
            report(Encoding::None, Gesture::Wheel(Direction::Up), CELL),
            None
        );
    }

    // The legacy encoding's fields are one byte offset by 32, so column 224 has
    // no representation. Declining is the decision: wrapping it would report a
    // different cell, and the click would land somewhere nobody pointed at.
    #[test]
    fn a_cell_the_legacy_encoding_cannot_express_is_declined() {
        let last = Place {
            line: 223,
            column: 223,
        };
        assert!(report(Encoding::Legacy, Gesture::Click, last).is_some());
        for past in [
            Place {
                line: 1,
                column: 224,
            },
            Place {
                line: 224,
                column: 1,
            },
        ] {
            assert_eq!(report(Encoding::Legacy, Gesture::Click, past), None);
            // SGR has no such limit, so the same cell is reportable there.
            assert!(report(Encoding::Sgr, Gesture::Click, past).is_some());
        }
    }

    #[test]
    fn a_click_finds_the_pane_under_it() {
        let state = workspace();
        assert_eq!(click(&state, 40, 5), vec![Event::ClickPane(Pane::Editor)]);
        assert_eq!(click(&state, 100, 5), vec![Event::ClickPane(Pane::Ai)]);
        assert_eq!(click(&state, 40, 20), vec![Event::FocusSplit(0)]);
    }

    #[test]
    fn a_click_in_a_split_moves_the_keyboard_to_it() {
        let state = State {
            terminals: vec![crate::Shell::Idle; 2],
            ..workspace()
        };
        let strip = panes(120, 26, 30, None, 0, 0, Shapes::default()).terminal;
        let second = crate::layout::split(strip, 2, 1);
        assert_eq!(click(&state, strip.x + 1, 20), vec![Event::FocusSplit(0)]);
        assert_eq!(click(&state, second.x + 1, 20), vec![Event::FocusSplit(1)]);
    }

    // The bug: hit-testing used the unfiltered tree, so in Review view — and in a
    // filtered tree — a click resolved to the wrong row.
    #[test]
    fn tree_clicks_resolve_against_the_rows_on_screen() {
        let state = workspace();
        assert_eq!(
            click(&state, 5, 1),
            vec![Event::ClickRow(PathBuf::from("/w/src"))]
        );
        assert_eq!(
            click(&state, 5, 2),
            vec![Event::ClickRow(PathBuf::from("/w/a.rs"))]
        );

        // In Review view the same rows are the changed files, not the tree.
        let mut review = workspace();
        review.view = View::Review;
        review.repo = Some(vec![crate::review::GitFile {
            path: "only.rs".to_string(),
            status: crate::review::GitStatus::Modified,
        }]);
        assert_eq!(
            click(&review, 5, 1),
            vec![Event::ClickRow(PathBuf::from("/w/only.rs"))]
        );
    }

    /// The box is drawn over the panes, so a press in one is a press on
    /// something nobody can see. The bug this guards: a click in the tree
    /// behind it opened whatever file was under the box. The wheel still comes
    /// through, because the box is what scrolls on it.
    #[test]
    fn the_results_box_swallows_a_press_on_the_pane_behind_it() {
        let mut searching = workspace();
        searching.search = Some(crate::Search::default());
        assert_eq!(click(&searching, 5, 1), vec![]);
        let wheeled = on_mouse(
            &searching,
            &panes(120, 26, 30, None, 0, 0, Shapes::default()),
            &mut Pointer::default(),
            Input {
                kind: Kind::ScrollDown,
                column: 5,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        )
        .events;
        assert!(
            matches!(wheeled.first(), Some(Event::Scroll { .. })),
            "the wheel never reached the box: {wheeled:?}"
        );
    }

    /// A press inside the box is the box's, and the row it lands on is read the
    /// way the renderer draws it: headings between the hits, and the list
    /// starting wherever the wheel left it. The bug this guards is the one the
    /// scroll rules exist for — a hit-test that counted hits, or counted from
    /// the top of the list, marks a different file from the one under the
    /// pointer, and `Enter` then opens it.
    #[test]
    fn a_press_in_the_results_box_marks_the_hit_under_the_pointer() {
        let hit = |file: &str| crate::search::Hit {
            file: file.to_string(),
            line: 1,
            column: 1,
            text: "update".to_string(),
        };
        let mut searching = workspace();
        searching.screen_width = 120;
        searching.screen_height = 26;
        searching.search = Some(crate::Search {
            // Five rows: a.rs, its two hits, b.rs, its one.
            results: crate::search::Results {
                hits: vec![hit("a.rs"), hit("a.rs"), hit("b.rs")],
                truncated: false,
            },
            ..crate::Search::default()
        });
        // The box is inset by three rows in a 26-row screen, and the border and
        // the three header rows come out of it — pinned rather than derived, so
        // moving the list on screen has to be admitted here.
        let top = 7;
        assert_eq!(click(&searching, 10, top - 1), vec![], "the query row");
        assert_eq!(click(&searching, 10, top), vec![], "a file heading");
        assert_eq!(click(&searching, 10, top + 1), vec![Event::SelectHit(0)]);
        assert_eq!(click(&searching, 10, top + 2), vec![Event::SelectHit(1)]);
        assert_eq!(click(&searching, 10, top + 3), vec![], "the next heading");
        assert_eq!(click(&searching, 10, top + 4), vec![Event::SelectHit(2)]);
        assert_eq!(click(&searching, 10, top + 5), vec![], "past the last row");

        // What the wheel left under the pointer, at the very same row.
        searching.search.as_mut().unwrap().scroll = 3;
        assert_eq!(click(&searching, 10, top), vec![], "the heading, scrolled");
        assert_eq!(click(&searching, 10, top + 1), vec![Event::SelectHit(2)]);

        // The bottom border is inside the rectangle and part of no row: the box
        // shows fifteen rows of a twenty-row inset.
        assert_eq!(click(&searching, 10, top + 15), vec![], "the box's chrome");
    }

    // The bug this guards: the renderer scrolls the pane but the hit-test still
    // counted from the top of the list, so every click landed rows too high.
    #[test]
    fn tree_clicks_count_from_the_scrolled_row() {
        let mut scrolled = workspace();
        scrolled.tree_scroll = 1;
        assert_eq!(
            click(&scrolled, 5, 1),
            vec![Event::ClickRow(PathBuf::from("/w/a.rs"))]
        );
    }

    // The bug: the action hit-test counted from the top of the list while the
    // row hit-test counted from the scrolled row, so once the tree was scrolled
    // an action click armed the action of a different row.
    #[test]
    fn action_clicks_count_from_the_scrolled_row() {
        // The tree pane is 30 wide, so its last inner column is 28 and a file's
        // two action icons sit at 25..=28 — the columns tree_lines reserves.
        let mut scrolled = workspace();
        scrolled.tree_scroll = 1;
        scrolled.tree_selection = Some(PathBuf::from("/w/a.rs"));
        assert_eq!(click(&scrolled, 25, 1), vec![Event::RowAction("delete")]);
        assert_eq!(click(&scrolled, 27, 1), vec![Event::RowAction("copy-path")]);

        // And the row scrolled off the top offers nothing at that position: its
        // four folder actions must not be reachable from the row below it.
        let mut folder = workspace();
        folder.tree_scroll = 1;
        folder.tree_selection = Some(PathBuf::from("/w/src"));
        assert_eq!(
            click(&folder, 21, 1),
            vec![Event::ClickRow(PathBuf::from("/w/a.rs"))]
        );
    }

    /// The Risk list has rows of its own, so it is hit-tested against its own
    /// rectangle and its own scroll offset. Against the tree's — which is what
    /// a `list_row` taking neither as an argument would have to use — a click
    /// would name whichever row the tree happened to be scrolled to, in a pane
    /// that is not the tree.
    #[test]
    fn risk_clicks_count_from_the_panes_own_first_row() {
        // The pane sits at row 18 and is 8 rows tall, so its first row inside
        // the borders is 19 — the row the layout's own test pins it at.
        let mut state = risk_workspace();
        assert_eq!(risk_click(&state, 5, 19), vec![Event::ClickRiskRow(0)]);
        assert_eq!(risk_click(&state, 5, 20), vec![Event::ClickRiskRow(1)]);
        // Past the last row is the pane and not a row: there is nothing there
        // to open.
        assert_eq!(
            risk_click(&state, 5, 21),
            vec![Event::ClickPane(Pane::Risk)]
        );
        // Scrolled, the pane's first row is the row it is scrolled to — the
        // renderer reads the same field.
        state.risk_scroll = 1;
        assert_eq!(risk_click(&state, 5, 19), vec![Event::ClickRiskRow(1)]);
    }

    /// The pane's bottom border is chrome, the way its top one is. A list
    /// longer than the pane draws does have a Function at that row's index, so
    /// reading the border as a row opens one nowhere near the pointer — and its
    /// icon columns would arm that row's action. The Buffers pane's hit-test was
    /// cloned from this one, which is where it turned up.
    #[test]
    fn a_click_on_the_risk_panes_bottom_border_is_not_a_row() {
        let mut state = risk_workspace();
        state.risk.figure = crate::risk::Figure::Current(crate::risk::Figures {
            functions: (0..20)
                .map(|index| crate::risk::Function {
                    file: "src/keys.rs".to_string(),
                    name: format!("wide{index:02}"),
                    line: index + 1,
                    metrics: crate::risk::Metrics {
                        cyclomatic: 31,
                        ..crate::risk::Metrics::default()
                    },
                })
                .collect(),
            unparsed: 0,
        });
        // The pane draws six rows, 19..=24, so 25 is its bottom border.
        assert_eq!(
            risk_click(&state, 5, 24),
            vec![Event::ClickRiskRow(5)],
            "the last row the pane draws"
        );
        assert_eq!(
            risk_click(&state, 5, 25),
            vec![Event::ClickPane(Pane::Risk)]
        );
    }

    /// The pane's own two actions are on its top border, at the columns `ui`
    /// draws them: the pane is 30 wide, so two icons take 25..=28. The start
    /// flips to the stop in the same slot, which is the whole point of one
    /// list feeding the icon, the click and the key.
    #[test]
    fn a_click_on_the_panes_border_icons_reaches_the_panes_actions() {
        let state = risk_workspace();
        assert_eq!(
            risk_click(&state, 25, 18),
            vec![Event::PaneAction(crate::risk::RECOMPUTE)]
        );
        assert_eq!(
            risk_click(&state, 27, 18),
            vec![Event::PaneAction(crate::risk::START_LOOP)]
        );
        // The border's name, not its icons.
        assert_eq!(
            risk_click(&state, 5, 18),
            vec![Event::ClickPane(Pane::Risk)]
        );
    }

    /// The Risk row's own action icon, on the columns the pane draws it at:
    /// the pane is 30 wide, so one icon sits at 27..=28, hard against the
    /// border — the same last two content columns the tree reserves.
    #[test]
    fn a_click_on_the_risk_rows_icon_asks_for_the_refactor() {
        let state = risk_workspace();
        assert_eq!(
            risk_click(&state, 27, 19),
            vec![Event::RowAction(crate::risk::REFACTOR)]
        );
        // Anywhere else on the row is the row, not its icon.
        assert_eq!(risk_click(&state, 26, 19), vec![Event::ClickRiskRow(0)]);
        // And the row the keyboard is not on draws no icon, so those columns
        // are that row rather than an action of the selected one.
        assert_eq!(risk_click(&state, 27, 20), vec![Event::ClickRiskRow(1)]);
    }

    /// The corner's other occupant, hit-tested against the same rectangle. A
    /// row of it is a buffer, its border is not a row, and a cell past the last
    /// buffer focuses the pane rather than naming a row that is not there — the
    /// pane has no action icons, so there is nothing else a column can mean.
    #[test]
    fn a_click_in_the_buffers_pane_names_the_row_it_landed_on() {
        let buffers = |count: usize| {
            let mut state = workspace();
            state.corner = crate::layout::Corner::Buffers;
            for index in 0..count {
                state.buffers.insert(
                    state.root.join(format!("{index:02}.rs")),
                    crate::editor::Buffer::open("contents", false, 4),
                );
            }
            state
        };
        let click = |state: &State, column, row| {
            on_mouse(
                state,
                &panes(
                    120,
                    26,
                    30,
                    None,
                    0,
                    0,
                    Shapes {
                        corner: crate::layout::Corner::Buffers,
                        ..Shapes::default()
                    },
                ),
                &mut Pointer::default(),
                Input {
                    kind: Kind::LeftDown,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )
            .events
        };
        let two = buffers(2);
        assert_eq!(click(&two, 5, 19), vec![Event::ClickBufferRow(0)]);
        assert_eq!(click(&two, 5, 20), vec![Event::ClickBufferRow(1)]);
        // The top border, which is chrome rather than the first row.
        assert_eq!(click(&two, 5, 18), vec![Event::ClickPane(Pane::Buffers)]);
        assert_eq!(click(&two, 5, 21), vec![Event::ClickPane(Pane::Buffers)]);
        // A list longer than the pane draws, unscrolled: the bottom border is
        // chrome too, and the list having a row at that index is not the
        // question — the pane is six rows tall, so index 6 is drawn nowhere.
        let many = buffers(20);
        let shown = panes(
            120,
            26,
            30,
            None,
            0,
            0,
            Shapes {
                corner: crate::layout::Corner::Buffers,
                ..Shapes::default()
            },
        )
        .corner
        .height as usize
            - 2;
        assert_eq!(shown, 6);
        assert!(many.buffers.len() > shown);
        assert_eq!(click(&many, 5, 24), vec![Event::ClickBufferRow(5)]);
        assert_eq!(click(&many, 5, 25), vec![Event::ClickPane(Pane::Buffers)]);
    }

    /// The Risk list, shown, with two Functions over the threshold — the same
    /// two the pane's own tests use.
    fn risk_workspace() -> State {
        let mut state = workspace();
        state.corner = crate::layout::Corner::Risk;
        state.risk_threshold = 20;
        state.risk.figure = crate::risk::Figure::Current(crate::risk::Figures {
            functions: vec![
                crate::risk::Function {
                    file: "src/keys.rs".to_string(),
                    name: "route".to_string(),
                    line: 88,
                    metrics: crate::risk::Metrics {
                        cyclomatic: 31,
                        ..crate::risk::Metrics::default()
                    },
                },
                crate::risk::Function {
                    file: "src/ui.rs".to_string(),
                    name: "draw".to_string(),
                    line: 17,
                    metrics: crate::risk::Metrics {
                        cyclomatic: 22,
                        ..crate::risk::Metrics::default()
                    },
                },
            ],
            unparsed: 0,
        });
        state
    }

    /// The corner's third occupant, hit-tested against the same rectangle. A
    /// row of it is a Visit, both borders are chrome, and the icon takes the
    /// row's last two content columns — 27..=28 in a 30-column pane, the same
    /// two the Risk row and the tree row reserve.
    #[test]
    fn a_click_in_the_cursor_history_names_the_row_or_its_icon() {
        let mut state = workspace();
        state.corner = crate::layout::Corner::History;
        state.visits = (0..20)
            .map(|which| crate::history::Visit {
                file: format!("src/{which:02}.rs"),
                line: which + 1,
                column: 1,
                text: "let a = 1;".to_string(),
            })
            .collect();
        // The row the keyboard is on, which is the only row that draws an icon.
        state.history_selection = 0;
        let click = |state: &State, column, row| {
            on_mouse(
                state,
                &panes(
                    120,
                    26,
                    30,
                    None,
                    0,
                    0,
                    Shapes {
                        corner: crate::layout::Corner::History,
                        ..Shapes::default()
                    },
                ),
                &mut Pointer::default(),
                Input {
                    kind: Kind::LeftDown,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )
            .events
        };
        assert_eq!(click(&state, 5, 19), vec![Event::ClickHistoryRow(0)]);
        assert_eq!(click(&state, 5, 20), vec![Event::ClickHistoryRow(1)]);
        assert_eq!(
            click(&state, 27, 19),
            vec![Event::RowAction(crate::history::GO_TO)]
        );
        // Anywhere else on that row is the row, and the same columns of a row
        // the keyboard is not on draw no icon at all.
        assert_eq!(click(&state, 26, 19), vec![Event::ClickHistoryRow(0)]);
        assert_eq!(click(&state, 27, 20), vec![Event::ClickHistoryRow(1)]);
        // Both borders are chrome. The bottom one matters most: the list is
        // longer than the six rows the pane draws, so it has a Visit at that
        // index and reading it as a row would go somewhere nobody pointed.
        assert_eq!(click(&state, 5, 18), vec![Event::ClickPane(Pane::History)]);
        assert_eq!(click(&state, 5, 25), vec![Event::ClickPane(Pane::History)]);
    }

    /// R35.8. The Transport is on the editor's top border, so a click there is
    /// a control and never a place in the text — and a buffer that cannot be
    /// read has no controls, so the same columns are just the pane again.
    #[test]
    fn a_click_on_the_transport_reaches_the_reading_it_names() {
        let mut state = workspace();
        state.current_buffer = Some(PathBuf::from("/w/guide.md"));
        state.speech.speed = 1.0;
        let editor = panes(120, 26, 30, None, 0, 0, Shapes::default()).editor;
        // The strip is `« ▸ » ▪ 1.00x `, fourteen columns ending on the one
        // before the corner — the columns `ui`'s render test pins.
        let last = editor.x + editor.width - 1;
        assert_eq!(
            click(&state, last - 14, 0),
            vec![Event::PaneAction(crate::reading::PREVIOUS)]
        );
        assert_eq!(
            click(&state, last - 12, 0),
            vec![Event::PaneAction(crate::reading::PLAY_PAUSE)]
        );
        assert_eq!(
            click(&state, last - 10, 0),
            vec![Event::PaneAction(crate::reading::NEXT)]
        );
        assert_eq!(
            click(&state, last - 8, 0),
            vec![Event::PaneAction(crate::reading::STOP)]
        );
        assert_eq!(
            click(&state, last - 5, 0),
            vec![Event::PaneAction(crate::reading::SPEED)]
        );
        // The border's name, not its controls. The corner itself is not
        // tested here: it is the AI divider's handle, which is grabbed before
        // any pane sees the press — `layout`'s own test holds it to naming no
        // control.
        assert_eq!(
            click(&state, editor.x + 10, 0),
            vec![Event::ClickPane(Pane::Editor)]
        );
        // A row inside the pane is still text, which is what the border row
        // being tested first has to leave alone.
        assert!(matches!(
            click(&state, editor.x + 8, 3).as_slice(),
            [Event::ClickText(_)]
        ));
        // A buffer nothing can read draws no Transport, so those columns are
        // the pane and not a control that quietly does nothing.
        state.current_buffer = Some(PathBuf::from("/w/a.rs"));
        assert_eq!(
            click(&state, last - 5, 0),
            vec![Event::ClickPane(Pane::Editor)]
        );
    }

    /// Where the pointer rests is the editor's text and nothing else: the row
    /// the Transport lives on is a border, and every other pane is the pointer
    /// having left — which is what takes a box it rested for down. Nothing is
    /// the answer for both, because both mean there is no symbol under it.
    #[test]
    fn a_move_answers_with_the_place_only_over_the_editors_text() {
        let state = editing();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let editor = panes.editor;
        let moved = |column, row| {
            on_mouse(
                &state,
                &panes,
                &mut Pointer::default(),
                Input {
                    kind: Kind::Moved,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )
            .events
        };
        assert_eq!(
            moved(editor.x + 1 + crate::gutter(&state) + 3, editor.y + 2),
            vec![Event::PointerMoved(Some(Place { line: 2, column: 4 }))]
        );
        assert_eq!(
            moved(editor.x + 4, editor.y),
            vec![Event::PointerMoved(None)]
        );
        assert_eq!(moved(1, editor.y + 2), vec![Event::PointerMoved(None)]);
        // A diff and a Preview draw rows that are not the buffer's lines, so
        // the cell the pointer is on names no place in the text (ADR 0007).
        for showing in [diffed(), previewing()] {
            assert_eq!(
                on_mouse(
                    &showing,
                    &panes,
                    &mut Pointer::default(),
                    Input {
                        kind: Kind::Moved,
                        column: editor.x + 8,
                        row: editor.y + 2,
                        modifiers: KeyModifiers::NONE,
                    },
                )
                .events,
                vec![Event::PointerMoved(None)]
            );
        }
    }

    /// A pointer crossing the pane reports a cell at a time, so a link that
    /// was news on every one of them would be a frame per cell. Only the
    /// change is reported, and letting go of the modifier is a change. Where
    /// it rests is reported every time either way: that one is the place, not
    /// the news, and the dwell window is armed off it.
    #[test]
    fn a_held_modifier_says_where_the_link_is_once_and_says_when_it_is_gone() {
        let mut state = workspace();
        let path = PathBuf::from("/w/a.rs");
        state.current_buffer = Some(path.clone());
        state
            .buffers
            .insert(path, crate::editor::Buffer::open("fn main() {}", false, 4));
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let cell = (panes.editor.x + 5, panes.editor.y + 1);
        let hover = |state: &State, modifiers| {
            on_mouse(
                state,
                &panes,
                &mut Pointer::default(),
                Input {
                    kind: Kind::Moved,
                    column: cell.0,
                    row: cell.1,
                    modifiers,
                },
            )
            .events
        };
        let at = place_in(&state, &panes, Pane::Editor, cell);
        assert_eq!(
            hover(&state, KeyModifiers::SUPER),
            vec![Event::PointerMoved(Some(at)), Event::HoverLink(Some(at))]
        );
        state.link = Some(at);
        assert_eq!(
            hover(&state, KeyModifiers::CTRL),
            vec![Event::PointerMoved(Some(at))]
        );
        assert_eq!(
            hover(&state, KeyModifiers::NONE),
            vec![Event::PointerMoved(Some(at)), Event::HoverLink(None)]
        );
    }

    fn risk_click(state: &State, column: u16, row: u16) -> Vec<Event> {
        on_mouse(
            state,
            &panes(
                120,
                26,
                30,
                None,
                0,
                0,
                Shapes {
                    corner: crate::layout::Corner::Risk,
                    ..Shapes::default()
                },
            ),
            &mut Pointer::default(),
            Input {
                kind: Kind::LeftDown,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
        )
        .events
    }

    #[test]
    fn a_click_past_the_last_row_focuses_the_pane() {
        assert_eq!(
            click(&workspace(), 5, 10),
            vec![Event::ClickPane(Pane::Tree)]
        );
    }

    #[test]
    fn the_divider_is_grabbed_before_the_row_beneath_it() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        // Column 29 is the tree's right border and also inside the tree pane.
        let outcome = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDown,
                column: 29,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(outcome.events.is_empty(), "no row click");
        assert_eq!(pointer.dragging, Some(Divider::Tree));

        let outcome = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDrag,
                column: 45,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(outcome.events, vec![Event::DragDivider(46)]);
    }

    #[test]
    fn the_divider_is_released_and_stops_resizing() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer {
            dragging: Some(Divider::Tree),
            ..Pointer::default()
        };
        on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftUp,
                column: 45,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(pointer.dragging, None);
    }

    #[test]
    fn the_divider_cannot_squeeze_a_pane_away() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer {
            dragging: Some(Divider::Tree),
            ..Pointer::default()
        };
        let narrow = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDrag,
                column: 0,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(narrow.events, vec![Event::DragDivider(12)]);
        let mut pointer = Pointer {
            dragging: Some(Divider::Tree),
            ..Pointer::default()
        };
        let wide = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDrag,
                column: 119,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(wide.events, vec![Event::DragDivider(90)]);
    }

    /// The AI pane's left border is the second handle, and it names a width
    /// rather than a column so the tree's divider can move under it.
    #[test]
    fn the_ai_pane_edge_is_grabbed_and_names_a_width() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        // Column 84 is the AI pane's left border: 30 tree + 54 editor.
        let outcome = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDown,
                column: 84,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(outcome.events.is_empty(), "no click through to the pane");
        assert_eq!(pointer.dragging, Some(Divider::Ai));

        let outcome = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDrag,
                column: 80,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(outcome.events, vec![Event::DragAiDivider(40)]);
    }

    /// The terminal runs the full width beneath, so its rows must reach the
    /// shell rather than a divider that is nowhere near them.
    #[test]
    fn a_divider_column_in_the_terminal_pane_is_a_click_in_the_terminal() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        for column in [29, 84] {
            let mut pointer = Pointer::default();
            let outcome = on_mouse(
                &state,
                &panes,
                &mut pointer,
                Input {
                    kind: Kind::LeftDown,
                    column,
                    row: 20,
                    modifiers: KeyModifiers::NONE,
                },
            );
            assert_eq!(pointer.dragging, None, "column {column} grabbed a divider");
            assert!(
                !outcome.events.is_empty(),
                "column {column} reached nothing"
            );
        }
    }

    /// The palette is centred on the screen, and `ui` centres it against the
    /// frame. Hit-testing it against the terminal's width agreed with that
    /// only while the terminal spanned the screen — a tall AI pane takes those
    /// columns, and the entries would sit half a pane to the right of where
    /// they were clicked. The same defect `layout::overlay` was extracted to
    /// stop, arriving by the other door.
    #[test]
    fn the_palette_is_hit_tested_where_it_is_drawn() {
        let state = State {
            modal: Modal::Palette,
            ..workspace()
        };
        for row in 0..26 {
            for column in 0..120 {
                let beside = palette_entry_at(
                    &state,
                    &panes(120, 26, 30, None, 0, 0, Shapes::default()),
                    column,
                    row,
                );
                let tall = palette_entry_at(
                    &state,
                    &panes(
                        120,
                        26,
                        30,
                        None,
                        0,
                        0,
                        Shapes {
                            ai: AiPane::Tall,
                            ..Shapes::default()
                        },
                    ),
                    column,
                    row,
                );
                assert_eq!(beside, tall, "entry at {column},{row} moved with the shape");
            }
        }
        // And it really does find entries, or the sweep above proves nothing.
        let panes = panes(
            120,
            26,
            30,
            None,
            0,
            0,
            Shapes {
                ai: AiPane::Tall,
                ..Shapes::default()
            },
        );
        assert!((0..26).any(|row| palette_entry_at(&state, &panes, 60, row) == Some('r')));
    }

    /// Every entry the palette offers is clickable at some cell, at the two
    /// screen heights that matter: 26 rows, which the replay recipe and the
    /// scenarios use, and 24, which is a stock macOS Terminal. The box is sized
    /// from the row count and `ui` renders it with no scroll offset, so a list
    /// longer than the box can draw loses its tail without a word — and the
    /// hit-test loses the same rows, so `Quit` was neither drawn nor clickable
    /// on a 26-row screen once the Corner's two new panes had pushed it off.
    ///
    /// The border row is swept for the same defect from the other side:
    /// `Area::holds` includes it and the index counts from `box.y + 1`, so a
    /// clipped list put the *first clipped row* under the bottom border, and a
    /// click on the box's edge opened the cheatsheet.
    #[test]
    fn every_palette_entry_is_clickable_on_a_short_screen() {
        let state = State {
            modal: Modal::Palette,
            ..workspace()
        };
        for height in [24, 26] {
            let panes = panes(120, height, 30, None, 0, 0, Shapes::default());
            let rows = crate::palette_rows(height);
            let box_area = crate::layout::overlay(
                panes.ai.right(),
                panes.tree.height + panes.terminal.height,
                rows.len() as u16,
                rows.iter()
                    .map(|(_, line)| line.chars().count() as u16)
                    .max()
                    .unwrap_or(0),
            );
            let found: Vec<char> = (0..height)
                .flat_map(|row| (0..120).map(move |column| (column, row)))
                .filter_map(|(column, row)| palette_entry_at(&state, &panes, column, row))
                .collect();
            for (_, entries) in crate::PALETTE {
                for (key, entry) in entries {
                    assert!(
                        found.contains(key),
                        "{entry} is unclickable at {height} rows"
                    );
                }
            }
            let border = box_area.bottom().saturating_sub(1);
            for column in 0..120 {
                assert_eq!(
                    palette_entry_at(&state, &panes, column, border),
                    None,
                    "the box's bottom border answers an entry at {height} rows"
                );
            }
        }
    }

    /// A tall pane's border runs the whole height, so the half of it beside
    /// the terminal is a handle too. Grabbing it only alongside the editor
    /// would leave two thirds of the border dead and the pane resizable only
    /// from its top rows.
    #[test]
    fn a_tall_ai_panes_edge_is_grabbed_beside_the_terminal() {
        let state = workspace();
        let panes = panes(
            120,
            26,
            30,
            None,
            0,
            0,
            Shapes {
                ai: AiPane::Tall,
                ..Shapes::default()
            },
        );
        let mut pointer = Pointer::default();
        // Row 20 is in the terminal's band; column 84 is the AI pane's border.
        let outcome = on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDown,
                column: 84,
                row: 20,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(outcome.events.is_empty(), "no click through to the pane");
        assert_eq!(pointer.dragging, Some(Divider::Ai));
        // The tree's border down there is still the terminal's own row: the
        // tree stops above it whatever shape the AI pane is in.
        let mut pointer = Pointer::default();
        on_mouse(
            &state,
            &panes,
            &mut pointer,
            Input {
                kind: Kind::LeftDown,
                column: 29,
                row: 20,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(pointer.dragging, None);
    }

    /// Both handles clamp against the screen, not against whatever the
    /// terminal has left — a tall AI pane takes columns off the terminal, and
    /// reading its width as the screen's shrank the range of both dividers
    /// every time the shape was asked for.
    #[test]
    fn a_tall_ai_pane_does_not_shrink_what_a_divider_can_reach() {
        let state = workspace();
        for divider in [Divider::Tree, Divider::Ai] {
            let reached: Vec<Vec<Event>> = [AiPane::Beside, AiPane::Tall]
                .into_iter()
                .map(|shape| {
                    let panes = panes(
                        120,
                        26,
                        30,
                        None,
                        0,
                        0,
                        Shapes {
                            ai: shape,
                            ..Shapes::default()
                        },
                    );
                    let mut pointer = Pointer {
                        dragging: Some(divider),
                        ..Pointer::default()
                    };
                    on_mouse(
                        &state,
                        &panes,
                        &mut pointer,
                        Input {
                            kind: Kind::LeftDrag,
                            column: 110,
                            row: 1,
                            modifiers: KeyModifiers::NONE,
                        },
                    )
                    .events
                })
                .collect();
            assert_eq!(reached[0], reached[1], "{divider:?} reaches less when tall");
        }
    }

    #[test]
    fn the_ai_pane_edge_cannot_squeeze_the_editor_away() {
        let state = workspace();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        for (column, expected) in [(119u16, 12u32), (0, 70)] {
            let mut pointer = Pointer {
                dragging: Some(Divider::Ai),
                ..Pointer::default()
            };
            let outcome = on_mouse(
                &state,
                &panes,
                &mut pointer,
                Input {
                    kind: Kind::LeftDrag,
                    column,
                    row: 1,
                    modifiers: KeyModifiers::NONE,
                },
            );
            assert_eq!(outcome.events, vec![Event::DragAiDivider(expected)]);
        }
    }

    #[test]
    fn scrolling_follows_the_pointer_and_never_moves_focus() {
        let state = workspace();
        let terminal = panes(120, 26, 30, None, 0, 0, Shapes::default()).terminal;
        let outcome = on_mouse(
            &state,
            &panes(120, 26, 30, None, 0, 0, Shapes::default()),
            &mut Pointer::default(),
            Input {
                kind: Kind::ScrollUp,
                column: terminal.x + 4,
                row: terminal.y + 2,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            outcome.events,
            vec![Event::Scroll {
                pane: Pane::Terminal,
                direction: Direction::Up,
                at: Place { line: 2, column: 4 },
            }]
        );
    }

    /// A move with no button down grabs no divider and finishes no drag: where
    /// it is is all it reports, in the editor's own line and column — and
    /// nothing at all once it is over another pane, which is what takes a
    /// diagnostic box down on the way out.
    #[test]
    fn moving_the_pointer_reports_where_it_rests_and_nothing_else() {
        // A file open, because a place in the text is what is reported and a
        // workspace with nothing open has none.
        let state = editing();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        let moved = |pointer: &mut Pointer, column, row| {
            on_mouse(
                &state,
                &panes,
                pointer,
                Input {
                    kind: Kind::Moved,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )
        };
        let over_text = moved(&mut pointer, 43, 5);
        assert_eq!(
            over_text.events,
            vec![Event::PointerMoved(Some(Place { line: 5, column: 5 }))]
        );
        assert_eq!(
            moved(&mut pointer, 5, 5).events,
            vec![Event::PointerMoved(None)]
        );
        assert_eq!(pointer, Pointer::default(), "a move took hold of something");
    }

    #[test]
    fn right_clicking_does_nothing_beyond_reporting_the_pane() {
        let state = workspace();
        let outcome = on_mouse(
            &state,
            &panes(120, 26, 30, None, 0, 0, Shapes::default()),
            &mut Pointer::default(),
            Input {
                kind: Kind::RightDown,
                column: 40,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(outcome.events, vec![Event::RightClick(Pane::Editor)]);
    }

    #[test]
    fn dragging_in_the_tree_selects_a_row_rather_than_text() {
        let state = workspace();
        let outcome = on_mouse(
            &state,
            &panes(120, 26, 30, None, 0, 0, Shapes::default()),
            &mut Pointer::default(),
            Input {
                kind: Kind::LeftDrag,
                column: 5,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(
            outcome.events,
            vec![Event::DragRow(PathBuf::from("/w/src"))]
        );
        assert!(outcome.select.is_none(), "a filename is not text you copy");
    }

    /// Drags from one screen position to another and reports the last outcome.
    fn drag(state: &State, from: (u16, u16), to: (u16, u16)) -> Outcome {
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        let mut outcome = Outcome::default();
        for (column, row) in [from, to] {
            outcome = on_mouse(
                state,
                &panes,
                &mut pointer,
                Input {
                    kind: Kind::LeftDrag,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            );
        }
        outcome
    }

    fn editing() -> State {
        let mut state = workspace();
        state.current_buffer = Some(PathBuf::from("/w/a.rs"));
        state
    }

    /// A buffer previewing, so `place_in` and `dragged` take the Preview arm:
    /// no gutter, and a row rather than a line-and-column.
    fn previewing() -> State {
        let mut state = workspace();
        let path = PathBuf::from("/w/a.rs");
        state.current_buffer = Some(path.clone());
        state
            .buffers
            .insert(path, crate::editor::Buffer::open("", true, 4));
        state
    }

    /// A diff, which draws the two marker columns as well as the numbers.
    fn diffed() -> State {
        let mut state = workspace();
        state.diff = Some(vec![crate::DiffLine {
            new_line: Some(1),
            old_line: None,
            removed: false,
            text: "x".repeat(40),
        }]);
        state.diff_file = Some("a.rs".to_string());
        state.current_buffer = Some(PathBuf::from("/w/a.rs"));
        state
    }

    // A diff's text starts two columns right of Source's, because the marker
    // that says which side a row came from is gutter too. Measured against
    // Source in the same click, since the bug this guards is the two being
    // measured as one — and the slide is what made it visible: an offset the
    // clamp allowed ran two columns past the end of the longest line.
    #[test]
    fn a_diff_click_accounts_for_the_marker_column() {
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let row = panes.editor.y + 1;
        let column = panes.editor.x + 1 + crate::layout::GUTTER + 2;
        assert_eq!(
            place_in(&diffed(), &panes, Pane::Editor, (column, row)),
            Place { line: 1, column: 1 }
        );
        assert_eq!(
            place_in(&editing(), &panes, Pane::Editor, (column, row)),
            Place { line: 1, column: 3 },
            "Source has no marker, so the same column is two characters further in"
        );
        // And the offset counts from wherever the slide left it, the way it
        // does for Source.
        let mut slid = diffed();
        slid.editor_hscroll = 12;
        assert_eq!(
            place_in(&slid, &panes, Pane::Editor, (column, row)),
            Place {
                line: 1,
                column: 13
            }
        );
    }

    // A Preview draws no line-number gutter, so its first column of text sits
    // `GUTTER` columns to the left of Source's — the bug this guards is a click
    // that still assumes `GUTTER` and picks a character nobody pointed at.
    #[test]
    fn a_preview_click_accounts_for_the_absent_gutter() {
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        // Two columns in from the border: real text in a Preview, since it has
        // no gutter at all, but still inside Source's.
        let column = panes.editor.x + 3;
        let row = panes.editor.y + 1;
        assert_eq!(
            place_in(&previewing(), &panes, Pane::Editor, (column, row)),
            Place { line: 1, column: 3 }
        );
        assert_eq!(
            place_in(&editing(), &panes, Pane::Editor, (column, row)),
            Place { line: 1, column: 1 },
            "still inside Source's gutter, so it clamps to the first column"
        );
    }

    // A Preview drag needs no help from the edge: the rows are already the
    // library's, so the span is a plain `DragText` and never a `Selection`
    // request the way a pty's drag is.
    #[test]
    fn dragging_a_preview_resolves_in_core_without_a_selection_request() {
        let outcome = drag(&previewing(), (36, 1), (40, 3));
        assert_eq!(
            outcome.events,
            // No gutter taken off, so the same screen columns name characters
            // five further into the row than they would in Source.
            vec![Event::DragText {
                from: Place { line: 1, column: 6 },
                to: Place {
                    line: 3,
                    column: 10
                },
            }]
        );
        assert!(outcome.select.is_none(), "the rows are already in-core");
    }

    // The editor's text starts past the border and the line-number gutter, and
    // row 1 of the pane is line 1 of the buffer. Nothing has to be read off a
    // screen for that, so the drag comes back finished.
    #[test]
    fn dragging_in_the_editor_covers_a_span_of_the_buffer() {
        let outcome = drag(&editing(), (39, 1), (43, 3));
        assert_eq!(
            outcome.events,
            vec![Event::DragText {
                from: Place { line: 1, column: 1 },
                to: Place { line: 3, column: 5 },
            }]
        );
        assert!(outcome.select.is_none(), "no pty to read");
    }

    #[test]
    fn dragging_upward_covers_what_dragging_downward_covers() {
        let state = editing();
        assert_eq!(
            drag(&state, (40, 3), (36, 1)).events,
            drag(&state, (36, 1), (40, 3)).events
        );
    }

    // Same bug as the tree's: the renderer scrolls the pane, so the row under
    // the pointer is not the buffer's line unless the offset is counted in.
    #[test]
    fn dragging_in_the_editor_counts_from_the_scrolled_line() {
        let mut scrolled = editing();
        scrolled.editor_scroll = 4;
        assert_eq!(
            drag(&scrolled, (36, 1), (36, 1)).events,
            vec![Event::DragText {
                from: Place { line: 5, column: 1 },
                to: Place { line: 5, column: 1 },
            }]
        );
    }

    // The other half of the same bug: once a long line has slid the view
    // sideways, column 1 of the pane is not column 1 of the line, so a drag or
    // a click that ignores the offset picks characters nobody pointed at.
    #[test]
    fn dragging_in_the_editor_counts_from_the_scrolled_column() {
        let mut scrolled = editing();
        scrolled.editor_hscroll = 12;
        assert_eq!(
            drag(&scrolled, (36, 1), (36, 1)).events,
            vec![Event::DragText {
                from: Place {
                    line: 1,
                    column: 13
                },
                to: Place {
                    line: 1,
                    column: 13
                },
            }]
        );
    }

    // A click in the text is not just a focus change: it takes the caret.
    #[test]
    fn clicking_in_the_editor_reports_the_place_clicked() {
        assert_eq!(
            click(&editing(), 43, 3),
            vec![Event::ClickText(Place { line: 3, column: 5 })]
        );
    }

    // With no buffer open there is no place to click, and with a diff shown
    // there is no cursor to move.
    #[test]
    fn clicking_the_editor_with_nothing_to_edit_only_focuses_it() {
        assert_eq!(
            click(&workspace(), 40, 3),
            vec![Event::ClickPane(Pane::Editor)]
        );
    }

    // A pty has no buffer to anchor to, so the span is a cell in its grid.
    #[test]
    fn dragging_in_the_terminal_asks_for_a_span_in_grid_coordinates() {
        let selection = drag(&workspace(), (11, 19), (10, 20))
            .select
            .expect("a selection request");
        assert_eq!(selection.pane, Pane::Terminal);
        assert_eq!(
            selection.from,
            Place {
                line: 1,
                column: 11
            }
        );
        assert_eq!(
            selection.to,
            Place {
                line: 2,
                column: 10
            }
        );
    }

    // The AI pane fell into the terminal's arm, so a drag in it named cells of
    // the terminal's grid: the edge then read the shell's screen and the
    // session's output could not be copied at all.
    #[test]
    fn dragging_in_the_ai_pane_asks_for_a_span_in_its_own_grid() {
        let ai = panes(120, 26, 30, None, 0, 0, Shapes::default()).ai;
        let selection = drag(&workspace(), (ai.x + 11, ai.y + 1), (ai.x + 10, ai.y + 2))
            .select
            .expect("a selection request");
        assert_eq!(selection.pane, Pane::Ai);
        assert_eq!(
            selection.from,
            Place {
                line: 1,
                column: 11
            }
        );
        assert_eq!(
            selection.to,
            Place {
                line: 2,
                column: 10
            }
        );
    }

    // ADR-0002: an AI CLI runs full-screen, where there is no history behind it,
    // so a span never names a row the pane is not showing.
    #[test]
    fn dragging_in_the_ai_pane_reaches_no_further_than_the_visible_screen() {
        let ai = panes(120, 26, 30, None, 0, 0, Shapes::default()).ai;
        let selection = drag(
            &workspace(),
            (ai.x + 1, ai.y + 1),
            (ai.x + 1, ai.bottom() - 2),
        )
        .select
        .expect("a selection request");
        assert_eq!(selection.to.line, (ai.height - 2) as usize);
    }

    /// Drives a whole gesture and reports every event it produced.
    fn gesture(state: &State, path: &[(Kind, u16, u16)]) -> Vec<Event> {
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        let mut events = Vec::new();
        for &(kind, column, row) in path {
            let input = Input {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            };
            events.extend(on_mouse(state, &panes, &mut pointer, input).events);
        }
        events
    }

    // A press and a release with nothing between them is a click, and the
    // release is what a child that asked for mouse events gets.
    #[test]
    fn a_press_and_release_clicks_through_to_the_pane() {
        let ai = panes(120, 26, 30, None, 0, 0, Shapes::default()).ai;
        let path = [
            (Kind::LeftDown, ai.x + 4, ai.y + 2),
            (Kind::LeftUp, ai.x + 4, ai.y + 2),
        ];
        assert_eq!(
            gesture(&workspace(), &path),
            vec![
                Event::ClickPane(Pane::Ai),
                // The cell in the AI pane's own grid, not on the screen: a
                // child's report knows nothing of the panes around it.
                Event::ClickThrough {
                    pane: Pane::Ai,
                    at: Place { line: 2, column: 4 },
                }
            ]
        );
    }

    // The hazard the release exists to avoid: a drag begins with a press, so
    // forwarding the press would activate whatever the selection started on.
    #[test]
    fn dragging_never_clicks_through_to_the_pane() {
        let ai = panes(120, 26, 30, None, 0, 0, Shapes::default()).ai;
        let path = [
            (Kind::LeftDown, ai.x + 4, ai.y + 2),
            (Kind::LeftDrag, ai.x + 9, ai.y + 3),
            (Kind::LeftUp, ai.x + 9, ai.y + 3),
        ];
        assert!(
            !gesture(&workspace(), &path)
                .iter()
                .any(|event| matches!(event, Event::ClickThrough { .. })),
            "a drag is ours, not the child's"
        );
    }

    /// Drives two presses on the given cells, stamped, through one pointer —
    /// which is what tells a double-click from two clicks.
    fn twice(state: &State, first: (u16, u16, u64), second: (u16, u16, u64)) -> Vec<Event> {
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let mut pointer = Pointer::default();
        let mut events = Vec::new();
        for (column, row, at_ms) in [first, second] {
            pointer.at_ms = at_ms;
            events.extend(
                on_mouse(
                    state,
                    &panes,
                    &mut pointer,
                    Input {
                        kind: Kind::LeftDown,
                        column,
                        row,
                        modifiers: KeyModifiers::NONE,
                    },
                )
                .events,
            );
        }
        events
    }

    // The second press on a cell is a double-click only inside the same window
    // a double-tap has, and only on the same cell: two clicks a reader made
    // minutes apart, or on two different words, are two clicks.
    #[test]
    fn two_presses_on_one_cell_inside_the_window_pick_the_word() {
        let state = editing();
        let doubled = Event::DoubleClickText(Place { line: 3, column: 5 });
        assert!(twice(&state, (43, 3, 0), (43, 3, 299)).contains(&doubled));
        assert!(!twice(&state, (43, 3, 0), (43, 3, 301)).contains(&doubled));
        assert!(!twice(&state, (44, 3, 0), (43, 3, 100)).contains(&doubled));
    }

    // Nothing to pick where a press names no place in the text: the pane, a
    // row of the tree and a control on the border are all clicks that happen
    // twice rather than a word under the pointer.
    #[test]
    fn two_presses_where_there_is_no_text_pick_no_word() {
        let state = workspace();
        assert!(!twice(&state, (40, 3, 0), (40, 3, 100))
            .iter()
            .any(|event| matches!(event, Event::DoubleClickText(_))));
    }

    /// A buffer with a foldable block, folded, so the gutter carries a toggle
    /// and the line that opens it carries the dots.
    fn folding() -> State {
        let mut state = workspace();
        let path = PathBuf::from("/w/a.rs");
        let mut buffer = crate::editor::Buffer::open("fn main() {\n    go();\n}\n", false, 4);
        crate::fold::toggle(&mut buffer, false);
        state.buffers.insert(path.clone(), buffer);
        state.current_buffer = Some(path);
        state
    }

    /// The two halves of one affordance: the toggle in the gutter's pad and
    /// the dots at the end of the folded line both open the block, and both
    /// place the caret first so the block toggled is the one it is on.
    ///
    /// A gutter column that is *not* the toggle's stays a plain click, or
    /// every press on a line number would fold something.
    #[test]
    fn pressing_a_fold_toggle_or_its_dots_opens_the_block() {
        let state = folding();
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let toggle = panes.editor.x + 1 + crate::layout::TOGGLE_COLUMN;
        assert_eq!(
            click(&state, toggle, panes.editor.y + 1),
            vec![
                Event::ClickText(Place { line: 1, column: 1 }),
                Event::ToggleFold { all: false }
            ]
        );
        // `fn main() {` is eleven characters, so the dots are the twelfth.
        let dots = panes.editor.x + 1 + crate::layout::GUTTER + 11;
        assert_eq!(
            click(&state, dots, panes.editor.y + 1),
            vec![
                Event::ClickText(Place {
                    line: 1,
                    column: 12
                }),
                Event::ToggleFold { all: false }
            ]
        );
        assert_eq!(
            click(&state, panes.editor.x + 2, panes.editor.y + 1),
            vec![Event::ClickText(Place { line: 1, column: 1 })],
            "a line number is not a toggle"
        );
        assert_eq!(
            click(&state, toggle, panes.editor.y + 2),
            vec![Event::ClickText(Place { line: 3, column: 1 })],
            "and neither is the pad of a line that opens no block"
        );
    }

    /// A terminal has no hand pointer, so resting on an icon is what says it
    /// is one. Reported by name and only when the answer changes: a pointer
    /// crossing a pane sends a report per cell, and each one reaching `update`
    /// would be a frame.
    #[test]
    fn resting_on_an_action_icon_names_it_and_leaving_takes_it_back() {
        let mut state = workspace();
        state.tree_selection = Some(PathBuf::from("/w/a.rs"));
        let panes = panes(120, 26, 30, None, 0, 0, Shapes::default());
        let moved = |state: &State, column, row| {
            on_mouse(
                state,
                &panes,
                &mut Pointer::default(),
                Input {
                    kind: Kind::Moved,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )
            .events
        };
        // The last of the two icons a file offers, hard against the border.
        let icon = panes.tree.x + panes.tree.width - 3;
        let row = panes.tree.y + 2;
        assert!(moved(&state, icon, row).contains(&Event::HoverAction(Some("copy-path"))));
        state.hovered_action = Some("copy-path");
        assert!(
            !moved(&state, icon, row)
                .iter()
                .any(|event| matches!(event, Event::HoverAction(_))),
            "the same answer is not news"
        );
        assert!(moved(&state, panes.tree.x + 2, row).contains(&Event::HoverAction(None)));
    }

    #[test]
    fn a_click_outside_every_pane_does_nothing() {
        assert!(click(&workspace(), 200, 5).is_empty());
    }
}
