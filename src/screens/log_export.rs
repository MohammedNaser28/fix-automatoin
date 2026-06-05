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
    let chunks = widgets::draw_layout(f, "EXPORT LOGS");

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header/Instructions
            Constraint::Min(0),    // Log text box
            Constraint::Length(1), // Divider
            Constraint::Length(4), // Buttons
        ])
        .split(chunks[1]);

    // ── Header ────────────────────────────────────────────────────────
    let header_lines = vec![
        Line::from(""),
        Line::from(vec![Span::raw(
            "  To share these logs for troubleshooting, run this command in another TTY:",
        )]),
        Line::from(vec![Span::styled(
            "  $ cat /var/log/fix-automation.log | curl -F 'f:1=<-' ix.io",
            Style::default().fg(THEME.cyan),
        )]),
        Line::from(""),
    ];

    f.render_widget(Paragraph::new(header_lines), main_layout[0]);

    // ── Log Text Box ─────────────────────────────────────────────────────────────
    let mut log_text: Vec<Line> = Vec::new();
    for log in &app.log_lines {
        let style = match log.kind {
            LogKind::Step => Style::default().fg(THEME.comment),
            LogKind::Output => Style::default().fg(THEME.foreground),
            LogKind::Ok => Style::default().fg(THEME.green),
            LogKind::Warn => Style::default().fg(THEME.yellow),
            LogKind::Error => Style::default().fg(THEME.red),
            _ => Style::default(),
        };
        log_text.push(Line::from(Span::styled(log.text.clone(), style)));
    }

    let log_block = Paragraph::new(log_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" execution log ")
                .border_style(Style::default().fg(THEME.comment)),
        )
        .style(Style::default().bg(ratatui::style::Color::Black));

    f.render_widget(log_block, main_layout[1]);

    // ── Divider ───────────────────────────────────────────────────────────────
    let divider = Paragraph::new("-".repeat(chunks[1].width as usize))
        .style(Style::default().fg(THEME.comment));
    f.render_widget(divider, main_layout[2]);

    // ── Buttons ───────────────────────────────────────────────────────────────
    let btn_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30), // back
            Constraint::Percentage(35),
        ])
        .split(main_layout[3]);

    let btn = Paragraph::new("\n> go back")
        .alignment(Alignment::Center)
        .block(
            Block::default().borders(Borders::ALL).style(
                Style::default()
                    .bg(THEME.selection)
                    .fg(THEME.cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );

    f.render_widget(btn, btn_layout[1]);
}
