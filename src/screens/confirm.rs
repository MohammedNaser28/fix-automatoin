use crate::app::{App, ConfirmFocus};
use crate::ui::{theme::THEME, widgets};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = widgets::draw_layout(f, "CONFIRM REPAIR TARGETS");

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(chunks[1]);

    // ── Summary block ─────────────────────────────────────────────────────────
    let root_name = app
        .selected_root
        .as_ref()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "—".into());
    let root_fs = app
        .selected_root
        .as_ref()
        .and_then(|d| d.fstype.clone())
        .unwrap_or_else(|| "?".into());
    let root_size = app
        .selected_root
        .as_ref()
        .map(|d| d.size.clone())
        .unwrap_or_else(|| "?".into());
    let efi_name = app
        .selected_efi
        .as_ref()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "—".into());
    let efi_fs = app
        .selected_efi
        .as_ref()
        .and_then(|d| d.fstype.clone())
        .unwrap_or_else(|| "vfat".into());
    let efi_size = app
        .selected_efi
        .as_ref()
        .map(|d| d.size.clone())
        .unwrap_or_else(|| "?".into());
    let distro = app.heuristic_distro();
    let firmware = if app.is_uefi { "UEFI" } else { "BIOS" };
    let grub_tgt = if app.is_uefi { "x86_64-efi" } else { "i386-pc" };

    let label_style = Style::default().fg(THEME.comment);
    let pad = "  ";

    let summary = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}root partition   ", pad), label_style),
            Span::styled(
                format!("/dev/{}", root_name),
                Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(root_fs, Style::default().fg(THEME.green)),
            Span::raw("  "),
            Span::styled(root_size, label_style),
        ]),
        Line::from(vec![
            Span::styled(format!("{}efi partition    ", pad), label_style),
            Span::styled(
                format!("/dev/{}", efi_name),
                Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(efi_fs, Style::default().fg(THEME.green)),
            Span::raw("  "),
            Span::styled(efi_size, label_style),
        ]),
        Line::from(vec![
            Span::styled(format!("{}distro detected  ", pad), label_style),
            Span::styled(
                distro,
                Style::default()
                    .fg(THEME.purple)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(format!("{}firmware         ", pad), label_style),
            Span::styled(
                firmware,
                Style::default()
                    .fg(THEME.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  + BIOS fallback", label_style),
        ]),
        Line::from(vec![
            Span::styled(format!("{}grub target      ", pad), label_style),
            Span::styled(
                grub_tgt,
                Style::default()
                    .fg(THEME.foreground)
                    .add_modifier(Modifier::BOLD),
            ),
            if app.is_uefi {
                Span::styled("  + i386-pc", label_style)
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
    ];

    f.render_widget(Paragraph::new(summary), main_layout[0]);

    // ── Divider ───────────────────────────────────────────────────────────────
    let button_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(main_layout[1]);

    let divider_width = chunks[1].width as usize;
    f.render_widget(
        Paragraph::new("─".repeat(divider_width)).style(Style::default().fg(THEME.comment)),
        button_layout[0],
    );

    // ── Buttons ───────────────────────────────────────────────────────────────
    let (confirm_style, back_style) = match app.confirm_focus {
        ConfirmFocus::Confirm => (
            Style::default()
                .bg(THEME.selection)
                .fg(THEME.cyan)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(THEME.comment),
        ),
        ConfirmFocus::Back => (
            Style::default().fg(THEME.comment),
            Style::default()
                .bg(THEME.selection)
                .fg(THEME.cyan)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let btn_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(13),
            Constraint::Length(2),
            Constraint::Length(13),
            Constraint::Min(0),
        ])
        .split(button_layout[1]);

    f.render_widget(
        Paragraph::new("\n confirm")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).style(confirm_style)),
        btn_row[1],
    );
    f.render_widget(
        Paragraph::new("\n back")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).style(back_style)),
        btn_row[3],
    );
}
