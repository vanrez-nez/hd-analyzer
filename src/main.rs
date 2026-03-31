mod app;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, Screen};

fn main() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal);
    restore_terminal(&mut terminal)?;

    if let Err(error) = result {
        eprintln!("{error:?}");
        return Err(error);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let mut app = App::new()?;

    while !app.should_quit {
        app.poll_scanner();

        terminal.draw(|frame| ui::draw(frame, &app))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(&mut app, key.code)?;
                }
            }
        }
    }

    Ok(())
}

fn handle_key_event(app: &mut App, code: KeyCode) -> Result<()> {
    match app.screen {
        Screen::SelectDrive => match code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => app.next_drive(),
            KeyCode::Up | KeyCode::Char('k') => app.previous_drive(),
            KeyCode::Enter => app.start_scan(),
            KeyCode::Esc => app.error_message = None,
            _ => {}
        },

        Screen::Results => match code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => app.next_result_entry(),
            KeyCode::Up | KeyCode::Char('k') => app.previous_result_entry(),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.enter_selected_directory(),
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => app.go_to_parent_directory(),
            KeyCode::Char('r') => app.rescan(),
            KeyCode::Char('s') => app.back_to_selection()?,
            KeyCode::Esc => app.error_message = None,
            _ => {}
        },

        Screen::ErrorLog => match code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                app.screen = Screen::Results;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(res) = &app.result {
                    if !res.read_errors.is_empty() {
                        app.error_scroll = (app.error_scroll + 1).min(res.read_errors.len() - 1);
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.error_scroll = app.error_scroll.saturating_sub(1);
            }
            _ => {}
        },
    }

    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
