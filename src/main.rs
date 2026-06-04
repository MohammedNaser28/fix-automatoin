#![cfg_attr(feature = "alpine", allow(dead_code))]

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::Duration};

mod app;
#[cfg(feature = "alpine")]
mod init;
mod repair;
mod screens;
mod sys;
mod ui;

use crate::app::{ACTION_ITEMS, App, ConfirmFocus, CurrentScreen};

fn main() -> Result<(), io::Error> {
    #[cfg(feature = "alpine")]
    if let Err(e) = init::init_system() {
        eprintln!("init_system: {} — continuing anyway", e);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Application State — disks, firmware, network loaded in App::new()
    let mut app = App::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_loop(&mut app, &mut terminal)
    }));

    // Restore Terminal
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);

    match result {
        Ok(r) => r,
        Err(_) => {
            #[cfg(feature = "alpine")]
            init::panic_reboot();
            std::process::exit(1);
        }
    }
}

fn run_loop(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), io::Error> {
    loop {
        terminal.draw(|f| screens::render(f, app))?;

        app.check_scan();
        app.drain_log();

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('p') => {
                    do_poweroff();
                    app.should_quit = true;
                }
                KeyCode::Char('r') => {
                    do_reboot();
                    app.should_quit = true;
                }
                _ => handle_input(app, key.code),
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn do_poweroff() {
    #[cfg(feature = "alpine")]
    init::poweroff();
    #[cfg(not(feature = "alpine"))]
    {
        let _ = std::process::Command::new("poweroff").status();
    }
}

fn do_reboot() {
    #[cfg(feature = "alpine")]
    init::reboot();
    #[cfg(not(feature = "alpine"))]
    {
        let _ = std::process::Command::new("reboot").status();
    }
}

fn handle_input(app: &mut App, code: KeyCode) {
    match app.current_screen {
        CurrentScreen::Welcome => match code {
            KeyCode::Enter => {
                app.current_screen = CurrentScreen::SelectRoot;
                app.table_state.select(Some(0));
            }
            KeyCode::Char('q') => app.should_quit = true,
            _ => {}
        },

        CurrentScreen::SelectRoot => match code {
            KeyCode::Up => app.select_previous(),
            KeyCode::Down => app.select_next(),
            KeyCode::Enter => {
                if let Some(i) = app.table_state.selected()
                    && i < app.disks.len()
                {
                    app.selected_root = Some(app.disks[i].clone());
                    app.current_screen = CurrentScreen::SelectEfi;
                    app.table_state.select(Some(0));
                }
            }
            KeyCode::Char('q') => app.should_quit = true,
            _ => {}
        },

        CurrentScreen::SelectEfi => {
            let efi_disks: Vec<&crate::sys::blkdev::DiskInfo> =
                app.disks.iter().filter(|d| d.is_efi).collect();
            let efi_count = efi_disks.len();

            match code {
                KeyCode::Up if efi_count > 0 => {
                    let i = match app.table_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                efi_count - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    app.table_state.select(Some(i));
                }
                KeyCode::Down if efi_count > 0 => {
                    let i = match app.table_state.selected() {
                        Some(i) => {
                            if i >= efi_count - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    app.table_state.select(Some(i));
                }
                KeyCode::Enter => {
                    if let Some(i) = app.table_state.selected()
                        && i < efi_count
                    {
                        app.selected_efi = Some((*efi_disks[i]).clone());
                        app.table_state.select(Some(0));
                        app.current_screen = CurrentScreen::Confirm;
                    }
                }
                KeyCode::Esc => {
                    app.table_state.select(Some(0));
                    app.current_screen = CurrentScreen::SelectRoot;
                }
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }

        CurrentScreen::Confirm => match code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                app.toggle_confirm_buttons();
            }
            KeyCode::Enter => match app.confirm_focus {
                ConfirmFocus::Confirm => {
                    app.start_diagnosis();
                }
                ConfirmFocus::Back => {
                    app.table_state.select(Some(0));
                    app.current_screen = CurrentScreen::SelectEfi;
                }
            },
            KeyCode::Esc => {
                app.table_state.select(Some(0));
                app.current_screen = CurrentScreen::SelectEfi;
            }
            KeyCode::Char('q') => app.should_quit = true,
            _ => {}
        },

        CurrentScreen::ActionMenu => {
            match code {
                KeyCode::Up => app.action_prev(),
                KeyCode::Down => app.action_next(),
                KeyCode::Enter => {
                    if let Some(action) = ACTION_ITEMS[app.action_cursor]
                        && action.is_available()
                    {
                        app.selected_action = Some(action);
                        app.start_repair();
                    }
                    // Post-MVP items: do nothing (shown grayed in UI)
                }
                KeyCode::Esc => {
                    app.current_screen = CurrentScreen::Confirm;
                }
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }

        CurrentScreen::DiagnoseLog => match code {
            KeyCode::Enter if app.exec_done => {
                app.action_cursor = ACTION_ITEMS.iter().position(|i| i.is_some()).unwrap_or(0);
                app.current_screen = CurrentScreen::ActionMenu;
            }
            KeyCode::Char('q') => app.should_quit = true,
            _ => {}
        },

        CurrentScreen::ExecLog => {
            match code {
                // When done, Enter returns to Result screen
                KeyCode::Enter if app.exec_done => {
                    app.result_cursor = 0; // default to back to menu
                    app.current_screen = CurrentScreen::Result;
                }
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }

        CurrentScreen::Result => {
            match code {
                KeyCode::Left if app.result_cursor > 0 => {
                    app.result_cursor -= 1;
                }
                KeyCode::Right if app.result_cursor < 3 => {
                    app.result_cursor += 1;
                }
                KeyCode::Enter => {
                    match app.result_cursor {
                        0 => {
                            // back to menu
                            app.current_screen = CurrentScreen::ActionMenu;
                        }
                        1 => {
                            // reboot
                            do_reboot();
                            app.should_quit = true;
                        }
                        2 => {
                            // poweroff
                            do_poweroff();
                            app.should_quit = true;
                        }
                        3 => {
                            // export logs
                            app.current_screen = CurrentScreen::LogExport;
                        }
                        _ => {}
                    }
                }
                KeyCode::Char('q') => app.should_quit = true,
                _ => {}
            }
        }

        CurrentScreen::LogExport => match code {
            KeyCode::Enter | KeyCode::Esc => {
                app.current_screen = CurrentScreen::Result;
            }
            KeyCode::Char('q') => app.should_quit = true,
            _ => {}
        },
    }
}
