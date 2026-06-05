use crate::ui::theme::THEME;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
};

pub fn draw_layout(f: &mut Frame, title: &str) -> Vec<Rect> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main Content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Render Global Header
    let header = Paragraph::new(format!(" GRUB-RESCUE v0.1.0 | Current: {}", title))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(THEME.green),
        )
        .style(THEME.cyan);
    f.render_widget(header, chunks[0]);

    // Render Global Footer
    let footer =
        Paragraph::new(" [Q] Quit - [P] Poweroff - [R] Reboot | [Enter] Next - [Esc] Back ").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(THEME.comment),
        );
    f.render_widget(footer, chunks[2]);

    chunks.to_vec()
}
