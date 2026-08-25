use crate::app::App;
use crate::ui::theme::THEME;
use crate::ui::widgets;
use ratatui::{
    Frame,
    layout::Constraint,
    style::Style,
    widgets::{Block, Cell, Paragraph, Row, Table},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = widgets::draw_layout(f, "SELECT EFI PARTITION");

    // 1. Count the filtered items dynamically to set safe navigation boundaries
    let efi_count = app.disks.iter().filter(|d| d.is_efi).count();

    if efi_count == 0 {
        // Empty state - never leave the user on a blank screen with no way forward
        let msg = vec![
            ratatui::text::Line::from(""),
            ratatui::text::Line::from("no EFI system partitions found")
                .style(Style::default().fg(THEME.yellow)),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(
                "[Enter] continue without EFI   [Esc] back to root selection",
            ),
        ];
        let p = Paragraph::new(msg)
            .block(Block::default().title("choose the EFI system partition (vfat, ~512MB)"))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(THEME.background));
        f.render_widget(p, chunks[1]);
        return;
    }

    // 2. Prevent state selection drift crashes
    if app.table_state.selected().is_none_or(|i| i >= efi_count) {
        app.table_state.select(Some(0));
    }

    // 3. Generate rows from the filtered dataset cleanly
    let rows = app.disks.iter().filter(|d| d.is_efi).map(|disk| {
        let fstype_str = disk.fstype.as_deref().unwrap_or("-");
        let contents_str = disk.contents.as_deref().unwrap_or("empty");

        Row::new(vec![
            // Removed manual "> " so the table engine can draw it contextually
            Cell::from(format!("  {}\n  contents: {}", disk.name, contents_str)),
            Cell::from(format!("{}\n", disk.size)),
            Cell::from(format!("{}\n", fstype_str)).style(Style::default().fg(THEME.green)),
        ])
        .height(2)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(70),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .block(Block::default().title("choose the EFI system partition (vfat, ~512MB)"))
    .row_highlight_style(Style::default().bg(THEME.selection).fg(THEME.orange))
    .highlight_symbol("> "); // Dynamically renders the indicator arrow only on the selected row

    f.render_stateful_widget(table, chunks[1], &mut app.table_state);
}
