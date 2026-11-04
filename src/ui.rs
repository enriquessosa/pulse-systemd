use crate::{
    app::{App, ScopeFilter, ScopeStatus},
    model::{ActiveState, display_text},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Row, Table, TableState, Wrap},
};

const HELP: &str = "Keyboard help\n\nUp/Down or k/j: move selection\nPageUp/PageDown: move one visible page\nHome/End: first/last service\n/: edit case-insensitive search (name or description)\nEnter: accept search; Esc: cancel edit\nBackspace: delete; Ctrl-U: clear search while editing\nTab: scope all / system / user\nf: active-state filter all / active / failed / inactive\n?: toggle help; Esc: close help\nq or Esc: quit while browsing; Ctrl-C: quit in any mode\n\nRead-only startup snapshot; refresh is not implemented yet.\nSystem and user discovery results are independent.\nNo service controls, properties, or journal view yet.";

#[derive(Default)]
pub struct View {
    table: TableState,
    page_size: usize,
}

fn diagnostic(text: &str) -> String {
    let mut chars = text.chars();
    let prefix: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{} [truncated]", display_text(&prefix))
    } else {
        display_text(&prefix)
    }
}

fn empty_message(app: &App) -> &'static str {
    let relevant: Vec<_> = app
        .scopes
        .iter()
        .filter(|state| match app.scope_filter {
            ScopeFilter::All => true,
            ScopeFilter::System => state.scope == crate::model::Scope::System,
            ScopeFilter::User => state.scope == crate::model::Scope::User,
        })
        .collect();
    if relevant
        .iter()
        .any(|scope| matches!(scope.status, ScopeStatus::Loading))
    {
        "Discovering services... You can navigate, search, or quit."
    } else if relevant
        .iter()
        .any(|scope| matches!(&scope.status, ScopeStatus::Ready(data) if !data.services.is_empty()))
    {
        "No services match the current search/filters."
    } else if relevant
        .iter()
        .any(|scope| matches!(scope.status, ScopeStatus::Failed(_)))
    {
        "No services available; a selected scope failed. See status above."
    } else {
        "No services reported by the selected managers."
    }
}

impl View {
    pub fn page_size(&self) -> usize {
        self.page_size.max(1)
    }

    pub fn render(&mut self, frame: &mut Frame, app: &App) {
        let area = frame.area();
        if area.width < 60 || area.height < 16 {
            self.page_size = 1;
            frame.render_widget(Paragraph::new("Pulse | read-only\nTerminal too small (minimum 60 x 16).\nResize to show services.\nCtrl-C: quit in any mode").wrap(Wrap { trim: false }), area);
            return;
        }
        if app.show_help {
            frame.render_widget(
                Paragraph::new(HELP)
                    .block(Block::bordered().title("Pulse | Help"))
                    .wrap(Wrap { trim: false }),
                area,
            );
            return;
        }
        let statuses: Vec<_> = app
            .scopes
            .iter()
            .map(|scope| {
                let (text, color, height) = match &scope.status {
                    ScopeStatus::Loading => {
                        (format!("{}: Loading...", scope.scope), Color::Yellow, 1)
                    }
                    ScopeStatus::Ready(data) if data.warning.is_empty() => (
                        format!("{}: {} services reported", scope.scope, data.services.len()),
                        Color::Green,
                        1,
                    ),
                    ScopeStatus::Ready(data) => (
                        format!(
                            "{}: {} services; WARNING: {}",
                            scope.scope,
                            data.services.len(),
                            diagnostic(&data.warning)
                        ),
                        Color::Yellow,
                        3,
                    ),
                    ScopeStatus::Failed(error) => (
                        format!("{} ERROR: {}", scope.scope, diagnostic(error)),
                        Color::Red,
                        3,
                    ),
                };
                (text, color, height)
            })
            .collect();
        let regions = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(statuses.iter().map(|s| s.2).sum()),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);
        frame.render_widget(
            Paragraph::new("Pulse | read-only | startup snapshot (no refresh)"),
            regions[0],
        );
        let scope_regions =
            Layout::vertical(statuses.iter().map(|s| Constraint::Length(s.2))).split(regions[1]);
        for ((text, color, _), region) in statuses.into_iter().zip(scope_regions.iter()) {
            frame.render_widget(
                Paragraph::new(text)
                    .style(Style::default().fg(color))
                    .wrap(Wrap { trim: false }),
                *region,
            );
        }
        let search = if app.is_searching() {
            "Search (editing; Enter accepts, Esc cancels)"
        } else {
            "Search (/ edits)"
        };
        let search = Line::from(format!(
            "{search}: {}{}",
            display_text(app.query()),
            if app.is_searching() { "_" } else { "" }
        ));
        let scroll = if app.is_searching() {
            search.width().saturating_sub(regions[2].width as usize) as u16
        } else {
            0
        };
        frame.render_widget(Paragraph::new(search).scroll((0, scroll)), regions[2]);
        let services = app.visible_services();
        let selected = app.selected_index(&services);
        self.table.select(selected);
        let title = format!(
            "Services: {} / {} reported | scope: {} | state: {}",
            services.len(),
            app.total_services(),
            app.scope_filter.label(),
            app.state_filter.label()
        );
        self.page_size = regions[3].height.saturating_sub(3) as usize;
        if services.is_empty() {
            frame.render_widget(
                Paragraph::new(empty_message(app))
                    .block(Block::bordered().title(title))
                    .wrap(Wrap { trim: false }),
                regions[3],
            );
        } else {
            let rows = services.iter().map(|service| {
                let style = match service.active {
                    ActiveState::Failed => Style::default().fg(Color::Red),
                    ActiveState::Active => Style::default().fg(Color::Green),
                    _ => Style::default(),
                };
                Row::new(vec![
                    service.scope.to_string(),
                    display_text(&service.name),
                    display_text(&service.load.to_string()),
                    display_text(&service.active.to_string()),
                    display_text(&service.substate.to_string()),
                    display_text(&service.description),
                ])
                .style(style)
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Percentage(26),
                    Constraint::Percentage(13),
                    Constraint::Percentage(14),
                    Constraint::Percentage(15),
                    Constraint::Min(10),
                ],
            )
            .header(
                Row::new([
                    "Scope",
                    "Service",
                    "Load",
                    "Active",
                    "Substate",
                    "Description",
                ])
                .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::bordered().title(title))
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
            frame.render_stateful_widget(table, regions[3], &mut self.table);
        }
        let details = selected
            .map(|index| {
                let service = services[index];
                vec![
                    Line::from(Span::styled(
                        format!(
                            "Selected: {} / {}",
                            service.scope,
                            display_text(&service.name)
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(
                        "Load: {} | Active: {} | Substate: {}",
                        display_text(&service.load.to_string()),
                        display_text(&service.active.to_string()),
                        display_text(&service.substate.to_string())
                    )),
                    Line::from(display_text(&service.description)),
                ]
            })
            .unwrap_or_else(|| vec![Line::from("Selected: none")]);
        frame.render_widget(Paragraph::new(details), regions[4]);
        frame.render_widget(
            Paragraph::new(
                "j/k move  PgUp/PgDn page  / search  Tab scope  f state  ? help  q quit",
            ),
            regions[5],
        );
    }
}
