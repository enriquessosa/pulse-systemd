use crossterm::terminal::is_raw_mode_enabled;
use pulse::terminal;
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
};

// Run only in a real PTY via tests/terminal_pty.py; a captured Rust test process
// is not an interactive terminal. --skip carries the fixture mode, as in process.rs.
#[test]
#[ignore = "PTY lifecycle fixture invoked by tests/terminal_pty.py"]
fn terminal_lifecycle_fixture() {
    let mode = std::env::args().next_back().unwrap();
    match mode.as_str() {
        "success" | "after-session-panic" => {
            terminal::run(|_| Ok(())).unwrap();
            if mode == "after-session-panic" {
                assert!(catch_unwind(|| panic!("fixture panic after terminal session")).is_err());
            }
        }
        "io-error" => {
            let error =
                terminal::run(|_| Err(io::Error::other("fixture I/O failure"))).unwrap_err();
            assert_eq!(error.to_string(), "fixture I/O failure");
        }
        "ui-panic" => {
            assert!(
                catch_unwind(AssertUnwindSafe(|| terminal::run(|_| panic!(
                    "fixture UI panic"
                ))))
                .is_err()
            );
        }
        "worker-panic" => {
            terminal::run(|screen| {
                assert!(
                    std::thread::spawn(|| panic!("fixture worker panic"))
                        .join()
                        .is_err()
                );
                assert!(
                    is_raw_mode_enabled()?,
                    "worker panic disabled the UI's raw mode"
                );
                screen.clear()?;
                Ok(())
            })
            .unwrap();
        }
        _ => panic!("unknown terminal fixture mode: {mode}"),
    }
    assert!(
        !is_raw_mode_enabled().unwrap(),
        "raw mode survived terminal cleanup"
    );
}
