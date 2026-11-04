use crate::{
    inspection::{InspectionPart, InspectionReport},
    journal::JournalReader,
    latest_worker::{LatestWorker, send_report},
    model::ServiceKey,
    process::Runner,
    systemd::Systemd,
};
use std::io;

pub type InspectionWorker = LatestWorker<ServiceKey, InspectionReport>;
impl InspectionWorker {
    pub fn spawn(runner: impl Runner + Send + 'static) -> io::Result<Self> {
        Self::start("pulse-inspection", move |key, id, cancelled, sender| {
            let properties = Systemd::new(&runner).properties(&key, cancelled);
            if !send_report(
                sender,
                InspectionReport {
                    id,
                    key: key.clone(),
                    part: InspectionPart::Properties(properties),
                },
                cancelled,
            ) {
                return;
            }
            let journal = JournalReader::new(&runner).recent(&key, cancelled);
            send_report(
                sender,
                InspectionReport {
                    id,
                    key,
                    part: InspectionPart::Journal(journal),
                },
                cancelled,
            );
        })
    }
}
