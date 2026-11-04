use pulse::{
    app::App,
    model::Scope,
    process::{CommandOutput, RunError, Runner},
    worker::DiscoveryWorker,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

struct ControlledRunner {
    started: mpsc::Sender<()>,
    release: Arc<AtomicBool>,
}
impl Runner for ControlledRunner {
    fn run(
        &self,
        _: &str,
        args: &[&str],
        cancelled: &AtomicBool,
    ) -> Result<CommandOutput, RunError> {
        self.started.send(()).unwrap();
        while !self.release.load(Ordering::Relaxed) {
            if cancelled.load(Ordering::Relaxed) {
                return Err(RunError::Cancelled);
            }
            thread::sleep(Duration::from_millis(2));
        }
        if args.contains(&"--user") {
            return Err(RunError::TimedOut);
        }
        Ok(CommandOutput {
            success: true,
            code: Some(0),
            stdout: b"x.service loaded active running Test service\n".to_vec(),
            stderr: vec![],
        })
    }
}

#[test]
fn pending_discovery_never_blocks_input_and_drop_cancels_inflight_work() {
    let (started, receiver) = mpsc::channel();
    let worker = DiscoveryWorker::spawn(ControlledRunner {
        started,
        release: Arc::new(AtomicBool::new(false)),
    })
    .unwrap();
    receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(worker.try_recv(), Err(TryRecvError::Empty)));
    let mut app = App::default();
    app.handle_event(crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));
    assert!(app.should_quit);
    let (done, receiver) = mpsc::channel();
    thread::spawn(move || {
        drop(worker);
        done.send(()).unwrap();
    });
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("worker must cancel and join promptly");
}

#[test]
fn completed_scope_arrives_separately_from_failed_scope() {
    let (started, _receiver) = mpsc::channel();
    let worker = DiscoveryWorker::spawn(ControlledRunner {
        started,
        release: Arc::new(AtomicBool::new(true)),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reports = vec![];
    while reports.len() < 2 {
        match worker.try_recv() {
            Ok(report) => reports.push(report),
            Err(TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(2))
            }
            _ => panic!("worker did not deliver both results"),
        }
    }
    assert_eq!(reports[0].scope, Scope::System);
    assert_eq!(reports[0].result.as_ref().unwrap().services.len(), 1);
    assert_eq!(reports[1].scope, Scope::User);
    assert!(reports[1].result.is_err());
}

#[test]
fn discovery_refresh_supersedes_inflight_work_and_can_retry_after_failure() {
    let (started, calls) = mpsc::channel();
    let release = Arc::new(AtomicBool::new(false));
    let mut worker = DiscoveryWorker::spawn(ControlledRunner {
        started,
        release: Arc::clone(&release),
    })
    .unwrap();
    calls.recv_timeout(Duration::from_secs(2)).unwrap();
    let old_id = worker.last_request_id();
    worker.request(()).unwrap();
    let latest_id = worker.request(()).unwrap();
    assert!(latest_id > old_id);
    release.store(true, Ordering::Relaxed);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reports = vec![];
    while reports.len() < 2 {
        assert!(Instant::now() < deadline);
        if let Ok(report) = worker.try_recv() {
            reports.push(report);
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }
    assert!(reports.iter().all(|report| report.id == latest_id));
    assert!(reports[0].result.is_ok());
    assert!(reports[1].result.is_err());
    let retry = worker.request(()).unwrap();
    let mut count = 0;
    while count < 2 {
        assert!(Instant::now() < deadline);
        if let Ok(report) = worker.try_recv() {
            assert_eq!(report.id, retry);
            count += 1;
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }
}
