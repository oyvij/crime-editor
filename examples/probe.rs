use ratatui::layout::{Constraint, Direction, Layout, Rect};

fn main() {
    for (w, h, divider) in [
        (120u16, 26u16, 30u16),
        (100, 26, 30),
        (80, 24, 45),
        (200, 50, 30),
    ] {
        let area = Rect::new(0, 0, w, h);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Percentage(30)])
            .split(area);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(divider),
                Constraint::Min(20),
                Constraint::Percentage(30),
            ])
            .split(rows[0]);
        println!(
            "{w}x{h} div={divider}: tree={:?} editor={:?} ai={:?} term={:?}",
            (cols[0].x, cols[0].y, cols[0].width, cols[0].height),
            (cols[1].x, cols[1].y, cols[1].width, cols[1].height),
            (cols[2].x, cols[2].y, cols[2].width, cols[2].height),
            (rows[1].x, rows[1].y, rows[1].width, rows[1].height),
        );
    }
}
