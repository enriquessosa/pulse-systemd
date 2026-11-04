use crossterm::event;
use pulse::{app::App, process::ProcessRunner, terminal, ui, worker::DiscoveryWorker};
use std::{io, sync::mpsc::TryRecvError, time::Duration};

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
    // Keep the worker outside the terminal callback: restoration runs before
    // joining subprocess work, including on an I/O failure or panic.
    let mut worker = None;
    let result = terminal::run(|terminal| {
        let worker = worker.insert(DiscoveryWorker::spawn(ProcessRunner::default())?);
        let mut app = App::default();
        let mut view = ui::View::default();
        while !app.should_quit {
            loop {
                match worker.try_recv() {
                    Ok(report) => app.apply_discovery(report),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        app.discovery_stopped();
                        break;
                    }
                }
            }
            terminal.draw(|frame| view.render(frame, &app))?;
            app.set_page_size(view.page_size());
            if event::poll(Duration::from_millis(50))? {
                app.handle_event(event::read()?);
            }
        }
        worker.cancel();
        Ok(())
    });
    drop(worker);
    result
}
