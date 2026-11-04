use pulse::{
    model::{ActiveState, LoadState, Scope, SubState, display_text},
    parsing::parse_services,
    process::{CommandOutput, RunError, Runner},
    systemd::{DiscoveryError, Systemd},
};
use std::{cell::RefCell, collections::VecDeque, io, sync::atomic::AtomicBool};

const SERVICES: &[u8] = include_bytes!("fixtures/services.txt");

#[test]
fn parses_states_scope_names_and_descriptions() {
    for scope in [Scope::System, Scope::User] {
        let services = parse_services(SERVICES, scope).unwrap();
        assert_eq!(services.len(), 8);
        assert!(services.windows(2).all(|pair| pair[0].name < pair[1].name));
        assert!(services.iter().all(|service| service.scope == scope));
        let by_name = |name: &str| services.iter().find(|s| s.name == name).unwrap();
        assert_eq!(by_name("alpha.service").active, ActiveState::Active);
        assert_eq!(by_name("missing.service").load, LoadState::NotFound);
        assert_eq!(by_name("masked.service").load, LoadState::Masked);
        assert_eq!(by_name("bad.service").active, ActiveState::Failed);
        assert_eq!(
            by_name("z-worker.service").description,
            "Worker with   multiple spaces"
        );
        assert_eq!(by_name("empty-description.service").description, "");
        assert_eq!(
            by_name("app-hello\\x2dworld@session.service").description,
            "Café background helper"
        );
        let unknown = by_name("unknown.service");
        assert_eq!(unknown.load, LoadState::Other("future-load".into()));
        assert_eq!(unknown.active, ActiveState::Other("future-active".into()));
        assert_eq!(unknown.substate, SubState::Other("future-sub".into()));
    }
}

#[test]
fn empty_output_is_success_and_whitespace_is_supported() {
    assert!(
        parse_services(include_bytes!("fixtures/empty.txt"), Scope::System)
            .unwrap()
            .is_empty()
    );
    let services = parse_services(
        b"\r\n\talpha.service\tloaded\tactive\trunning\tDesc\r\n\n",
        Scope::User,
    )
    .unwrap();
    assert_eq!(services[0].description, "Desc");
}

#[test]
fn rejects_malformed_output_without_partial_success() {
    let error =
        parse_services(include_bytes!("fixtures/malformed.txt"), Scope::System).unwrap_err();
    assert_eq!(error.line, 2);
    let encoding_error = parse_services(
        b"ok.service loaded active running Ok\nbad\xff.service loaded active running Bad",
        Scope::User,
    )
    .unwrap_err();
    assert_eq!(encoding_error.line, 2);
    for output in [
        b"thing.socket loaded active running Socket".as_slice(),
        b".service loaded active running Empty name",
        b"../evil.service loaded active running Invalid name",
        b"\xff.service loaded active running Bad encoding",
        b"x.service loaded active running \x1b[2J",
        b"\x0b",
        b"x.service loaded ACTIVE running Invalid state",
        b"x.service loaded active running One\nx.service loaded active running Duplicate",
        b"x.service loaded active running One\n2 loaded units listed.",
    ] {
        assert!(parse_services(output, Scope::System).is_err(), "{output:?}");
    }
}

#[test]
fn missing_utility_retains_the_io_error_kind() {
    let backend = Systemd::new(mock(vec![Err(RunError::Io(io::Error::new(
        io::ErrorKind::NotFound,
        "missing systemctl",
    )))]));
    assert!(
        matches!(backend.discover(Scope::System, &AtomicBool::new(false)),
        Err(DiscoveryError::Process(RunError::Io(error))) if error.kind() == io::ErrorKind::NotFound)
    );
}

#[test]
fn terminal_controls_and_bidi_are_escaped() {
    let text = display_text("error\x1b[2J\n\u{202e}bad");
    assert!(!text.contains('\x1b'));
    assert!(!text.contains('\n'));
    assert!(!text.contains('\u{202e}'));
    let services = parse_services(
        "x.service loaded active running \u{202e}hidden".as_bytes(),
        Scope::System,
    )
    .unwrap();
    assert_eq!(services[0].description, "\\u{202e}hidden");
}

struct MockRunner {
    outputs: RefCell<VecDeque<Result<CommandOutput, RunError>>>,
    calls: RefCell<Vec<Vec<String>>>,
}
impl Runner for MockRunner {
    fn run(&self, program: &str, args: &[&str], _: &AtomicBool) -> Result<CommandOutput, RunError> {
        assert_eq!(program, "systemctl");
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| s.to_string()).collect());
        self.outputs.borrow_mut().pop_front().unwrap()
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
fn mock(outputs: Vec<Result<CommandOutput, RunError>>) -> MockRunner {
    MockRunner {
        outputs: RefCell::new(outputs.into()),
        calls: RefCell::default(),
    }
}

#[test]
fn scopes_are_queried_read_only_and_fail_independently() {
    let runner = mock(vec![
        output(true, SERVICES, b"warning: limited visibility\n"),
        output(false, b"", include_bytes!("fixtures/unavailable.txt")),
    ]);
    let backend = Systemd::new(&runner);
    let [system, user] = backend.discover_all(&AtomicBool::new(false));
    assert_eq!(system.scope, Scope::System);
    assert_eq!(user.scope, Scope::User);
    let discovery = system.result.unwrap();
    assert_eq!(discovery.services.len(), 8);
    assert!(discovery.warning.contains("limited visibility"));
    assert!(matches!(
        user.result,
        Err(DiscoveryError::CommandFailed { code: Some(1), .. })
    ));
    for (call, scope) in runner.calls.borrow().iter().zip(["--system", "--user"]) {
        assert_eq!(
            call,
            &[
                scope,
                "--no-pager",
                "--no-legend",
                "--plain",
                "--full",
                "--all",
                "--type=service",
                "list-units"
            ]
        );
    }
}

#[test]
fn missing_denied_unavailable_and_malformed_are_never_empty_success() {
    let failures = [
        Err(RunError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "missing systemctl",
        ))),
        Err(RunError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "denied",
        ))),
        output(false, SERVICES, include_bytes!("fixtures/denied.txt")),
        output(false, b"", include_bytes!("fixtures/unavailable.txt")),
        output(true, include_bytes!("fixtures/malformed.txt"), b""),
        Err(RunError::TimedOut),
        Err(RunError::Cancelled),
        Err(RunError::OutputLimit),
    ];
    for failure in failures {
        assert!(
            Systemd::new(mock(vec![failure]))
                .discover(Scope::User, &AtomicBool::new(false))
                .is_err()
        );
    }
}

#[test]
fn adapter_keeps_empty_success_and_failure_diagnostics_distinct() {
    let runner = mock(vec![
        output(true, b"", b""),
        output(false, b"\x1b[2Jfailed\xff", b""),
    ]);
    let backend = Systemd::new(runner);
    assert!(
        backend
            .discover(Scope::System, &AtomicBool::new(false))
            .unwrap()
            .services
            .is_empty()
    );
    let error = backend
        .discover(Scope::User, &AtomicBool::new(false))
        .unwrap_err();
    assert!(matches!(error, DiscoveryError::CommandFailed { .. }));
    assert!(!error.to_string().contains('\x1b'));
    assert!(error.to_string().contains("failed"));
}
