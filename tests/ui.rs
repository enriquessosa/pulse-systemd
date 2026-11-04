mod support;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pulse::{
    app::{App, ScopeStatus},
    model::Scope,
    ui::View,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use support::{report, services};

fn text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}
fn draw(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    render(&mut View::default(), &mut terminal, app);
    text(&terminal)
}
fn render(view: &mut View, terminal: &mut Terminal<TestBackend>, app: &App) {
    terminal.draw(|frame| view.render(frame, app)).unwrap();
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}
fn populated() -> App {
    let mut app = App::default();
    for scope in [Scope::System, Scope::User] {
        app.apply_discovery(report(scope, services(scope)));
    }
    app
}

#[test]
fn renders_loading_help_and_tiny_terminals_without_panics() {
    let mut app = App::default();
    for (width, height) in [(0, 0), (1, 1), (59, 24), (80, 15), (80, 24), (120, 40)] {
        let output = draw(&app, width, height);
        if width >= 60 && height >= 16 {
            assert!(output.contains("system: Loading"));
            assert!(output.contains("user: Loading"));
            assert!(output.contains("Discovering services"));
        } else if width == 59 {
            assert!(output.contains("Terminal too small"));
        }
    }
    key(&mut app, KeyCode::Char('?'));
    let output = draw(&app, 100, 24);
    for expected in [
        "Keyboard help",
        "PageUp/PageDown",
        "Ctrl-U",
        "Tab:",
        "refresh current view",
    ] {
        assert!(output.contains(expected), "{expected}");
    }
}

#[test]
fn renders_states_scope_selection_and_scrolling() {
    let mut app = populated();
    let mut view = View::default();
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
    render(&mut view, &mut terminal, &app);
    let output = text(&terminal);
    for expected in [
        "system: 8 services",
        "user: 8 services",
        "Scope",
        "Load",
        "Active",
        "Substate",
        "Description",
        "not-found",
        "masked",
        "Selected: system / alpha.service",
        "Load: loaded | Active: active | Substate: running",
    ] {
        assert!(output.contains(expected), "{expected}");
    }
    assert!(view.page_size() > 1);
    app.set_page_size(view.page_size());
    key(&mut app, KeyCode::End);
    render(&mut view, &mut terminal, &app);
    assert!(text(&terminal).contains("Selected: user / z-worker.service"));
    let highlighted: Vec<String> = terminal
        .backend()
        .buffer()
        .content
        .chunks(120)
        .filter(|row| row.iter().any(|cell| cell.bg == Color::DarkGray))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect();
    assert_eq!(highlighted.len(), 1);
    assert!(highlighted[0].contains("user") && highlighted[0].contains("z-worker.service"));
    terminal
        .resize(ratatui::layout::Rect::new(0, 0, 80, 20))
        .unwrap();
    render(&mut view, &mut terminal, &app);
    assert!(text(&terminal).contains("Selected: user / z-worker.service"));
}

#[test]
fn distinguishes_empty_filtered_failed_and_partial_results() {
    let mut app = App::default();
    for scope in [Scope::System, Scope::User] {
        app.apply_discovery(report(scope, vec![]));
    }
    assert!(draw(&app, 100, 24).contains("No services reported"));
    app.apply_discovery(report(Scope::System, services(Scope::System)));
    key(&mut app, KeyCode::Char('/'));
    for c in "no-matches".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert!(draw(&app, 100, 24).contains("No services match"));
    key(&mut app, KeyCode::Esc);
    app.scopes[1].status = ScopeStatus::Failed("Permission denied".into());
    let output = draw(&app, 100, 24);
    assert!(output.contains("user ERROR: Permission denied"));
    assert!(output.contains("alpha.service"));
    app.scopes[0].status = ScopeStatus::Failed("systemctl missing".into());
    let output = draw(&app, 100, 24);
    assert!(output.contains("system ERROR: systemctl missing"));
    assert!(output.contains("No services available"));
    assert!(output.contains("Selected: none"));
}

#[test]
fn warnings_are_visible_and_failure_diagnostics_cannot_escape_the_terminal() {
    let mut app = populated();
    if let ScopeStatus::Ready(data) = &mut app.scopes[0].status {
        data.warning = "limited visibility".into();
    }
    app.scopes[1].status = ScopeStatus::Failed(format!("denied\x1b[2J\u{202e}{}", "x".repeat(200)));
    let output = draw(&app, 100, 24);
    assert!(output.contains("WARNING: limited visibility"));
    assert!(!output.contains('\x1b') && !output.contains('\u{202e}'));
    assert!(output.contains("[truncated]"));
}

#[test]
fn long_search_keeps_the_input_end_visible() {
    let mut app = populated();
    key(&mut app, KeyCode::Char('/'));
    for c in format!("{}end", "é".repeat(100)).chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert!(draw(&app, 60, 24).contains("end_"));
}

#[test]
fn inspection_renders_loading_properties_logs_errors_and_empty_results() {
    use pulse::{
        inspection::{InspectError, InspectionPart, InspectionReport, ReadState},
        parsing::{parse_journal, parse_properties},
    };
    let mut app = populated();
    key(&mut app, KeyCode::Enter);
    let selected = app.selected_key().cloned().unwrap();
    app.inspection.as_mut().unwrap().request_id = Some(1);
    assert!(draw(&app, 80, 24).contains("Loading properties"));
    app.apply_inspection(InspectionReport {
        id: 1,
        key: selected.clone(),
        part: InspectionPart::Properties(Ok(parse_properties(include_bytes!(
            "fixtures/properties.txt"
        ))
        .unwrap())),
    });
    let output = draw(&app, 80, 24);
    for expected in [
        "system / alpha.service",
        "[Properties]",
        "MainPID: 123",
        "FragmentPath: (empty)",
        "ActiveEnterTimestamp: (not reported)",
    ] {
        assert!(output.contains(expected), "{expected}");
    }
    key(&mut app, KeyCode::Tab);
    assert!(draw(&app, 80, 24).contains("Loading journal"));
    app.apply_inspection(InspectionReport {
        id: 1,
        key: selected,
        part: InspectionPart::Journal(Ok(
            parse_journal(include_bytes!("fixtures/journal.txt")).unwrap()
        )),
    });
    assert!(draw(&app, 120, 24).contains("Started café helper"));
    let inspection = app.inspection.as_mut().unwrap();
    inspection.journal =
        ReadState::Failed(InspectError::PermissionDenied("Permission denied".into()));
    assert!(draw(&app, 80, 24).contains("Permission denied"));
    app.inspection.as_mut().unwrap().journal =
        ReadState::Ready(parse_journal(b"-- No entries --\n").unwrap());
    assert!(draw(&app, 80, 24).contains("No recent journal entries returned"));
    key(&mut app, KeyCode::Tab);
    app.inspection.as_mut().unwrap().properties = ReadState::Ready(parse_properties(b"").unwrap());
    assert!(draw(&app, 80, 24).contains("No properties returned"));
    for (width, height) in [(0, 0), (1, 1), (59, 15), (60, 16)] {
        draw(&app, width, height);
    }
    key(&mut app, KeyCode::Char('?'));
    let output = draw(&app, 80, 24);
    assert!(output.contains("Enter: inspect"));
    assert!(output.contains("Left/Right pan"));
}

#[test]
fn refresh_displays_stale_data_age_and_error_until_successful_recovery() {
    use pulse::{
        inspection::{InspectError, InspectionPart, InspectionReport},
        parsing::parse_properties,
        process::RunError,
        systemd::{DiscoveryError, ScopeDiscovery},
    };
    let mut app = populated();
    app.refresh_discovery();
    let output = draw(&app, 100, 24);
    for expected in ["STALE", "Refreshing", "alpha.service", "Last success"] {
        assert!(output.contains(expected), "{expected}");
    }
    app.apply_discovery(ScopeDiscovery {
        scope: Scope::User,
        result: Err(DiscoveryError::Process(RunError::TimedOut)),
    });
    let output = draw(&app, 100, 24);
    assert!(output.contains("Last refresh ERROR"));
    assert!(output.contains("timed out"));
    assert!(output.contains("STALE") && output.contains("alpha.service"));
    app.apply_discovery(report(Scope::User, services(Scope::User)));
    app.apply_discovery(report(Scope::System, services(Scope::System)));
    assert!(!draw(&app, 100, 24).contains("STALE"));
    key(&mut app, KeyCode::Enter);
    let selected = app.selected_key().cloned().unwrap();
    app.inspection.as_mut().unwrap().request_id = Some(1);
    app.apply_inspection(InspectionReport {
        id: 1,
        key: selected.clone(),
        part: InspectionPart::Properties(Ok(parse_properties(b"MainPID=123").unwrap())),
    });
    app.inspection.as_mut().unwrap().refresh();
    let output = draw(&app, 80, 24);
    assert!(output.contains("STALE") && output.contains("MainPID: 123"));
    app.inspection.as_mut().unwrap().request_id = Some(2);
    app.apply_inspection(InspectionReport {
        id: 2,
        key: selected.clone(),
        part: InspectionPart::Properties(Err(InspectError::PermissionDenied(
            "Permission denied".into(),
        ))),
    });
    let output = draw(&app, 80, 24);
    assert!(
        output.contains("STALE")
            && output.contains("Permission denied")
            && output.contains("MainPID: 123")
    );
    app.inspection.as_mut().unwrap().refresh();
    app.inspection.as_mut().unwrap().request_id = Some(3);
    app.apply_inspection(InspectionReport {
        id: 3,
        key: selected,
        part: InspectionPart::Properties(Ok(parse_properties(b"MainPID=456").unwrap())),
    });
    let output = draw(&app, 80, 24);
    assert!(
        !output.contains("STALE")
            && !output.contains("Permission denied")
            && output.contains("MainPID: 456")
    );
}
