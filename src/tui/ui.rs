//! Top-level render function: title bar, tab bar, main area, status bar.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use super::app::{App, Tab};
use super::tabs;

/// Render the full UI into the given frame.
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_title(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_main(f, app, chunks[2]);
    render_status(f, app, chunks[3]);

    if app.show_help {
        render_help(f);
    }
}

fn render_title(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title_text = format!(" logi-tune v0.1.0 — {}", app.device_name);
    let battery_text = format_battery(app);
    let battery_width = battery_text
        .iter()
        .map(|s| s.content.len() as u16)
        .sum::<u16>();

    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(battery_width + 1)]).areas(area);

    let title_para =
        Paragraph::new(title_text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(title_para, left);

    let battery_para =
        Paragraph::new(Line::from(battery_text)).style(Style::default().bg(Color::DarkGray));
    f.render_widget(battery_para, right);
}

/// Build the battery display spans.
fn format_battery(app: &App) -> Vec<Span<'static>> {
    let Some(ref b) = app.battery else {
        return vec![];
    };

    let level = match b.level {
        Some(l) => l,
        None => return vec![Span::raw("??? ")],
    };

    let filled = (level as usize * 10 / 100).min(10);
    let bar: String = "█".repeat(filled) + &"░".repeat(10 - filled);

    let bar_color = if b.charging || b.charge_complete {
        Color::Cyan
    } else if level > 50 {
        Color::Green
    } else if level > 20 {
        Color::Yellow
    } else {
        Color::Red
    };

    let prefix = if b.charge_complete {
        Span::styled("✓ ", Style::default().fg(Color::Cyan).bg(Color::DarkGray))
    } else if b.charging {
        Span::styled("⚡ ", Style::default().fg(Color::Cyan).bg(Color::DarkGray))
    } else {
        Span::raw("")
    };

    vec![
        prefix,
        Span::styled(bar, Style::default().fg(bar_color).bg(Color::DarkGray)),
        Span::styled(
            format!(" {}% ", level),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
    ]
}

fn render_tabs(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let titles = vec!["Buttons", "Scroll", "Thumb Wheel", "DPI"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.active_tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn render_main(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    match app.active_tab {
        Tab::Buttons => tabs::buttons::render(f, app, area),
        Tab::Scroll => tabs::scroll::render(f, app, area),
        Tab::ThumbWheel => tabs::thumb_wheel::render(f, app, area),
        Tab::Dpi => tabs::dpi::render(f, app, area),
    }
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn render_status(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let hint = "q:Quit  s:Save  a:Apply  r:Reset  ?:Help  Tab:Switch";

    let mut spans = vec![];

    if app.applying {
        let frame = SPINNER[(app.tick as usize / 2) % SPINNER.len()];
        spans.push(Span::styled(
            format!(" {} Applying… ", frame),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(ref msg) = app.status {
        spans.push(Span::raw(format!(" {} ", msg)));
    } else {
        spans.push(Span::styled(
            format!(" {} ", hint),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if app.dirty && !app.applying {
        spans.push(Span::styled(
            " [*]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    f.render_widget(paragraph, area);
}

fn render_help(f: &mut Frame) {
    use ratatui::{layout::Rect, widgets::Clear};

    let area = f.area();
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 14u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(""),
        Line::from("  Tab / Shift+Tab  Cycle tabs"),
        Line::from("  s                Save config"),
        Line::from("  a                Apply to device"),
        Line::from("  r                Reset current tab"),
        Line::from("  q / Esc          Quit"),
        Line::from("  ?                Toggle this help"),
        Line::from(""),
        Line::from("  Buttons tab:"),
        Line::from("    ↑↓ navigate  Enter open picker"),
        Line::from("  Scroll / Thumb:"),
        Line::from("    ↑↓ row  ←→ adjust  Space toggle"),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().bg(Color::DarkGray));

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}
