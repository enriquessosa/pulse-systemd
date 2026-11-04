use pulse::{model::display_text, process::ProcessRunner, systemd::Systemd};
use std::{process::ExitCode, sync::atomic::AtomicBool};

fn main() -> ExitCode {
    let backend = Systemd::new(ProcessRunner::default());
    let mut failed = false;
    for report in backend.discover_all(&AtomicBool::new(false)) {
        match report.result {
            Ok(discovery) => {
                println!("{}: {} services", report.scope, discovery.services.len());
                if !discovery.warning.is_empty() {
                    eprintln!("{} warning: {}", report.scope, discovery.warning);
                }
                for service in discovery.services {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        service.scope,
                        display_text(&service.name),
                        service.load,
                        service.active,
                        service.substate,
                        service.description
                    );
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("{}: {error}", report.scope);
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
