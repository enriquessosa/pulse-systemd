use pulse::{
    inspection::{Inspection, InspectionPart, InspectionTab, ReadState},
    inspection_worker::InspectionWorker,
    model::{Scope, ServiceKey},
    process::{CommandOutput, RunError, Runner},
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct Controlled {
    started: mpsc::Sender<String>,
    cancelled: mpsc::Sender<()>,
}
impl Runner for Controlled {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        cancelled: &AtomicBool,
    ) -> Result<CommandOutput, RunError> {
        let name = if program == "systemctl" {
            *args.last().unwrap()
        } else {
            args.iter()
                .find_map(|arg| {
                    arg.strip_prefix("--unit=")
                        .or_else(|| arg.strip_prefix("--user-unit="))
                })
                .unwrap()
        };
        self.started.send(name.into()).unwrap();
        if name == "slow.service" {
            while !cancelled.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(2));
            }
            self.cancelled.send(()).unwrap();
            return Err(RunError::Cancelled);
        }
        let stdout = if program == "systemctl" {
            format!("Id={name}\nMainPID=9\n")
        } else {
            format!("2026-11-04T12:00:00.000001-04:00 helper[9]: {name}\n")
        };
        Ok(CommandOutput {
            success: true,
            code: Some(0),
            stdout: stdout.into_bytes(),
            stderr: vec![],
        })
    }
}
fn key(name: &str) -> ServiceKey {
    ServiceKey {
        scope: Scope::User,
        name: name.into(),
    }
}

#[test]
fn switching_selection_cancels_old_work_and_delivers_only_matching_results() {
    let (started, calls) = mpsc::channel();
    let (cancelled, cancellations) = mpsc::channel();
    let mut worker = InspectionWorker::spawn(Controlled { started, cancelled }).unwrap();
    worker.request(key("slow.service")).unwrap();
    assert_eq!(
        calls.recv_timeout(Duration::from_secs(2)).unwrap(),
        "slow.service"
    );
    let start = Instant::now();
    worker.request(key("middle.service")).unwrap();
    let id = worker.request(key("latest.service")).unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    cancellations.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut state = Inspection::new(key("latest.service"), InspectionTab::Journal);
    state.request_id = Some(id);
    let deadline = Instant::now() + Duration::from_secs(2);
    while matches!(state.properties, ReadState::Loading)
        || matches!(state.journal, ReadState::Loading)
    {
        assert!(
            Instant::now() < deadline,
            "latest selection did not complete"
        );
        if let Ok(report) = worker.try_recv() {
            state.apply(report);
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line.contains("latest.service"))
    );
    state.switch_tab();
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line == "Id: latest.service")
    );
}

#[test]
fn full_result_queue_and_idle_wait_are_cancellable_on_drop() {
    for active in [false, true] {
        let (started, calls) = mpsc::channel();
        let (cancelled, _cancellations) = mpsc::channel();
        let mut worker = InspectionWorker::spawn(Controlled { started, cancelled }).unwrap();
        if active {
            worker.request(key("first.service")).unwrap();
            calls.recv_timeout(Duration::from_secs(2)).unwrap();
            calls.recv_timeout(Duration::from_secs(2)).unwrap();
            worker.request(key("second.service")).unwrap();
            calls.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        let (done, finished) = mpsc::channel();
        thread::spawn(move || {
            drop(worker);
            done.send(()).unwrap();
        });
        finished
            .recv_timeout(Duration::from_secs(2))
            .expect("drop must not block on a full queue or idle wait");
    }
}

#[test]
fn failed_properties_do_not_prevent_journal_delivery() {
    struct Partial;
    impl Runner for Partial {
        fn run(
            &self,
            program: &str,
            _: &[&str],
            _: &AtomicBool,
        ) -> Result<CommandOutput, RunError> {
            if program == "systemctl" {
                return Err(RunError::TimedOut);
            }
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
    }
    let mut worker = InspectionWorker::spawn(Partial).unwrap();
    let id = worker.request(key("x.service")).unwrap();
    let mut count = 0;
    let deadline = Instant::now() + Duration::from_secs(2);
    while count < 2 {
        assert!(Instant::now() < deadline);
        if let Ok(report) = worker.try_recv() {
            assert_eq!(report.id, id);
            match report.part {
                InspectionPart::Properties(result) => assert!(result.is_err()),
                InspectionPart::Journal(result) => assert!(result.unwrap().lines.is_empty()),
            }
            count += 1;
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }
}
