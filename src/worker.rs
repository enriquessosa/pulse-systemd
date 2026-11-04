use crate::{
    latest_worker::{LatestWorker, send_report},
    model::Scope,
    process::Runner,
    systemd::{Discovery, DiscoveryError, Systemd},
};
use std::io;

pub struct DiscoveryReport {
    pub id: u64,
    pub scope: Scope,
    pub result: Result<Discovery, DiscoveryError>,
}

pub type DiscoveryWorker = LatestWorker<(), DiscoveryReport>;
impl DiscoveryWorker {
    pub fn spawn(runner: impl Runner + Send + 'static) -> io::Result<Self> {
        let mut worker = Self::start("pulse-discovery", move |(), id, cancelled, sender| {
            let backend = Systemd::new(&runner);
            for scope in [Scope::System, Scope::User] {
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let result = backend.discover(scope, cancelled);
                if !send_report(sender, DiscoveryReport { id, scope, result }, cancelled) {
                    return;
                }
            }
        })?;
        worker.request(())?;
        Ok(worker)
    }
}
