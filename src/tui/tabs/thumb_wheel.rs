//! Thumb wheel tab: invert toggle and sensitivity bar.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Thumb Wheel ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows: Vec<Line> = vec![
        render_invert(app),
        Line::from(""),
        render_sensitivity(app),
    ];

    let paragraph = Paragraph::new(rows);
    f.render_widget(paragraph, inner);
}

// ---------------------------------------------------------------------------
// Row renderers
// ---------------------------------------------------------------------------

fn selected_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn render_invert(app: &App) -> Line<'static> {
    let selected = app.thumb_selected == 0;
    let style = selected_style(selected);
    let state = if app.config.thumb_wheel.invert { "ON " } else { "OFF" };
    let label = format!("  Invert:       {}   Space toggle", state);
    Line::from(Span::styled(label, style))
}

fn render_sensitivity(app: &App) -> Line<'static> {
    let selected = app.thumb_selected == 1;
    let style = selected_style(selected);
    let sens = app.config.thumb_wheel.sensitivity as usize;

    // Visual bar: 10 cells
    let bar: String = "█".repeat(sens) + &"░".repeat(10 - sens);

    let label = format!(
        "  Sensitivity:  {:>2}/10  [{}]  ←→ adjust",
        sens, bar
    );
    Line::from(Span::styled(label, style))
}
