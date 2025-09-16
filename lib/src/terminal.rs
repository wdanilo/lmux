use crate::prelude::*;

// ============
// === Size ===
// ============

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

impl Size {
    pub fn current() -> Self {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        Self { cols: cols as usize, rows: rows as usize }
    }
}

// =========================
// === Capture / Cleanup ===
// =========================

pub fn capture() -> Result {
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    // Disable line wrap
    crossterm::execute!(stdout, crossterm::style::Print("\x1B[?7l"))?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    crossterm::execute!(stdout, crossterm::cursor::Hide)?;
    crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
    Ok(())
}

pub fn cleanup() -> Result {
    let mut stdout = std::io::stdout();
    // Enable line wrap
    crossterm::execute!(stdout, crossterm::style::Print("\x1B[?7h"))?;
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
    crossterm::execute!(stdout, crossterm::cursor::Show)?;
    crossterm::execute!(stdout, crossterm::event::DisableMouseCapture)?;
    Ok(())
}