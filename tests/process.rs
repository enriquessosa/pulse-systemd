use pulse::process::{ProcessRunner, RunError, Runner};
use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

// The current test executable is a portable child fixture; --skip carries a
// mode token without requiring a shell or another installed executable.
fn fixture(
    runner: &ProcessRunner,
    mode: &str,
    cancelled: &AtomicBool,
) -> Result<pulse::process::CommandOutput, RunError> {
    runner.run(
        std::env::current_exe().unwrap().to_str().unwrap(),
        &[
            "--exact",
            "subprocess_fixture",
            "--ignored",
            "--nocapture",
            "--skip",
            mode,
        ],
        cancelled,
    )
}

#[test]
#[ignore = "child fixture exercised by the process runner tests"]
fn subprocess_fixture() {
    match std::env::args().next_back().unwrap().as_str() {
        "mode_success" => {
            println!("locale={}", std::env::var("LC_ALL").unwrap());
            println!("colors={}", std::env::var("SYSTEMD_COLORS").unwrap());
            println!("urlify={}", std::env::var("SYSTEMD_URLIFY").unwrap());
            let mut stdin = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut stdin).unwrap();
            assert!(stdin.is_empty());
            eprintln!("fixture warning");
        }
        "mode_failure" => {
            eprintln!("fixture failure");
            std::process::exit(7);
        }
        "mode_sleep" => thread::sleep(Duration::from_secs(10)),
        "mode_flood" => {
            let bytes = [b'x'; 8192];
            loop {
                io::stdout().write_all(&bytes).unwrap();
                io::stderr().write_all(&bytes).unwrap();
            }
        }
        mode => panic!("unknown fixture mode: {mode}"),
    }
}

#[test]
fn drains_both_pipes_and_sets_deterministic_environment() {
    let output = fixture(
        &ProcessRunner::default(),
        "mode_success",
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(output.success);
    assert_eq!(output.code, Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in ["locale=C", "colors=0", "urlify=0"] {
        assert!(stdout.contains(expected));
    }
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("fixture warning")
    );
}

#[test]
fn preserves_nonzero_exit_and_spawn_error() {
    let runner = ProcessRunner::default();
    let output = fixture(&runner, "mode_failure", &AtomicBool::new(false)).unwrap();
    assert!(!output.success);
    assert_eq!(output.code, Some(7));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("fixture failure")
    );
    assert!(
        matches!(runner.run("/pulse/nonexistent/systemctl", &[], &AtomicBool::new(false)),
        Err(RunError::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
}

#[test]
fn bounds_output_and_elapsed_time() {
    let runner = ProcessRunner {
        timeout: Duration::from_secs(5),
        max_output_bytes: 2048,
    };
    assert!(matches!(
        fixture(&runner, "mode_flood", &AtomicBool::new(false)),
        Err(RunError::OutputLimit)
    ));
    let runner = ProcessRunner {
        timeout: Duration::from_millis(150),
        ..runner
    };
    let start = Instant::now();
    assert!(matches!(
        fixture(&runner, "mode_sleep", &AtomicBool::new(false)),
        Err(RunError::TimedOut)
    ));
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn cancellation_before_spawn_and_during_execution() {
    let runner = ProcessRunner::default();
    assert!(matches!(
        runner.run("/must/not/spawn", &[], &AtomicBool::new(true)),
        Err(RunError::Cancelled)
    ));
    let cancelled = AtomicBool::new(false);
    let start = Instant::now();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(30));
            cancelled.store(true, Ordering::Relaxed);
        });
        assert!(matches!(
            fixture(&runner, "mode_sleep", &cancelled),
            Err(RunError::Cancelled)
        ));
    });
    assert!(start.elapsed() < Duration::from_secs(3));
}
