use pulse::{
    inspection::{
        InspectError, Inspection, InspectionPart, InspectionReport, InspectionTab, ReadState,
    },
    journal::JournalReader,
    model::{DISPLAY_LINE_CHARS, JOURNAL_DISPLAY_LINES, Property, Scope, ServiceKey},
    parsing::{parse_journal, parse_properties},
    process::{CommandOutput, RunError, Runner},
    systemd::Systemd,
};
use std::{cell::RefCell, collections::VecDeque, io, sync::atomic::AtomicBool};

fn key(scope: Scope, name: &str) -> ServiceKey {
    ServiceKey {
        scope,
        name: name.into(),
    }
}
fn output(success: bool, stdout: &[u8], stderr: &[u8]) -> Result<CommandOutput, RunError> {
    Ok(CommandOutput {
        success,
        code: Some(if success { 0 } else { 1 }),
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    })
}
#[derive(Default)]
struct Mock {
    results: RefCell<VecDeque<Result<CommandOutput, RunError>>>,
    calls: RefCell<Vec<(String, Vec<String>)>>,
}
impl Mock {
    fn new(results: Vec<Result<CommandOutput, RunError>>) -> Self {
        Self {
            results: RefCell::new(results.into()),
            ..Self::default()
        }
    }
}
impl Runner for Mock {
    fn run(&self, program: &str, args: &[&str], _: &AtomicBool) -> Result<CommandOutput, RunError> {
        self.calls
            .borrow_mut()
            .push((program.into(), args.iter().map(|s| s.to_string()).collect()));
        self.results.borrow_mut().pop_front().unwrap()
    }
}

#[test]
fn properties_preserve_empty_missing_equals_and_numeric_values() {
    let data = parse_properties(include_bytes!("fixtures/properties.txt")).unwrap();
    assert_eq!(
        data.values[&Property::Description],
        "Alpha daemon with key=value"
    );
    assert_eq!(data.values[&Property::MainPID], "123");
    assert_eq!(data.values[&Property::FragmentPath], "");
    assert!(!data.values.contains_key(&Property::ActiveEnterTimestamp));
    assert!(parse_properties(b"").unwrap().values.is_empty());
    for malformed in [
        b"not a property".as_slice(),
        b"Id=x.service\nId=y.service",
        b"MainPID=bad",
        b"ExecMainStatus=no",
        b"Unknown=value",
        b"Description=\xff",
    ] {
        assert!(parse_properties(malformed).is_err());
    }
    let safe = parse_properties("Description=\u{202e}\x1b[2Jtext".as_bytes()).unwrap();
    assert!(!safe.values[&Property::Description].contains('\x1b'));
    assert!(!safe.values[&Property::Description].contains('\u{202e}'));
    assert!(
        parse_properties(format!("Description={}", "x".repeat(DISPLAY_LINE_CHARS + 1)).as_bytes())
            .unwrap()
            .values[&Property::Description]
            .ends_with("[truncated]")
    );
}

#[test]
fn journal_handles_empty_multiline_encoding_and_display_bounds() {
    for empty in [
        b"".as_slice(),
        b"-- No entries --\n",
        b"\r\n-- No entries --\r\n",
    ] {
        assert!(parse_journal(empty).unwrap().lines.is_empty());
    }
    let data = parse_journal(include_bytes!("fixtures/journal.txt")).unwrap();
    assert_eq!(data.lines.len(), 4);
    assert!(data.lines[2].starts_with("    continuation"));
    assert!(parse_journal(b"\xff").is_err());
    let data = parse_journal(
        format!(
            "\x1b[2J{}\n{}",
            "x".repeat(DISPLAY_LINE_CHARS + 1),
            "row\n".repeat(JOURNAL_DISPLAY_LINES)
        )
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(data.lines.len(), JOURNAL_DISPLAY_LINES);
    assert!(data.truncated);
    assert!(!data.lines[0].contains('\x1b'));
    assert!(data.lines[0].ends_with("[truncated]"));
}

#[test]
fn exact_scope_and_name_arguments_are_safe_for_both_adapters() {
    for scope in [Scope::System, Scope::User] {
        let key = key(scope, "-strange\\x2dname@session.service");
        let runner = Mock::new(vec![
            output(
                true,
                include_bytes!("fixtures/properties.txt"),
                b"partial properties warning",
            ),
            output(
                true,
                include_bytes!("fixtures/journal.txt"),
                b"limited journal visibility",
            ),
        ]);
        let properties = Systemd::new(&runner)
            .properties(&key, &AtomicBool::new(false))
            .unwrap();
        assert!(properties.warning.contains("warning"));
        let journal = JournalReader::new(&runner)
            .recent(&key, &AtomicBool::new(false))
            .unwrap();
        assert!(journal.warning.contains("limited journal"));
        let calls = runner.calls.borrow();
        assert_eq!(calls[0].0, "systemctl");
        assert_eq!(
            calls[0].1,
            [
                scope.flag(),
                "--no-pager",
                "--no-ask-password",
                "--all",
                "--property=Id,Description,LoadState,ActiveState,SubState,FragmentPath,UnitFileState,MainPID,ExecMainCode,ExecMainStatus,Result,ActiveEnterTimestamp",
                "show",
                "--",
                &key.name
            ]
        );
        assert_eq!(calls[1].0, "journalctl");
        let unit = format!(
            "{}={}",
            if scope == Scope::System {
                "--unit"
            } else {
                "--user-unit"
            },
            key.name
        );
        assert_eq!(
            calls[1].1,
            [
                scope.flag(),
                &unit,
                "--no-pager",
                "--lines=100",
                "--output=short-iso-precise",
                "--all",
                "--no-hostname"
            ]
        );
    }
}

#[test]
fn unsafe_names_are_rejected_before_spawning() {
    let runner = Mock::default();
    for name in [
        "x*.service",
        "x?.service",
        "../x.service",
        "x.service;touch /tmp/evil",
        "x.service\n",
        ".service",
        "other.socket",
    ] {
        let key = key(Scope::System, name);
        assert!(matches!(
            Systemd::new(&runner).properties(&key, &AtomicBool::new(false)),
            Err(InspectError::InvalidService)
        ));
        assert!(matches!(
            JournalReader::new(&runner).recent(&key, &AtomicBool::new(false)),
            Err(InspectError::InvalidService)
        ));
    }
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn denial_unavailability_parse_and_process_errors_are_distinct_from_empty() {
    let key = key(Scope::User, "alpha.service");
    let cases = [
        (
            output(false, b"", b"Permission denied"),
            "Permission denied",
        ),
        (
            output(
                true,
                b"-- No entries --\r\n",
                b"No journal files were opened due to insufficient permissions.",
            ),
            "Permission denied",
        ),
        (
            Err(RunError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "missing journalctl",
            ))),
            "Unavailable",
        ),
        (
            Err(RunError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "denied",
            ))),
            "Permission denied",
        ),
        (
            output(false, b"", b"Failed to connect to bus: No medium found"),
            "Error",
        ),
        (Err(RunError::TimedOut), "Error"),
        (Err(RunError::Cancelled), "Error"),
        (Err(RunError::OutputLimit), "Error"),
        (output(true, b"\xff", b""), "Error"),
    ];
    for (result, label) in cases {
        let error = JournalReader::new(Mock::new(vec![result]))
            .recent(&key, &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(error.label(), label);
    }
    let runner = Mock::new(vec![
        output(true, b"-- No entries --\n", b"Hint: limited visibility"),
        output(true, b"", b""),
    ]);
    let data = JournalReader::new(&runner)
        .recent(&key, &AtomicBool::new(false))
        .unwrap();
    assert!(data.lines.is_empty() && !data.warning.is_empty());
    assert!(
        Systemd::new(&runner)
            .properties(&key, &AtomicBool::new(false))
            .unwrap()
            .values
            .is_empty()
    );
}

#[test]
fn stale_scope_name_and_generation_results_are_ignored() {
    let selected = key(Scope::User, "alpha.service");
    let mut state = Inspection::new(selected.clone(), InspectionTab::Properties);
    state.request_id = Some(2);
    for (id, key) in [
        (1, selected.clone()),
        (2, key(Scope::System, "alpha.service")),
        (2, key(Scope::User, "other.service")),
    ] {
        state.apply(InspectionReport {
            id,
            key,
            part: InspectionPart::Properties(Ok(parse_properties(b"Id=wrong.service").unwrap())),
        });
        assert!(matches!(state.properties, ReadState::Loading));
    }
    state.apply(InspectionReport {
        id: 2,
        key: selected.clone(),
        part: InspectionPart::Properties(Ok(parse_properties(include_bytes!(
            "fixtures/properties.txt"
        ))
        .unwrap())),
    });
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line == "FragmentPath: (empty)")
    );
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line == "ActiveEnterTimestamp: (not reported)")
    );
    state.apply(InspectionReport {
        id: 2,
        key: selected,
        part: InspectionPart::Journal(Err(InspectError::PermissionDenied(
            "Permission denied".into(),
        ))),
    });
    state.switch_tab();
    assert!(state.lines()[0].contains("Permission denied"));
    state.switch_tab();
    assert!(state.lines().iter().any(|line| line == "MainPID: 123"));
}
