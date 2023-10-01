use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, CrosstermBackend, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use ratatui_textarea::TextArea;
use std::time::Duration;

use crate::{emulator::Emulator, time_travel::TimeTravel};

pub fn main_loop(emulator: Emulator) -> Result<()> {
    let mut time_travel = TimeTravel::new(emulator);

    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut command_bar_shown = false;
    let mut command_bar = TextArea::default();
    command_bar.set_cursor_line_style(Style::default());

    loop {
        // Render the UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(10), Constraint::Length(26)])
                .split(f.size());

            {
                let vertical_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(chunks[0]);

                let disassembler = time_travel.current.memory.disassembler.as_ref().unwrap();
                let disassembly = disassembler.disassemble_pc_relative(
                    &time_travel.current.memory,
                    time_travel.current.pc,
                    20,
                );

                let pc_start = format!("{:16x}", time_travel.current.pc);

                let hl_line = disassembly
                    .lines()
                    .position(|line| line.starts_with(&pc_start))
                    .unwrap();

                let skip_amount = (hl_line as i32 - 8).max(0) as usize;
                let items: Vec<ListItem> = disassembly
                    .lines()
                    .enumerate()
                    .skip(skip_amount)
                    .take(vertical_split[0].height as usize)
                    .map(|(i, line)| {
                        let list_item = ListItem::new(Line::from(Span::raw(line.to_string())));
                        if i == hl_line {
                            list_item.style(
                                Style::default()
                                    .add_modifier(Modifier::REVERSED)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            list_item
                        }
                    })
                    .collect();

                let disassmebly_memory_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(30), Constraint::Length(32)])
                    .split(vertical_split[0]);

                f.render_widget(
                    List::new(items).block(
                        Block::default()
                            .title("Disassembly")
                            .borders(Borders::ALL)
                            .border_style(Style::default()),
                    ),
                    disassmebly_memory_split[0],
                );

                // create hexdump
                let dump = time_travel
                    .current
                    .memory
                    .hexdump(time_travel.current.last_mem_access, 30);

                f.render_widget(
                    Paragraph::new(dump).block(
                        Block::default()
                            .title("Memory")
                            .borders(Borders::ALL)
                            .border_style(Style::default()),
                    ),
                    disassmebly_memory_split[1],
                );

                let output_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(vertical_split[1]);

                let output = time_travel.current.stdout.clone();
                let lines = (output.chars().filter(|c| *c == '\n').count() as u16)
                    .max(output_split[0].height);

                f.render_widget(
                    Paragraph::new(time_travel.current.stdout.clone())
                        .scroll((lines - output_split[0].height, 0))
                        .block(
                            Block::default()
                                .title("stdout")
                                .borders(Borders::ALL)
                                .border_style(Style::default()),
                        ),
                    output_split[0],
                );

                f.render_widget(
                    Paragraph::new(format!("")).block(
                        Block::default()
                            .title("stderr")
                            .borders(Borders::ALL)
                            .border_style(Style::default()),
                    ),
                    output_split[1],
                );
            }

            f.render_widget(
                Paragraph::new(time_travel.current.print_registers()).block(
                    Block::default()
                        .title("Registers")
                        .borders(Borders::ALL)
                        .border_style(Style::default()),
                ),
                chunks[1],
            );

            // floating window if command bar shown
            if command_bar_shown {
                let floating = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(f.size());

                let widget = command_bar.widget();
                f.render_widget(widget, floating[1]);
            }
        })?;

        // Check for user input every 250 milliseconds
        if crossterm::event::poll(Duration::from_millis(250))? {
            if command_bar_shown {
                match crossterm::event::read()? {
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc, ..
                    }) => {
                        command_bar_shown = false;
                        command_bar = TextArea::default();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        command_bar_shown = false;
                        do_command(&mut time_travel, &command_bar.lines()[0]);
                        command_bar = TextArea::default();
                    }
                    input => {
                        command_bar.input(input);
                    }
                };
            } else if let Event::Key(key) = crossterm::event::read()? {
                match key.code {
                    KeyCode::Char('j') => {
                        time_travel.step(1);
                    }
                    KeyCode::Char('k') => {
                        time_travel.step(-1);
                    }
                    KeyCode::Char('q') => break,
                    KeyCode::Char(':') => {
                        command_bar_shown = true;
                        command_bar.input(key);
                    }
                    _ => {}
                };
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn do_command(time_travel: &mut TimeTravel, command: &str) {
    let tokens = command
        .strip_prefix(':')
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>();

    match tokens[0] {
        "step" => {
            if let Ok(step_amount) = tokens[1].parse() {
                time_travel.step(step_amount);
            }
        }

        _ => {}
    }
}
