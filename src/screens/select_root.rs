use crate::app::App;
use crate::ui::{theme::THEME, widgets};
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = widgets::draw_layout(f, "SELECT ROOT PARTITION");

    // Subtitle
    let subtitle =
        ratatui::widgets::Paragraph::new("  choose the linux root ( / ) partition to repair")
            .style(Style::default().fg(THEME.comment));
    let [subtitle_area, table_area] = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(chunks[1])[..]
    else {
        return;
    };
    f.render_widget(subtitle, subtitle_area);

    // Column headers
    let header = Row::new(
        ["device", "size", "fstype", "label"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(THEME.cyan))),
    )
    .height(1)
    .bottom_margin(1);

    // Rows
    let rows = app.disks.iter().map(|disk| {
        let fstype_str = disk.fstype.as_deref().unwrap_or("-");
        let fstype_color = match fstype_str {
            "ext4" | "btrfs" | "xfs" => THEME.green,
            "ntfs" => THEME.red,
            "swap" => THEME.purple,
            "vfat" => THEME.cyan,
            _ => THEME.foreground,
        };
        let label_str = disk.label.clone().unwrap_or_else(|| "-".into());

        Row::new(vec![
            Cell::from(format!("  {}", disk.name)).style(Style::default().fg(THEME.foreground)),
            Cell::from(disk.size.clone()),
            Cell::from(fstype_str).style(Style::default().fg(fstype_color)),
            Cell::from(label_str).style(Style::default().fg(THEME.comment)),
        ])
        .height(2)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(40),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::NONE))
    .row_highlight_style(
        Style::default()
            .bg(THEME.selection)
            .fg(THEME.cyan)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ");

    f.render_stateful_widget(table, table_area, &mut app.table_state);
}
