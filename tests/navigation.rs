mod support;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pulse::{
    app::{App, ScopeFilter, ScopeStatus, StateFilter},
    model::Scope,
    process::RunError,
    systemd::{DiscoveryError, ScopeDiscovery},
};
use support::{report, services};

fn key(app: &mut App, code: KeyCode) {
    app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
}
fn control(app: &mut App, c: char) {
    app.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::CONTROL,
    )));
}
fn populated() -> App {
    let mut app = App::default();
    for scope in [Scope::System, Scope::User] {
        app.apply_discovery(report(scope, services(scope)));
    }
    app
}
fn search(app: &mut App, text: &str) {
    key(app, KeyCode::Char('/'));
    control(app, 'u');
    for c in text.chars() {
        key(app, KeyCode::Char(c));
    }
    key(app, KeyCode::Enter);
}

#[test]
fn navigation_clamps_and_uses_the_visible_page_size() {
    let mut app = populated();
    assert_eq!(app.visible_services().len(), 16);
    app.set_page_size(3);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.selected_index(&app.visible_services()), Some(0));
    key(&mut app, KeyCode::PageDown);
    assert_eq!(app.selected_index(&app.visible_services()), Some(3));
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_index(&app.visible_services()), Some(3));
    key(&mut app, KeyCode::PageUp);
    assert_eq!(app.selected_index(&app.visible_services()), Some(0));
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_index(&app.visible_services()), Some(15));
    key(&mut app, KeyCode::Home);
    assert_eq!(app.selected_index(&app.visible_services()), Some(0));
}

#[test]
fn search_matches_names_and_descriptions_and_handles_no_matches() {
    let mut app = populated();
    search(&mut app, "CAFÉ");
    assert_eq!(app.visible_services().len(), 2);
    assert!(app.selected_key().unwrap().name.starts_with("app-hello"));
    search(&mut app, "missing.service");
    assert_eq!(app.visible_services().len(), 2);
    search(&mut app, "no such service");
    assert!(app.visible_services().is_empty());
    assert!(app.selected_key().is_none());
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageDown,
    ] {
        key(&mut app, code);
    }
    assert!(app.selected_key().is_none());
    search(&mut app, "");
    assert_eq!(app.selected_index(&app.visible_services()), Some(0));
}

#[test]
fn editing_is_modal_cancellable_unicode_safe_and_bounded() {
    let mut app = populated();
    key(&mut app, KeyCode::End);
    let selected = app.selected_key().cloned();
    key(&mut app, KeyCode::Char('/'));
    for c in "q?é".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.query(), "q?é");
    assert!(!app.should_quit && !app.show_help);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.query(), "q?");
    key(&mut app, KeyCode::Esc);
    assert!(!app.is_searching() && !app.should_quit);
    assert_eq!(app.query(), "");
    assert_eq!(app.selected_key(), selected.as_ref());
    key(&mut app, KeyCode::Char('/'));
    for _ in 0..200 {
        key(&mut app, KeyCode::Char('é'));
    }
    assert_eq!(app.query().len(), 256);
    control(&mut app, 'u');
    assert!(app.query().is_empty());
    control(&mut app, 'c');
    assert!(app.should_quit);
}

#[test]
fn filters_compose_and_preserve_visible_identity() {
    let mut app = populated();
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.scope_filter, ScopeFilter::System);
    assert_eq!(app.visible_services().len(), 8);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.scope_filter, ScopeFilter::User);
    assert_eq!(app.selected_key().unwrap().scope, Scope::User);
    key(&mut app, KeyCode::Char('f'));
    assert_eq!(app.state_filter, StateFilter::Active);
    assert_eq!(app.visible_services().len(), 1);
    assert_eq!(app.selected_key().unwrap().name, "alpha.service");
    key(&mut app, KeyCode::Char('f'));
    assert_eq!(app.state_filter, StateFilter::Failed);
    assert_eq!(app.selected_key().unwrap().name, "bad.service");
    key(&mut app, KeyCode::Char('f'));
    assert_eq!(app.state_filter, StateFilter::Inactive);
    assert_eq!(app.visible_services().len(), 4);
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.scope_filter, ScopeFilter::All);
    assert_eq!(app.state_filter, StateFilter::All);
    assert_eq!(app.selected_key().unwrap().scope, Scope::User);
    search(&mut app, "alpha");
    assert_eq!(app.visible_services().len(), 2);
}

#[test]
fn selection_survives_arriving_scopes_and_reordering_by_identity() {
    let mut app = App::default();
    app.apply_discovery(report(Scope::User, services(Scope::User)));
    key(&mut app, KeyCode::End);
    let selected = app.selected_key().cloned();
    app.apply_discovery(report(Scope::System, services(Scope::System)));
    assert_eq!(app.selected_key(), selected.as_ref());
    let mut changed = services(Scope::User);
    changed.reverse();
    app.apply_discovery(report(Scope::User, changed));
    assert_eq!(app.selected_key(), selected.as_ref());
    app.apply_discovery(report(Scope::User, vec![]));
    assert_eq!(app.selected_key().unwrap().scope, Scope::System);
    app.apply_discovery(report(Scope::System, vec![]));
    assert!(app.selected_key().is_none());
}

#[test]
fn failures_and_worker_disconnect_preserve_completed_scopes() {
    let mut app = App::default();
    app.apply_discovery(report(Scope::System, services(Scope::System)));
    app.apply_discovery(ScopeDiscovery {
        scope: Scope::User,
        result: Err(DiscoveryError::Process(RunError::TimedOut)),
    });
    assert!(
        matches!(app.scopes[1].status, ScopeStatus::Failed(ref error) if error.contains("timed out"))
    );
    app.discovery_stopped();
    assert_eq!(app.visible_services().len(), 8);
    let mut pending = App::default();
    pending.apply_discovery(report(Scope::System, vec![]));
    pending.discovery_stopped();
    assert!(matches!(pending.scopes[0].status, ScopeStatus::Ready(_)));
    assert!(matches!(pending.scopes[1].status, ScopeStatus::Failed(_)));
}

#[test]
fn help_releases_repeats_resize_and_exit_work_in_their_modes() {
    let mut app = populated();
    let selected = app.selected_key().cloned();
    app.handle_event(Event::Resize(1, 1));
    let mut release = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    app.handle_event(Event::Key(release));
    assert!(!app.should_quit);
    assert_eq!(app.selected_key(), selected.as_ref());
    let mut repeat = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    repeat.kind = KeyEventKind::Repeat;
    app.handle_event(Event::Key(repeat));
    assert_eq!(app.selected_index(&app.visible_services()), Some(1));
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_index(&app.visible_services()), Some(1));
    key(&mut app, KeyCode::Esc);
    assert!(!app.show_help && !app.should_quit);
    key(&mut app, KeyCode::Esc);
    assert!(app.should_quit);
}
