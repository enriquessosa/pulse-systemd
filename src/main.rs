use crossterm::event;
use pulse::{app::App, terminal, ui};
use std::{io, time::Duration};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pulse: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    terminal::run(|terminal| {
        let mut app = App::default();
        while !app.should_quit {
            terminal.draw(|frame| ui::render(frame, &app))?;
            if event::poll(Duration::from_millis(100))? {
                app.handle_event(event::read()?);
            }
        }
        Ok(())
    })
}
