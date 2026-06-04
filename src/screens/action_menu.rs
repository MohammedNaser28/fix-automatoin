use crate::app::{ACTION_ITEMS, App};
use crate::ui::{theme::THEME, widgets};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = widgets::draw_layout(f, "ACTION MENU");

    // Title line: distro + selected partition
    let distro = app.heuristic_distro();
    let root_name = app
        .selected_root
        .as_ref()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "?".into());

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                distro.to_lowercase(),
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(THEME.comment)),
            Span::styled(
                format!("/dev/{}", root_name),
                Style::default().fg(THEME.cyan),
            ),
        ]),
        Line::from(""),
    ];

    if !app.diagnosis_summary.is_empty() {
        for msg in &app.diagnosis_summary {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(msg.clone(), Style::default().fg(THEME.yellow)),
            ]));
        }
        lines.push(Line::from(""));
    }

    let section_labels = ["repair", "disk", "help"];
    let mut section_idx = 0;

    for (i, item) in ACTION_ITEMS.iter().enumerate() {
        match item {
            None => {
                let label = section_labels.get(section_idx).copied().unwrap_or("misc");
                section_idx += 1;
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", label),
                    Style::default().fg(THEME.comment),
                )]));
            }
            Some(action) => {
                let is_selected = i == app.action_cursor;
                let is_available = action.is_available();

                let label_style = if is_selected {
                    Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD)
                } else if is_available {
                    Style::default().fg(THEME.foreground)
                } else {
                    Style::default().fg(THEME.comment)
                };

                let arrow = if is_selected {
                    Span::styled("  ▶ ", Style::default().fg(THEME.orange))
                } else {
                    Span::raw("    ")
                };

                // Build tag or description span
                let is_recommended = app.recommended_action.as_ref() == Some(action);

                let tag_span = match action {
                    _ if is_recommended => {
                        Span::styled(" [recommended]", Style::default().fg(THEME.green))
                    }
                    crate::app::Action::OpenChrootShell => {
                        Span::styled(" [advanced]", Style::default().fg(THEME.yellow))
                    }
                    a if !a.is_available() => {
                        Span::styled(" [post-MVP]", Style::default().fg(THEME.comment))
                    }
                    _ => Span::styled(
                        format!("  {}", action.description()),
                        Style::default().fg(THEME.comment),
                    ),
                };

                lines.push(Line::from(vec![
                    arrow,
                    Span::styled(format!("{:<22}", action.label()), label_style),
                    tag_span,
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  [Tab/↑↓] navigate   [Enter] select   [Esc] back",
        Style::default().fg(THEME.comment),
    )]));

    f.render_widget(Paragraph::new(lines), chunks[1]);

    // If the selected action is post-MVP, show a hint in the layout spacer
    let _ = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0)])
        .split(chunks[1]);
}
