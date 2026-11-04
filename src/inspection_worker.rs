use crate::{
    inspection::{InspectionPart, InspectionReport},
    journal::JournalReader,
    model::ServiceKey,
    process::Runner,
    systemd::Systemd,
};
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

struct Request {
    id: u64,
    key: ServiceKey,
    cancelled: Arc<AtomicBool>,
}
#[derive(Default)]
struct Pending {
    request: Option<Request>,
    cancelled: Option<Arc<AtomicBool>>,
    stopped: bool,
}

/// One running request, one replaceable pending request, and two bounded results.
/// Replacing a selection never joins a thread or waits on subprocess execution.
pub struct InspectionWorker {
    shared: Arc<(Mutex<Pending>, Condvar)>,
    receiver: Receiver<InspectionReport>,
    thread: Option<JoinHandle<()>>,
    next_id: u64,
}

fn send(
    sender: &SyncSender<InspectionReport>,
    mut report: InspectionReport,
    cancelled: &AtomicBool,
) -> bool {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return false;
        }
        match sender.try_send(report) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(value)) => {
                report = value;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl InspectionWorker {
    pub fn spawn(runner: impl Runner + Send + 'static) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let (sender, receiver) = mpsc::sync_channel(2);
        let thread = thread::Builder::new()
            .name("pulse-inspection".into())
            .spawn(move || {
                let systemd = Systemd::new(&runner);
                let journal = JournalReader::new(&runner);
                loop {
                    let (lock, wake) = &*worker_shared;
                    let Ok(mut pending) = lock.lock() else {
                        break;
                    };
                    while pending.request.is_none() && !pending.stopped {
                        pending = match wake.wait(pending) {
                            Ok(pending) => pending,
                            Err(_) => return,
                        };
                    }
                    if pending.stopped {
                        break;
                    }
                    let Some(request) = pending.request.take() else {
                        continue;
                    };
                    drop(pending);
                    let properties = systemd.properties(&request.key, &request.cancelled);
                    if !send(
                        &sender,
                        InspectionReport {
                            id: request.id,
                            key: request.key.clone(),
                            part: InspectionPart::Properties(properties),
                        },
                        &request.cancelled,
                    ) {
                        continue;
                    }
                    let logs = journal.recent(&request.key, &request.cancelled);
                    send(
                        &sender,
                        InspectionReport {
                            id: request.id,
                            key: request.key,
                            part: InspectionPart::Journal(logs),
                        },
                        &request.cancelled,
                    );
                }
            })?;
        Ok(Self {
            shared,
            receiver,
            thread: Some(thread),
            next_id: 0,
        })
    }

    pub fn request(&mut self, key: ServiceKey) -> io::Result<u64> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("inspection request id exhausted"))?;
        let (lock, wake) = &*self.shared;
        let mut pending = lock
            .lock()
            .map_err(|_| io::Error::other("inspection worker state poisoned"))?;
        if let Some(cancelled) = pending.cancelled.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        pending.cancelled = Some(Arc::clone(&cancelled));
        pending.request = Some(Request {
            id: self.next_id,
            key,
            cancelled,
        });
        wake.notify_one();
        Ok(self.next_id)
    }
    pub fn cancel_current(&self) {
        if let Ok(mut pending) = self.shared.0.lock() {
            if let Some(cancelled) = pending.cancelled.take() {
                cancelled.store(true, Ordering::Relaxed);
            }
            pending.request = None;
        }
    }
    pub fn try_recv(&self) -> Result<InspectionReport, TryRecvError> {
        self.receiver.try_recv()
    }
}
impl Drop for InspectionWorker {
    fn drop(&mut self) {
        self.cancel_current();
        if let Ok(mut pending) = self.shared.0.lock() {
            pending.stopped = true;
        }
        self.shared.1.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
