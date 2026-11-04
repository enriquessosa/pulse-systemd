use crate::{
    freshness::Freshness,
    model::{Journal, Properties, Property, ServiceKey, display_text, valid_service_name},
    parsing::ParseError,
    process::{CommandOutput, RunError, Runner},
};
use std::{fmt, io, sync::atomic::AtomicBool};

#[derive(Debug)]
pub enum InspectError {
    InvalidService,
    WorkerStopped,
    Process(RunError),
    Command {
        program: &'static str,
        code: Option<i32>,
        diagnostic: String,
    },
    PermissionDenied(String),
    Parse(ParseError),
}
impl InspectError {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PermissionDenied(_) => "Permission denied",
            Self::Process(RunError::Io(error))
                if error.kind() == io::ErrorKind::PermissionDenied =>
            {
                "Permission denied"
            }
            Self::Process(RunError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                "Unavailable"
            }
            _ => "Error",
        }
    }
}
impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidService => f.write_str("invalid .service unit name"),
            Self::WorkerStopped => {
                f.write_str("Inspection worker stopped before reporting a result")
            }
            Self::Process(error) => error.fmt(f),
            Self::Command {
                program,
                code,
                diagnostic,
            } => write!(f, "{program} failed (exit {code:?}): {diagnostic}"),
            Self::PermissionDenied(message) => f.write_str(message),
            Self::Parse(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for InspectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Process(e) => Some(e),
            Self::Parse(e) => Some(e),
            _ => None,
        }
    }
}

fn explicit_denial(diagnostic: &str) -> bool {
    let diagnostic = diagnostic.to_ascii_lowercase();
    [
        "permission denied",
        "access denied",
        "operation not permitted",
        "insufficient permissions",
    ]
    .iter()
    .any(|phrase| diagnostic.contains(phrase))
}

pub(crate) fn read_command(
    runner: &impl Runner,
    program: &'static str,
    args: &[&str],
    key: &ServiceKey,
    cancelled: &AtomicBool,
) -> Result<CommandOutput, InspectError> {
    if !valid_service_name(&key.name) {
        return Err(InspectError::InvalidService);
    }
    let output = runner
        .run(program, args, cancelled)
        .map_err(InspectError::Process)?;
    let diagnostic = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let diagnostic = display_text(&String::from_utf8_lossy(diagnostic));
    // Only classify an explicitly observed denial, never infer a stopped manager
    // from a generic bus error. Successful partial-access warnings stay with data.
    let empty = std::str::from_utf8(&output.stdout)
        .is_ok_and(|text| text.trim().is_empty() || text.trim() == "-- No entries --");
    if (!output.success || empty) && explicit_denial(&diagnostic) {
        return Err(InspectError::PermissionDenied(diagnostic));
    }
    if !output.success {
        return Err(InspectError::Command {
            program,
            code: output.code,
            diagnostic,
        });
    }
    Ok(output)
}

pub enum InspectionPart {
    Properties(Result<Properties, InspectError>),
    Journal(Result<Journal, InspectError>),
}
pub struct InspectionReport {
    pub id: u64,
    pub key: ServiceKey,
    pub part: InspectionPart,
}

pub enum ReadState<T> {
    Loading,
    Ready(T),
    Failed(InspectError),
}

fn complete<T>(
    state: &mut ReadState<T>,
    freshness: &mut Freshness,
    result: Result<T, InspectError>,
) {
    match result {
        Ok(data) => {
            *state = ReadState::Ready(data);
            freshness.succeeded();
        }
        Err(error) => {
            freshness.failed(format!("{}: {error}", error.label()));
            if !matches!(state, ReadState::Ready(_)) {
                *state = ReadState::Failed(error);
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InspectionTab {
    #[default]
    Properties,
    Journal,
}

pub struct Inspection {
    pub key: ServiceKey,
    pub request_id: Option<u64>,
    pub properties: ReadState<Properties>,
    pub journal: ReadState<Journal>,
    pub properties_freshness: Freshness,
    pub journal_freshness: Freshness,
    pub tab: InspectionTab,
    pub row: usize,
    pub column: usize,
    pub column_limit: usize,
    pub page_size: usize,
}
impl Inspection {
    pub fn worker_stopped(&mut self) {
        if self.properties_freshness.refreshing {
            complete(
                &mut self.properties,
                &mut self.properties_freshness,
                Err(InspectError::WorkerStopped),
            );
        }
        if self.journal_freshness.refreshing {
            complete(
                &mut self.journal,
                &mut self.journal_freshness,
                Err(InspectError::WorkerStopped),
            );
        }
    }
    pub fn new(key: ServiceKey, tab: InspectionTab) -> Self {
        Self {
            key,
            request_id: None,
            properties: ReadState::Loading,
            journal: ReadState::Loading,
            properties_freshness: Freshness::default(),
            journal_freshness: Freshness::default(),
            tab,
            row: 0,
            column: 0,
            column_limit: 0,
            page_size: 1,
        }
    }
    pub fn apply(&mut self, report: InspectionReport) {
        if Some(report.id) != self.request_id || report.key != self.key {
            return;
        }
        match report.part {
            InspectionPart::Properties(result) => {
                complete(&mut self.properties, &mut self.properties_freshness, result);
            }
            InspectionPart::Journal(result) => {
                complete(&mut self.journal, &mut self.journal_freshness, result);
            }
        }
        self.row = self.row.min(self.lines().len().saturating_sub(1));
    }
    pub fn switch_tab(&mut self) {
        self.tab = match self.tab {
            InspectionTab::Properties => InspectionTab::Journal,
            InspectionTab::Journal => InspectionTab::Properties,
        };
        self.row = 0;
        self.column = 0;
    }

    pub fn refresh(&mut self) {
        self.request_id = None;
        self.properties_freshness.begin();
        self.journal_freshness.begin();
        if !matches!(self.properties, ReadState::Ready(_)) {
            self.properties = ReadState::Loading;
        }
        if !matches!(self.journal, ReadState::Ready(_)) {
            self.journal = ReadState::Loading;
        }
    }

    pub fn freshness(&self) -> &Freshness {
        match self.tab {
            InspectionTab::Properties => &self.properties_freshness,
            InspectionTab::Journal => &self.journal_freshness,
        }
    }
    pub fn lines(&self) -> Vec<String> {
        fn error(error: &InspectError) -> Vec<String> {
            vec![format!(
                "{}: {}",
                error.label(),
                crate::parsing::bounded_line(&error.to_string()).0
            )]
        }
        let mut lines = match self.tab {
            InspectionTab::Properties => match &self.properties {
                ReadState::Loading => vec!["Loading properties...".into()],
                ReadState::Failed(e) => error(e),
                ReadState::Ready(data) if data.values.is_empty() => {
                    vec!["No properties returned.".into()]
                }
                ReadState::Ready(data) => Property::ALL
                    .into_iter()
                    .map(|property| {
                        format!(
                            "{}: {}",
                            property.name(),
                            match data.values.get(&property) {
                                Some(value) if value.is_empty() => "(empty)",
                                Some(value) => value,
                                None => "(not reported)",
                            }
                        )
                    })
                    .collect(),
            },
            InspectionTab::Journal => match &self.journal {
                ReadState::Loading => vec!["Loading journal...".into()],
                ReadState::Failed(e) => error(e),
                ReadState::Ready(data) if data.lines.is_empty() => {
                    vec!["No recent journal entries returned.".into()]
                }
                ReadState::Ready(data) => data.lines.clone(),
            },
        };
        let warning = match self.tab {
            InspectionTab::Properties => match &self.properties {
                ReadState::Ready(data) => data.warning.as_str(),
                _ => "",
            },
            InspectionTab::Journal => match &self.journal {
                ReadState::Ready(data) => data.warning.as_str(),
                _ => "",
            },
        };
        if !warning.is_empty() {
            lines.insert(
                0,
                format!("WARNING: {}", crate::parsing::bounded_line(warning).0),
            );
        }
        if matches!(&self.journal, ReadState::Ready(data) if data.truncated)
            && self.tab == InspectionTab::Journal
        {
            lines.insert(0, "[Journal display truncated]".into());
        }
        let failed = match self.tab {
            InspectionTab::Properties => matches!(self.properties, ReadState::Failed(_)),
            InspectionTab::Journal => matches!(self.journal, ReadState::Failed(_)),
        };
        if !failed && let Some(error) = &self.freshness().last_error {
            lines.insert(
                0,
                format!(
                    "Last refresh error: {}",
                    crate::parsing::bounded_line(error).0
                ),
            );
        }
        lines
    }
    pub fn scroll(&mut self, delta: isize) {
        self.row = self
            .row
            .saturating_add_signed(delta)
            .min(self.lines().len().saturating_sub(1));
    }
}
