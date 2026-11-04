use crossterm::event;
use pulse::{
    app::App, inspection_worker::InspectionWorker, model::INSPECTION_OUTPUT_BYTES,
    process::ProcessRunner, terminal, ui, worker::DiscoveryWorker,
};
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
    let mut inspector = None;
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
            if let Some(inspection) = &mut app.inspection {
                if inspector.is_none() {
                    inspector = Some(InspectionWorker::spawn(ProcessRunner {
                        max_output_bytes: INSPECTION_OUTPUT_BYTES,
                        ..ProcessRunner::default()
                    })?);
                }
                if let Some(inspector) = &mut inspector {
                    if inspection.request_id.is_none() {
                        inspection.request_id = Some(inspector.request(inspection.key.clone())?);
                    }
                    loop {
                        match inspector.try_recv() {
                            Ok(report) => inspection.apply(report),
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                inspection.worker_stopped();
                                break;
                            }
                        }
                    }
                }
            } else if let Some(inspector) = &inspector {
                inspector.cancel_current();
            }
            terminal.draw(|frame| view.render(frame, &app))?;
            app.set_page_size(view.page_size());
            if let Some(inspection) = &mut app.inspection {
                inspection.page_size = view.inspection_page_size();
                inspection.column_limit = view.inspection_column_limit();
                inspection.column = inspection.column.min(inspection.column_limit);
            }
            if event::poll(Duration::from_millis(50))? {
                app.handle_event(event::read()?);
            }
        }
        worker.cancel();
        if let Some(inspector) = &inspector {
            inspector.cancel_current();
        }
        Ok(())
    });
    drop(worker);
    drop(inspector);
    result
}
