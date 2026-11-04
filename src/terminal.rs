use crossterm::{
    cursor::Show,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, IsTerminal, Stdout},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub type Screen = Terminal<CrosstermBackend<Stdout>>;

struct Restore(Arc<AtomicBool>);

impl Restore {
    fn finish(&self) -> io::Result<()> {
        if self.0.swap(false, Ordering::Relaxed) {
            restore()
        } else {
            Ok(())
        }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = self.finish();
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
    enable_raw_mode()?;
    let guard = Restore(Arc::new(AtomicBool::new(true)));
    let active = Arc::clone(&guard.0);
    let owner = std::thread::current().id();
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A worker panic must not disable input in the still-running UI. The
        // hook remains installed after this session, but then becomes inactive.
        if std::thread::current().id() == owner && active.swap(false, Ordering::Relaxed) {
            let _ = restore();
        }
        previous_hook(info);
    }));
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = body(&mut terminal);
    result.and(guard.finish())
}
