//! Buttons tab: display button list and action-picker popup.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::config::ButtonAction;
use crate::tui::app::{App, InputMode};

const BUTTON_NAMES: [&str; 4] = [
    "Middle Click",
    "Back",
    "Forward",
    "Thumb Button",
];

fn button_action(app: &App, idx: usize) -> &ButtonAction {
    match idx {
        0 => &app.config.buttons.middle_button,
        1 => &app.config.buttons.back_button,
        2 => &app.config.buttons.forward_button,
        3 => &app.config.buttons.thumb_button,
        _ => &app.config.buttons.middle_button,
    }
}

const ACTION_NAMES: [&str; 8] = [
    "1. Default",
    "2. Key Combo …",
    "3. Execute command …",
    "4. Toggle Scroll Mode",
    "5. DPI Up",
    "6. DPI Down",
    "7. Disabled",
    "8. Smart Combo (app-aware) …",
];

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Buttons ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = BUTTON_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let action = button_action(app, i);
            let selected = i == app.button_selected;
            let style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<20}", name), style),
                Span::styled(format!("{}", action), style.fg(Color::Gray)),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.button_selected));

    let list = List::new(items)
        .block(Block::default())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, inner, &mut list_state);

    match &app.input_mode.clone() {
        InputMode::ActionPicker => render_action_picker(f, app, area),
        InputMode::TextInput { prompt, buffer } => {
            render_text_input(f, prompt, buffer, area)
        }
        InputMode::Normal => {}
    }
}

fn render_action_picker(f: &mut Frame, app: &App, parent: Rect) {
    let popup_width = 36u16.min(parent.width.saturating_sub(4));
    let popup_height = (ACTION_NAMES.len() as u16 + 2).min(parent.height.saturating_sub(4));
    let x = parent.x + (parent.width.saturating_sub(popup_width)) / 2;
    let y = parent.y + (parent.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = ACTION_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == app.action_picker_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(format!(" {} ", name), style)))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.action_picker_index));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Choose Action ");
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let list = List::new(items).highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    f.render_stateful_widget(list, inner, &mut list_state);
}

fn render_text_input(f: &mut Frame, prompt: &str, buffer: &str, parent: Rect) {
    let popup_width = 50u16.min(parent.width.saturating_sub(4));
    let popup_height = 5u16.min(parent.height.saturating_sub(4));
    let x = parent.x + (parent.width.saturating_sub(popup_width)) / 2;
    let y = parent.y + (parent.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Input ");
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let text = vec![
        Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!("> {}█", buffer),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            "Enter to confirm, Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text);
    f.render_widget(paragraph, inner);
}
