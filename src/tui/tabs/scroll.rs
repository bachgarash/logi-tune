//! Scroll wheel tab: lines-per-notch, invert toggle, and SmartShift threshold gauge.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Scroll Wheel ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows: Vec<Line> = vec![
        render_lines_per_notch(app),
        Line::from(""),
        render_invert(app),
        Line::from(""),
        render_smart_shift(app),
    ];

    let paragraph = Paragraph::new(rows);
    f.render_widget(paragraph, inner);
}

fn selected_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn render_lines_per_notch(app: &App) -> Line<'static> {
    let selected = app.scroll_selected == 0;
    let style = selected_style(selected);
    let label = format!(
        "  Lines per notch:  {} / 10   ←→ adjust",
        app.config.scroll.lines_per_notch
    );
    Line::from(Span::styled(label, style))
}

fn render_invert(app: &App) -> Line<'static> {
    let selected = app.scroll_selected == 1;
    let style = selected_style(selected);
    let state = if app.config.scroll.invert {
        "ON "
    } else {
        "OFF"
    };
    let label = format!("  Invert scroll:     {}        Space toggle", state);
    Line::from(Span::styled(label, style))
}

fn render_smart_shift(app: &App) -> Line<'static> {
    let selected = app.scroll_selected == 2;
    let style = selected_style(selected);
    let threshold = app.config.scroll.smart_shift_threshold;

    // Visual gauge: 20 blocks, proportional to 0–255
    let filled = (threshold as usize * 20) / 255;
    let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);

    let label = format!(
        "  SmartShift threshold: {:>3}  [{}]  ←→ adjust",
        threshold, bar
    );
    Line::from(Span::styled(label, style))
}
