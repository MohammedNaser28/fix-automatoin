use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use crate::app::{App, ScanState};
use crate::ui::theme::THEME;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let base_style = Style::default().bg(THEME.background).fg(THEME.foreground);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top bar
            Constraint::Length(1), // divider
            Constraint::Min(0),    // main content
            Constraint::Length(1), // divider
            Constraint::Length(4), // footer + button
        ])
        .split(area);

    // Background fill
    f.render_widget(Block::default().style(base_style), area);

    // Top bar
    f.render_widget(
        Paragraph::new("grub-rescue v0.1.0 — rescue USB")
            .style(Style::default().bg(THEME.background).fg(THEME.comment)),
        chunks[0],
    );

    // Dividers
    let divider = Paragraph::new("─".repeat(area.width as usize))
        .style(Style::default().bg(THEME.background).fg(THEME.comment));
    f.render_widget(divider.clone(), chunks[1]);
    f.render_widget(divider,         chunks[3]);

    // ── Main content ─────────────────────────────────────────────────────────
    let disk_count = app.disks.len();
    let firmware_label = if app.is_uefi { "UEFI" } else { "BIOS" };
    let arch_tag       = if app.is_uefi { " x86_64-efi " } else { " i386-pc " };

    let network_line = match &app.network_info {
        Some(ip) => Line::from(vec![
            Span::styled("✓ ", Style::default().fg(THEME.green)),
            Span::raw("network: "),
            Span::styled("DHCP acquired  ", Style::default().fg(THEME.yellow)),
            Span::styled(ip.clone(), Style::default().fg(THEME.comment)),
        ]),
        None => Line::from(vec![
            Span::styled("✗ ", Style::default().fg(THEME.red)),
            Span::styled("no network detected", Style::default().fg(THEME.comment)),
        ]),
    };

    let scan_line = if app.scan_state == ScanState::Scanning {
        Line::from(vec![
            Span::styled("⟳ ", Style::default().fg(THEME.yellow)),
            Span::styled("scanning block devices...", Style::default().fg(THEME.yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(THEME.green)),
            Span::raw("found "),
            Span::styled(
                disk_count.to_string(),
                Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" block devices"),
        ])
    };

    let main_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "GRUB RESCUE",
            Style::default().fg(THEME.cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("bootable repair tool", Style::default().fg(THEME.comment))),
        Line::from(""),
        scan_line,
        Line::from(""),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(THEME.green)),
            Span::raw("detected firmware: "),
            Span::styled(firmware_label, Style::default().fg(THEME.yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(arch_tag, Style::default().bg(THEME.selection).fg(THEME.cyan)),
        ]),
        network_line,
    ];

    f.render_widget(
        Paragraph::new(main_lines)
            .alignment(Alignment::Center)
            .style(Style::default().bg(THEME.background)),
        chunks[2],
    );

    // ── Footer / button ───────────────────────────────────────────────────────
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(chunks[4]);

    f.render_widget(
        Paragraph::new("press enter to continue")
            .style(Style::default().bg(THEME.background).fg(THEME.comment))
            .alignment(Alignment::Center),
        footer_chunks[0],
    );

    let btn_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(43),
            Constraint::Percentage(14),
            Constraint::Percentage(43),
        ])
        .split(footer_chunks[2]);

    f.render_widget(
        Paragraph::new("continue →")
            .style(
                Style::default()
                    .bg(THEME.selection)
                    .fg(THEME.cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        btn_split[1],
    );
}