// src/screens/mod.rs

use crate::app::{App, CurrentScreen};
use ratatui::Frame;

pub mod action_menu;
pub mod confirm;
pub mod exec_log;
pub mod log_export;
pub mod results;
pub mod select_efi;
pub mod select_root;
pub mod welcome;

pub fn render(f: &mut Frame, app: &mut App) {
    match app.current_screen {
        CurrentScreen::Welcome => welcome::render(f, app),
        CurrentScreen::SelectRoot => select_root::render(f, app),
        CurrentScreen::SelectEfi => select_efi::render(f, app),
        CurrentScreen::Confirm => confirm::render(f, app),
        CurrentScreen::ActionMenu => action_menu::render(f, app),
        CurrentScreen::DiagnoseLog => exec_log::render(f, app),
        CurrentScreen::ExecLog => exec_log::render(f, app),
        CurrentScreen::Result => results::render(f, app),
        CurrentScreen::LogExport => log_export::render(f, app),
    }
}
