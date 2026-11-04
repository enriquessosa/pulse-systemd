use crate::{
    model::Scope,
    process::Runner,
    systemd::{ScopeDiscovery, Systemd},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
};

pub struct DiscoveryWorker {
    cancelled: Arc<AtomicBool>,
    receiver: Receiver<ScopeDiscovery>,
    thread: Option<JoinHandle<()>>,
}

impl DiscoveryWorker {
    pub fn spawn(runner: impl Runner + Send + 'static) -> io::Result<Self> {
        // Exactly one result per scope; sending cannot wait on UI consumption.
        let (sender, receiver) = mpsc::sync_channel(2);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let thread = thread::Builder::new()
            .name("pulse-discovery".into())
            .spawn(move || {
                let backend = Systemd::new(runner);
                for scope in [Scope::System, Scope::User] {
                    if worker_cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let result = backend.discover(scope, &worker_cancelled);
                    if sender.send(ScopeDiscovery { scope, result }).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            cancelled,
            receiver,
            thread: Some(thread),
        })
    }

    pub fn try_recv(&self) -> Result<ScopeDiscovery, TryRecvError> {
        self.receiver.try_recv()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for DiscoveryWorker {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            // The production runner observes cancellation and reaps its child.
            // Join after terminal restoration so even an exit-path error is safe.
            let _ = thread.join();
        }
    }
}
