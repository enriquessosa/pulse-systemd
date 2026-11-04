use crate::{
    inspection::{InspectError, read_command},
    model::{JOURNAL_ENTRIES, Journal, Scope, ServiceKey, display_text},
    parsing::parse_journal,
    process::Runner,
};
use std::sync::atomic::AtomicBool;

pub struct JournalReader<R> {
    runner: R,
}
impl<R: Runner> JournalReader<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
    pub fn recent(
        &self,
        key: &ServiceKey,
        cancelled: &AtomicBool,
    ) -> Result<Journal, InspectError> {
        let unit = match key.scope {
            Scope::System => format!("--unit={}", key.name),
            Scope::User => format!("--user-unit={}", key.name),
        };
        let lines = format!("--lines={JOURNAL_ENTRIES}");
        let output = read_command(
            &self.runner,
            "journalctl",
            &[
                key.scope.flag(),
                &unit,
                "--no-pager",
                &lines,
                "--output=short-iso-precise",
                "--all",
                "--no-hostname",
            ],
            key,
            cancelled,
        )?;
        let mut journal = parse_journal(&output.stdout).map_err(InspectError::Parse)?;
        journal.warning = display_text(&String::from_utf8_lossy(&output.stderr));
        Ok(journal)
    }
}
