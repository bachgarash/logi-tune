//! DPI tab: profile list with Unicode bar charts and active profile selection.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" DPI Profiles ");
    let inner = outer_block.inner(area);
    f.render_widget(outer_block, area);

    // Split: left = profile list, right = bar chart
    // Split: left = profile list, right = bar chart
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    render_profile_list(f, app, columns[0]);
    render_bar_chart(f, app, columns[1]);
    render_hints(f, inner);
}

fn render_profile_list(f: &mut Frame, app: &App, area: Rect) {
    // Reserve last 2 rows for hints
    let list_area = Rect {
        height: area.height.saturating_sub(2),
        ..area
    };

    let mut lines: Vec<Line> = Vec::new();
    for (i, &dpi) in app.config.dpi.profiles.iter().enumerate() {
        let is_active = i == app.config.dpi.active;
        let is_selected = i == app.dpi_selected;

        let marker = if is_active { "●" } else { "○" };

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(Span::styled(
            format!(" {} {:>5} DPI", marker, dpi),
            style,
        )));
    }

    let paragraph = Paragraph::new(lines).block(Block::default().title("Profiles"));
    f.render_widget(paragraph, list_area);
}

fn render_bar_chart(f: &mut Frame, app: &App, area: Rect) {
    const MAX_DPI: u16 = 8000;
    // Leave a column of space and a title
    let chart_width = area.width.saturating_sub(4) as usize;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " DPI bar chart",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for (i, &dpi) in app.config.dpi.profiles.iter().enumerate() {
        let is_active = i == app.config.dpi.active;
        let is_selected = i == app.dpi_selected;

        let filled = ((dpi as usize) * chart_width) / (MAX_DPI as usize);
        let bar: String = "█".repeat(filled.min(chart_width));

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" {:>5} ", dpi), style),
            Span::styled(bar, style),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);
}

fn render_hints(f: &mut Frame, area: Rect) {
    if area.height < 2 {
        return;
    }
    let hint_area = Rect {
        y: area.y + area.height.saturating_sub(1),
        height: 1,
        ..area
    };

    let hint = " ↑↓ select  ←→ adjust DPI  Enter set active  + add  - remove";
    let paragraph = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(paragraph, hint_area);
}
