//! Terminal pager for scrollable text output.
//!
//! This module provides functionality for displaying large text content
//! in a scrollable pager with 'q' to exit.

use std::io::{stdout, Write};
use termion::{
    event::Key,
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
    screen::{AlternateScreen, IntoAlternateScreen},
};

type PagerOutput = AlternateScreen<RawTerminal<std::io::Stdout>>;

/// A simple terminal pager for scrollable text
pub struct Pager {
    stdout: PagerOutput,
    lines: Vec<String>,
    current_line: usize,
    terminal_height: u16,
}

impl Pager {
    /// Create a new pager with the given text content
    pub fn new(content: &str) -> std::io::Result<Self> {
        let raw = stdout().into_raw_mode()?;
        let stdout = raw.into_alternate_screen()?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let terminal_height = termion::terminal_size()?.1;

        let visible_height = terminal_height.saturating_sub(1) as usize;
        let max_scroll = lines.len().saturating_sub(visible_height);

        Ok(Self {
            stdout,
            lines,
            current_line: max_scroll, // Start at the bottom
            terminal_height,
        })
    }

    /// Display the content in a scrollable view
    pub fn display(&mut self) -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut keys = stdin.keys();

        self.draw()?;

        while let Some(Ok(key)) = keys.next() {
            match key {
                Key::Char('q') | Key::Ctrl('c') => break,
                Key::Char('j') | Key::Down => self.scroll_down()?,
                Key::Char('k') | Key::Up => self.scroll_up()?,
                Key::PageDown | Key::Char(' ') => self.page_down()?,
                Key::PageUp => self.page_up()?,
                _ => (),
            }
            self.draw()?;
        }

        Ok(())
    }

    fn draw(&mut self) -> std::io::Result<()> {
        write!(self.stdout, "{}", termion::clear::All)?;
        write!(self.stdout, "{}", termion::cursor::Goto(1, 1))?;

        let visible_height = self.terminal_height.saturating_sub(1) as usize;
        let visible_lines = self
            .lines
            .iter()
            .skip(self.current_line)
            .take(visible_height);

        for line in visible_lines {
            writeln!(self.stdout, "{}\r", line)?;
        }

        // Draw scroll indicator
        let total_lines = self.lines.len();
        if total_lines > visible_height {
            let progress = (self.current_line as f64 / (total_lines - visible_height) as f64
                * 100.0)
                .round() as usize;
            write!(
                self.stdout,
                "{}--{}%--",
                termion::cursor::Goto(1, self.terminal_height),
                progress
            )?;
        }

        self.stdout.flush()
    }

    fn scroll_up(&mut self) -> std::io::Result<()> {
        if self.current_line > 0 {
            self.current_line -= 1;
        }
        Ok(())
    }

    fn scroll_down(&mut self) -> std::io::Result<()> {
        let max_scroll = self.max_scroll();
        if self.current_line < max_scroll {
            self.current_line += 1;
        }
        Ok(())
    }

    fn page_up(&mut self) -> std::io::Result<()> {
        let page_size = self.terminal_height.saturating_sub(1) as usize;
        self.current_line = self.current_line.saturating_sub(page_size);
        Ok(())
    }

    fn page_down(&mut self) -> std::io::Result<()> {
        let page_size = self.terminal_height.saturating_sub(1) as usize;
        let max_scroll = self.max_scroll();
        self.current_line = (self.current_line + page_size).min(max_scroll);
        Ok(())
    }

    fn max_scroll(&self) -> usize {
        let visible_height = self.terminal_height.saturating_sub(1) as usize;
        self.lines.len().saturating_sub(visible_height)
    }
}
