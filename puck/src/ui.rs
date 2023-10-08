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
use std::{io::Stdout, time::Duration};

use remu::{emulator::Emulator, time_travel::TimeTravel};

pub struct App {
    time_travel: TimeTravel,
    breakpoint: Breakpoint,
    enable_auto: bool,
    auto_delay: u64,
    running: bool,
    command_bar: TextArea<'static>,
    command_bar_shown: bool,
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

enum Breakpoint {
    None,
    Syscall,
    Symbol(String),
    Address(u64),
}

impl App {
    pub fn new(emulator: Emulator) -> Result<App> {
        let mut stdout = std::io::stdout();
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

        let mut command_bar = TextArea::default();
        command_bar.set_cursor_line_style(Style::default());

        Ok(App {
            time_travel: TimeTravel::new(emulator),
            breakpoint: Breakpoint::None,
            enable_auto: false,
            auto_delay: 16,
            running: true,
            terminal: Terminal::new(CrosstermBackend::new(stdout))?,
            command_bar,
            command_bar_shown: false,
        })
    }

    fn render_ui(&mut self) -> Result<()> {
        let disassembler = &self.time_travel.current.memory.disassembler;

        let disassembly = disassembler.disassemble_pc_relative(
            &self.time_travel.current.memory,
            self.time_travel.current.pc,
            30,
        );

        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(10), Constraint::Length(28)])
                .split(f.size());

            {
                let vertical_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(chunks[0]);

                let pc_start = format!("{:16x}", self.time_travel.current.pc);

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
                    .constraints([Constraint::Min(30), Constraint::Length(34)])
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
                let dump = self
                    .time_travel
                    .current
                    .memory
                    .hexdump(self.time_travel.current.last_mem_access, 30);

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

                let output = self.time_travel.current.stdout.clone();
                let lines = (output.chars().filter(|c| *c == '\n').count() as u16)
                    .max(output_split[0].height);

                f.render_widget(
                    Paragraph::new(self.time_travel.current.stdout.clone())
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
                    Paragraph::new(self.time_travel.current.stderr.clone()).block(
                        Block::default()
                            .title("stderr")
                            .borders(Borders::ALL)
                            .border_style(Style::default()),
                    ),
                    output_split[1],
                );
            }

            f.render_widget(
                Paragraph::new(self.time_travel.current.print_registers()).block(
                    Block::default()
                        .title("Registers")
                        .borders(Borders::ALL)
                        .border_style(Style::default()),
                ),
                chunks[1],
            );

            // floating window if command bar shown
            if self.command_bar_shown {
                let floating = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(f.size());

                let widget = self.command_bar.widget();
                f.render_widget(widget, floating[1]);
            }
        })?;

        Ok(())
    }

    pub fn check_input(&mut self) -> Result<()> {
        let input = if self.enable_auto {
            crossterm::event::poll(Duration::from_millis(self.auto_delay))?
        } else {
            crossterm::event::poll(Duration::from_millis(250))?
        };

        if !input && self.enable_auto {
            self.time_travel.step(1);
        }

        if input {
            if self.command_bar_shown {
                match crossterm::event::read()? {
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc, ..
                    }) => {
                        self.command_bar_shown = false;
                        self.command_bar = TextArea::default();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        self.command_bar_shown = false;
                        self.do_command();
                        self.command_bar = TextArea::default();
                    }
                    input => {
                        self.command_bar.input(input);
                    }
                };
            } else if let Event::Key(key) = crossterm::event::read()? {
                match key.code {
                    KeyCode::Char('j') => {
                        self.time_travel.step(1);
                    }
                    KeyCode::Char('k') => {
                        self.time_travel.step(-1);
                    }
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Char(':') => {
                        self.command_bar_shown = true;
                        self.command_bar.input(key);
                    }
                    _ => {}
                };
            }
        }

        Ok(())
    }

    pub fn main_loop(&mut self) -> Result<()> {
        while self.running {
            self.render_ui()?;
            self.check_input()?;
        }

        Ok(())
    }

    fn do_command(&mut self) {
        let command = self.command_bar.lines()[0].as_str();

        let tokens = command
            .strip_prefix(':')
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();

        match tokens[0] {
            "s" | "step" => {
                let step_amount = tokens.get(1).map(|s| s.parse().unwrap_or(1)).unwrap_or(1);
                self.time_travel.step(step_amount);
            }

            "sa" | "stopauto" => {
                self.enable_auto = false;
            }

            "a" | "auto" => {
                self.enable_auto = true;
                let auto_delay = tokens.get(1).map(|s| s.parse().unwrap_or(16)).unwrap_or(16);
                self.auto_delay = auto_delay;
            }

            // advance to next breakpoint, or end of program
            "n" | "next" => match self.breakpoint {
                Breakpoint::None => while self.time_travel.step(1).is_none() {},
                Breakpoint::Syscall => todo!(),
                Breakpoint::Symbol(ref search_symbol) => {
                    while self.time_travel.step(1).is_none() {
                        if let Some(symbol_at_addr) = self
                            .time_travel
                            .current
                            .memory
                            .disassembler
                            .get_symbol_at_addr(self.time_travel.current.pc)
                        {
                            if &symbol_at_addr == search_symbol {
                                break;
                            }
                        }
                    }
                }
                Breakpoint::Address(a) => {
                    while self.time_travel.step(1).is_none() {
                        if self.time_travel.current.pc == a {
                            break;
                        }
                    }
                }
            },

            // set breakpoint
            "bp" => match tokens.get(1) {
                Some(&"syscall") => {
                    self.breakpoint = Breakpoint::Syscall;
                }
                Some(&symbol_name) => match u64::from_str_radix(symbol_name, 16) {
                    Ok(a) => {
                        self.breakpoint = Breakpoint::Address(a);
                    }
                    Err(_) => {
                        self.breakpoint = Breakpoint::Symbol(symbol_name.to_string());
                    }
                },
                None => {
                    self.breakpoint = Breakpoint::None;
                }
            },

            _ => {}
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().unwrap();
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
        )
        .unwrap();
        self.terminal.show_cursor().unwrap()
    }
}
