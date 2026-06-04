use crate::app::{App, LogKind};
use crate::ui::{theme::THEME, widgets};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let action_label = app.selected_action.map(|a| a.label()).unwrap_or("repair");

    let title = if app.current_screen == crate::app::CurrentScreen::DiagnoseLog {
        "SYSTEM DIAGNOSIS"
    } else {
        &action_label.to_uppercase()
    };

    let chunks = widgets::draw_layout(f, title);

    let distro = app.heuristic_distro();

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(action_label, Style::default().fg(THEME.foreground)),
            Span::styled(" — ", Style::default().fg(THEME.comment)),
            Span::styled(
                distro.to_lowercase(),
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    // Filter step lines to show at the top
    for log in &app.log_lines {
        match log.kind {
            LogKind::Step => {
                lines.push(Line::from(vec![
                    Span::styled("▸ ", Style::default().fg(THEME.comment)),
                    Span::styled(&log.text, Style::default().fg(THEME.comment)),
                ]));
            }
            LogKind::Ok => {
                if let Some(last) = lines.last_mut() {
                    last.spans
                        .push(Span::styled("   ✓ done", Style::default().fg(THEME.green)));
                }
            }
            LogKind::Warn => {
                if let Some(last) = lines.last_mut() {
                    last.spans.push(Span::styled(
                        "   ⚠ warning",
                        Style::default().fg(THEME.yellow),
                    ));
                }
            }
            LogKind::Error => {
                if let Some(last) = lines.last_mut() {
                    last.spans
                        .push(Span::styled("   ✗ failed", Style::default().fg(THEME.red)));
                }
            }
            _ => {}
        }
    }

    lines.push(Line::from(""));

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(chunks[1]);

    let log_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(lines.len() as u16), Constraint::Min(0)])
        .split(main_layout[0]);

    f.render_widget(Paragraph::new(lines), log_layout[0]);

    // Command output box
    let mut output_lines: Vec<Line> = Vec::new();
    for log in &app.log_lines {
        if log.kind == LogKind::Output {
            if log.text.starts_with('$') {
                output_lines.push(Line::from(Span::styled(
                    &log.text,
                    Style::default().fg(THEME.comment),
                )));
            } else {
                output_lines.push(Line::from(Span::styled(
                    &log.text,
                    Style::default().fg(THEME.foreground),
                )));
            }
        }
    }

    let output_block = Paragraph::new(output_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" command output ")
                .border_style(Style::default().fg(THEME.comment)),
        )
        .style(Style::default().bg(ratatui::style::Color::Rgb(22, 27, 34))); // Darker bg for output box

    f.render_widget(output_block, log_layout[1]);

    // Progress bar
    let progress_pct = if app.exec_total > 0 {
        ((app.exec_step as f32 / app.exec_total as f32) * 100.0) as u16
    } else {
        0
    };

    let bar_width = main_layout[1].width as usize - 40;
    let filled_width = ((progress_pct as f32 / 100.0) * bar_width as f32) as usize;
    let empty_width = bar_width.saturating_sub(filled_width);

    let status_text = if app.exec_done {
        "repair complete"
    } else {
        "running..."
    };

    let progress_line = Line::from(vec![
        Span::styled("█".repeat(filled_width), Style::default().fg(THEME.cyan)),
        Span::styled("░".repeat(empty_width), Style::default().fg(THEME.comment)),
        Span::styled(
            format!(
                " step {} of {} · {}",
                app.exec_step, app.exec_total, status_text
            ),
            Style::default().fg(THEME.comment),
        ),
    ]);

    f.render_widget(Paragraph::new(progress_line), main_layout[1]);
}
