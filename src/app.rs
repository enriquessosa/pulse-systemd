use crate::{
    inspection::{Inspection, InspectionReport, InspectionTab},
    model::{ActiveState, Scope, Service},
    systemd::{Discovery, ScopeDiscovery},
};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScopeFilter {
    #[default]
    All,
    System,
    User,
}
impl ScopeFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::System => "system",
            Self::User => "user",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::All => Self::System,
            Self::System => Self::User,
            Self::User => Self::All,
        }
    }
    fn matches(self, scope: Scope) -> bool {
        matches!(
            (self, scope),
            (Self::All, _) | (Self::System, Scope::System) | (Self::User, Scope::User)
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StateFilter {
    #[default]
    All,
    Active,
    Failed,
    Inactive,
}
impl StateFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Inactive => "inactive",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::All => Self::Active,
            Self::Active => Self::Failed,
            Self::Failed => Self::Inactive,
            Self::Inactive => Self::All,
        }
    }
    fn matches(self, state: &ActiveState) -> bool {
        match self {
            Self::All => true,
            Self::Active => *state == ActiveState::Active,
            Self::Failed => *state == ActiveState::Failed,
            Self::Inactive => *state == ActiveState::Inactive,
        }
    }
}

pub enum ScopeStatus {
    Loading,
    Ready(Discovery),
    Failed(String),
}

pub struct ScopeState {
    pub scope: Scope,
    pub status: ScopeStatus,
}

pub use crate::model::ServiceKey;

pub struct App {
    pub show_help: bool,
    pub should_quit: bool,
    pub scopes: [ScopeState; 2],
    pub scope_filter: ScopeFilter,
    pub state_filter: StateFilter,
    pub inspection: Option<Inspection>,
    query: String,
    search_snapshot: Option<(String, Option<ServiceKey>)>,
    selected: Option<ServiceKey>,
    page_size: usize,
}
impl Default for App {
    fn default() -> Self {
        Self {
            show_help: false,
            should_quit: false,
            scopes: [Scope::System, Scope::User].map(|scope| ScopeState {
                scope,
                status: ScopeStatus::Loading,
            }),
            scope_filter: ScopeFilter::All,
            state_filter: StateFilter::All,
            inspection: None,
            query: String::new(),
            search_snapshot: None,
            selected: None,
            page_size: 1,
        }
    }
}

impl App {
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn is_searching(&self) -> bool {
        self.search_snapshot.is_some()
    }
    pub fn selected_key(&self) -> Option<&ServiceKey> {
        self.selected.as_ref()
    }
    pub fn set_page_size(&mut self, rows: usize) {
        self.page_size = rows.clamp(1, isize::MAX as usize);
    }

    pub fn total_services(&self) -> usize {
        self.scopes
            .iter()
            .map(|scope| match &scope.status {
                ScopeStatus::Ready(data) => data.services.len(),
                _ => 0,
            })
            .sum()
    }

    pub fn visible_services(&self) -> Vec<&Service> {
        let query = self.query.to_lowercase();
        self.scopes
            .iter()
            .flat_map(|scope| match &scope.status {
                ScopeStatus::Ready(data) => data.services.as_slice(),
                _ => &[],
            })
            .filter(|service| {
                self.scope_filter.matches(service.scope)
                    && self.state_filter.matches(&service.active)
                    && (query.is_empty()
                        || service.name.to_lowercase().contains(&query)
                        || service.description.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn selected_index(&self, services: &[&Service]) -> Option<usize> {
        let key = self.selected.as_ref()?;
        services
            .iter()
            .position(|service| service.scope == key.scope && service.name == key.name)
    }

    fn reconcile_selection(&mut self) {
        let services = self.visible_services();
        if self.selected_index(&services).is_none() {
            self.selected = services.first().map(|service| ServiceKey::from(*service));
        }
        self.sync_inspection();
    }

    fn sync_inspection(&mut self) {
        if let Some(inspection) = &self.inspection {
            match &self.selected {
                Some(key) if *key != inspection.key => {
                    self.inspection = Some(Inspection::new(key.clone(), inspection.tab))
                }
                None => self.inspection = None,
                _ => {}
            }
        }
    }

    pub fn apply_inspection(&mut self, report: InspectionReport) {
        if let Some(inspection) = &mut self.inspection {
            inspection.apply(report);
        }
    }

    fn open_inspection(&mut self, tab: InspectionTab) {
        if let Some(key) = &self.selected {
            self.inspection = Some(Inspection::new(key.clone(), tab));
        }
    }

    pub fn apply_discovery(&mut self, report: ScopeDiscovery) {
        if let Some(scope) = self
            .scopes
            .iter_mut()
            .find(|scope| scope.scope == report.scope)
        {
            scope.status = match report.result {
                Ok(data) => ScopeStatus::Ready(data),
                Err(error) => ScopeStatus::Failed(error.to_string()),
            };
        }
        self.reconcile_selection();
    }

    pub fn discovery_stopped(&mut self) {
        for scope in &mut self.scopes {
            if matches!(scope.status, ScopeStatus::Loading) {
                scope.status = ScopeStatus::Failed(
                    "Discovery worker stopped before reporting this scope".into(),
                );
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let services = self.visible_services();
        if services.is_empty() {
            self.selected = None;
            return;
        }
        let index = self.selected_index(&services).unwrap_or(0);
        let next = index.saturating_add_signed(delta).min(services.len() - 1);
        self.selected = Some(ServiceKey::from(services[next]));
        self.sync_inspection();
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else {
            return;
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if self.is_searching() {
            match key.code {
                KeyCode::Enter => {
                    self.search_snapshot = None;
                }
                KeyCode::Esc => {
                    if let Some((query, selected)) = self.search_snapshot.take() {
                        self.query = query;
                        self.selected = selected;
                    }
                }
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.query.clear()
                }
                // Bound both search work and the rendered input line.
                KeyCode::Char(c)
                    if !c.is_control()
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && self.query.len() + c.len_utf8() <= 256 =>
                {
                    self.query.push(c);
                }
                _ => {}
            }
            self.reconcile_selection();
            return;
        }
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }
        if self.inspection.is_some() {
            match key.code {
                KeyCode::Esc => self.inspection = None,
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('?') => self.show_help = true,
                KeyCode::Char('[') => self.move_selection(-1),
                KeyCode::Char(']') => self.move_selection(1),
                code => {
                    if let Some(inspection) = &mut self.inspection {
                        match code {
                            KeyCode::Tab => inspection.switch_tab(),
                            KeyCode::Up | KeyCode::Char('k') => inspection.scroll(-1),
                            KeyCode::Down | KeyCode::Char('j') => inspection.scroll(1),
                            KeyCode::PageUp => inspection.scroll(-(inspection.page_size as isize)),
                            KeyCode::PageDown => inspection.scroll(inspection.page_size as isize),
                            KeyCode::Home => inspection.scroll(isize::MIN),
                            KeyCode::End => inspection.scroll(isize::MAX),
                            KeyCode::Left => {
                                inspection.column = inspection.column.saturating_sub(8)
                            }
                            KeyCode::Right => {
                                inspection.column = inspection
                                    .column
                                    .saturating_add(8)
                                    .min(inspection.column_limit)
                            }
                            _ => {}
                        }
                    }
                }
            }
            return;
        }
        match key.code {
            KeyCode::Enter => self.open_inspection(InspectionTab::Properties),
            KeyCode::Char('l') => self.open_inspection(InspectionTab::Journal),
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => {
                self.search_snapshot = Some((self.query.clone(), self.selected.clone()))
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(self.page_size as isize),
            KeyCode::PageUp => self.move_selection(-(self.page_size as isize)),
            KeyCode::Home => self.move_selection(isize::MIN),
            KeyCode::End => self.move_selection(isize::MAX),
            KeyCode::Tab => {
                self.scope_filter = self.scope_filter.next();
                self.reconcile_selection();
            }
            KeyCode::Char('f') => {
                self.state_filter = self.state_filter.next();
                self.reconcile_selection();
            }
            _ => {}
        }
    }
}
