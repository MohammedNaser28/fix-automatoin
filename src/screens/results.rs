use crate::app::{App, LogKind};
use crate::ui::{theme::THEME, widgets};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = widgets::draw_layout(f, "REPAIR COMPLETE");

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Success header
            Constraint::Min(0),    // Checklist
            Constraint::Length(1), // Divider
            Constraint::Length(4), // Buttons
        ])
        .split(chunks[1]);

    // ── Success Header ────────────────────────────────────────────────────────
    let header_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "✓",
            Style::default()
                .fg(THEME.green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "System repair completed successfully",
            Style::default()
                .fg(THEME.foreground)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    f.render_widget(
        Paragraph::new(header_lines)
            .alignment(Alignment::Center)
            .style(Style::default().bg(THEME.background)),
        main_layout[0],
    );

    // ── Checklist ─────────────────────────────────────────────────────────────
    let mut checklist: Vec<Line> = Vec::new();
    for log in &app.log_lines {
        if log.kind == LogKind::Step {
            checklist.push(Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(THEME.green)),
                Span::styled(log.text.clone(), Style::default().fg(THEME.comment)),
            ]));
        }
    }
    checklist.push(Line::from(""));

    // Add a summary
    let action_label = app.selected_action.map(|a| a.label()).unwrap_or("repair");
    checklist.push(Line::from(vec![
        Span::styled("  action performed: ", Style::default().fg(THEME.comment)),
        Span::styled(action_label, Style::default().fg(THEME.cyan)),
    ]));

    f.render_widget(Paragraph::new(checklist), main_layout[1]);

    // ── Divider ───────────────────────────────────────────────────────────────
    let divider = Paragraph::new("─".repeat(chunks[1].width as usize))
        .style(Style::default().fg(THEME.comment));
    f.render_widget(divider, main_layout[2]);

    // ── Buttons ───────────────────────────────────────────────────────────────
    let btn_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Percentage(22), // back to menu
            Constraint::Percentage(1),
            Constraint::Percentage(22), // reboot
            Constraint::Percentage(1),
            Constraint::Percentage(22), // poweroff
            Constraint::Percentage(1),
            Constraint::Percentage(22), // export logs
            Constraint::Percentage(5),
        ])
        .split(main_layout[3]);

    let options = [
        ("back to menu", 1),
        ("reboot", 3),
        ("poweroff", 5),
        ("export logs", 7),
    ];

    for (i, (label, col_idx)) in options.iter().enumerate() {
        let is_selected = app.result_cursor == i;
        let (style, prefix) = if is_selected {
            (
                Style::default()
                    .bg(THEME.selection)
                    .fg(THEME.cyan)
                    .add_modifier(Modifier::BOLD),
                "▶ ",
            )
        } else {
            (Style::default().fg(THEME.comment), "  ")
        };

        let btn = Paragraph::new(format!("\n{}{}", prefix, label))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).style(style));

        f.render_widget(btn, btn_layout[*col_idx]);
    }
}
