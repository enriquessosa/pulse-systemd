mod support;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pulse::{
    app::{App, ScopeStatus},
    freshness::Freshness,
    inspection::{
        InspectError, Inspection, InspectionPart, InspectionReport, InspectionTab, ReadState,
    },
    model::{Scope, ServiceKey},
    parsing::{parse_journal, parse_properties},
    process::RunError,
    systemd::{DiscoveryError, ScopeDiscovery},
    worker::DiscoveryReport,
};
use std::time::{Duration, Instant};
use support::{report, services};
fn key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}
fn populated() -> App {
    let mut app = App::default();
    for scope in [Scope::System, Scope::User] {
        app.apply_discovery(report(scope, services(scope)));
    }
    app.discovery_request_id = Some(1);
    app
}
fn failed(scope: Scope) -> ScopeDiscovery {
    ScopeDiscovery {
        scope,
        result: Err(DiscoveryError::Process(RunError::TimedOut)),
    }
}
fn refresh_result(app: &mut App, id: u64, report: ScopeDiscovery) {
    app.apply_discovery_refresh(DiscoveryReport {
        id,
        scope: report.scope,
        result: report.result,
    });
}

#[test]
fn discovery_preserves_filtered_selection_while_refreshing_failing_and_recovering() {
    let mut app = populated();
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab); // user only
    key(&mut app, KeyCode::Char('/'));
    for c in "service".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('f')); // active only
    let selected = app.selected_key().cloned();
    let filter = (app.scope_filter, app.state_filter);
    let timestamp = app.scopes[1].freshness.last_success;
    key(&mut app, KeyCode::Char('r'));
    assert!(app.discovery_refresh_requested && app.discovery_request_id.is_none());
    assert!(
        app.scopes
            .iter()
            .all(|scope| scope.freshness.is_stale() && scope.freshness.refreshing)
    );
    assert_eq!(app.selected_key(), selected.as_ref());
    assert_eq!(app.visible_services().len(), 1);
    app.discovery_request_id = Some(2);
    app.discovery_refresh_requested = false;
    refresh_result(&mut app, 2, failed(Scope::User));
    assert!(matches!(app.scopes[1].status, ScopeStatus::Ready(_)));
    assert!(app.scopes[1].freshness.is_stale());
    assert_eq!(app.scopes[1].freshness.last_success, timestamp);
    assert!(
        app.scopes[1]
            .freshness
            .last_error
            .as_ref()
            .unwrap()
            .contains("timed out")
    );
    assert_eq!(app.selected_key(), selected.as_ref());
    key(&mut app, KeyCode::Char('r'));
    app.discovery_request_id = Some(3);
    let mut changed = services(Scope::User);
    changed.reverse();
    refresh_result(&mut app, 3, report(Scope::User, changed));
    assert_eq!(app.selected_key(), selected.as_ref());
    assert_eq!((app.scope_filter, app.state_filter), filter);
    assert_eq!(app.query(), "service");
    assert!(!app.scopes[1].freshness.is_stale());
    assert!(app.scopes[1].freshness.last_error.is_none());
    assert!(app.scopes[0].freshness.refreshing); // scopes finish independently
}

#[test]
fn obsolete_discovery_success_and_error_cannot_replace_current_results() {
    let mut app = populated();
    let selected = app.selected_key().cloned();
    app.refresh_discovery(); // invalidate even before a worker ID is assigned
    refresh_result(&mut app, 1, report(Scope::System, vec![]));
    assert_eq!(app.total_services(), 16);
    app.discovery_request_id = Some(3);
    refresh_result(&mut app, 2, report(Scope::System, vec![]));
    refresh_result(&mut app, 2, failed(Scope::User));
    assert!(
        app.scopes
            .iter()
            .all(|scope| scope.freshness.refreshing && scope.freshness.last_error.is_none())
    );
    assert_eq!(app.selected_key(), selected.as_ref());
    refresh_result(&mut app, 3, report(Scope::System, vec![]));
    assert_eq!(app.selected_key().unwrap().scope, Scope::User);
    refresh_result(&mut app, 3, report(Scope::User, vec![]));
    assert!(app.selected_key().is_none());
    assert!(app.visible_services().is_empty());
    assert!(app.scopes.iter().all(|scope| !scope.freshness.is_stale()));
}

#[test]
fn empty_snapshot_failures_and_worker_shutdown_do_not_erase_known_results() {
    let mut app = App::default();
    app.apply_discovery(report(Scope::System, vec![]));
    app.apply_discovery(failed(Scope::User));
    app.refresh_discovery();
    app.discovery_stopped();
    assert!(matches!(&app.scopes[0].status, ScopeStatus::Ready(data) if data.services.is_empty()));
    assert!(app.scopes[0].freshness.is_stale());
    assert!(matches!(app.scopes[1].status, ScopeStatus::Failed(_)));
    app.refresh_discovery();
    app.discovery_request_id = Some(5);
    refresh_result(&mut app, 5, report(Scope::User, services(Scope::User)));
    assert_eq!(app.visible_services().len(), 8);
    assert!(app.scopes[1].freshness.last_error.is_none());
}

#[test]
fn inspection_retains_pane_offsets_and_each_last_success_on_partial_failure() {
    let selected = ServiceKey {
        scope: Scope::User,
        name: "alpha.service".into(),
    };
    let mut state = Inspection::new(selected.clone(), InspectionTab::Journal);
    state.request_id = Some(1);
    state.apply(InspectionReport {
        id: 1,
        key: selected.clone(),
        part: InspectionPart::Properties(Ok(parse_properties(include_bytes!(
            "fixtures/properties.txt"
        ))
        .unwrap())),
    });
    state.apply(InspectionReport {
        id: 1,
        key: selected.clone(),
        part: InspectionPart::Journal(Ok(
            parse_journal(include_bytes!("fixtures/journal.txt")).unwrap()
        )),
    });
    state.row = 2;
    state.column = 8;
    let old_time = state.journal_freshness.last_success;
    state.refresh();
    assert_eq!(
        (state.row, state.column, state.tab),
        (2, 8, InspectionTab::Journal)
    );
    assert!(state.journal_freshness.is_stale());
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line.contains("Started café"))
    );
    state.apply(InspectionReport {
        id: 1,
        key: selected.clone(),
        part: InspectionPart::Journal(Ok(parse_journal(b"").unwrap())),
    });
    assert!(matches!(&state.journal, ReadState::Ready(data) if !data.lines.is_empty()));
    state.request_id = Some(2);
    state.apply(InspectionReport {
        id: 2,
        key: selected.clone(),
        part: InspectionPart::Journal(Err(InspectError::PermissionDenied(
            "Permission denied".into(),
        ))),
    });
    assert_eq!(state.journal_freshness.last_success, old_time);
    assert!(state.journal_freshness.is_stale());
    assert!(
        state
            .lines()
            .iter()
            .any(|line| line.contains("Permission denied"))
    );
    assert!(matches!(&state.journal, ReadState::Ready(data) if data.lines.len() == 4));
    state.apply(InspectionReport {
        id: 2,
        key: selected.clone(),
        part: InspectionPart::Properties(Ok(parse_properties(b"MainPID=42").unwrap())),
    });
    assert!(!state.properties_freshness.is_stale());
    assert!(state.journal_freshness.is_stale());
    state.refresh();
    state.request_id = Some(3);
    state.apply(InspectionReport {
        id: 3,
        key: selected,
        part: InspectionPart::Journal(Ok(parse_journal(b"").unwrap())),
    });
    assert!(!state.journal_freshness.is_stale());
    assert!(state.journal_freshness.last_error.is_none());
    assert_eq!(state.row, 0);
    assert_eq!(state.column, 8); // render clamps horizontal offset
    assert!(state.lines()[0].contains("No recent journal"));
}

#[test]
fn refresh_is_contextual_and_search_text_does_not_submit_work() {
    let mut app = populated();
    key(&mut app, KeyCode::Char('/'));
    key(&mut app, KeyCode::Char('r'));
    assert_eq!(app.query(), "r");
    assert!(!app.discovery_refresh_requested);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('r'));
    assert!(!app.discovery_refresh_requested);
    assert!(app.inspection.as_ref().unwrap().request_id.is_none());
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Char('r'));
    assert!(!app.discovery_refresh_requested);
}

#[test]
fn freshness_age_is_monotonic_and_does_not_reset_on_error_or_retry() {
    let now = Instant::now();
    let last = now - Duration::from_secs(7);
    let mut state = Freshness {
        refreshing: false,
        last_success: Some(last),
        last_error: None,
    };
    assert_eq!(state.summary_at(now), "Last success 7s ago");
    state.failed("failure".into());
    state.begin();
    assert_eq!(state.last_success, Some(last));
    assert!(state.summary_at(now).contains("STALE"));
    assert!(state.summary_at(now).contains("Refreshing"));
    state.succeeded();
    assert!(!state.is_stale());
    assert!(state.last_error.is_none());
}
