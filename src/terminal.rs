use crossterm::{
    cursor::Show,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, IsTerminal, Stdout};

pub type Screen = Terminal<CrosstermBackend<Stdout>>;

struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = restore();
    }
}

fn restore() -> io::Result<()> {
    // Attempt both operations even if one fails, including during unwinding.
    let screen = execute!(io::stdout(), LeaveAlternateScreen, Show);
    let raw = disable_raw_mode();
    screen.and(raw)
}

pub fn run(body: impl FnOnce(&mut Screen) -> io::Result<()>) -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            "Pulse requires an interactive terminal on stdin and stdout",
        ));
    }
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        previous_hook(info);
    }));
    enable_raw_mode()?;
    let guard = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = body(&mut terminal);
    let cleanup = restore();
    drop(guard);
    result.and(cleanup)
}
