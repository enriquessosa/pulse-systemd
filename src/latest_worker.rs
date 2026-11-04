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

struct Request<Q> {
    id: u64,
    value: Q,
    cancelled: Arc<AtomicBool>,
}
struct Pending<Q> {
    request: Option<Request<Q>>,
    cancelled: Option<Arc<AtomicBool>>,
    stopped: bool,
}

impl<Q> Default for Pending<Q> {
    fn default() -> Self {
        Self {
            request: None,
            cancelled: None,
            stopped: false,
        }
    }
}

/// One running request, one replaceable pending request, and two bounded results.
/// Replacing a selection never joins a thread or waits on subprocess execution.
pub struct LatestWorker<Q, R> {
    shared: Arc<(Mutex<Pending<Q>>, Condvar)>,
    receiver: Receiver<R>,
    thread: Option<JoinHandle<()>>,
    next_id: u64,
}

pub(crate) fn send_report<R>(
    sender: &SyncSender<R>,
    mut report: R,
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

impl<Q, R> LatestWorker<Q, R> {
    pub(crate) fn start(
        name: &str,
        mut handle: impl FnMut(Q, u64, &AtomicBool, &SyncSender<R>) + Send + 'static,
    ) -> io::Result<Self>
    where
        Q: Send + 'static,
        R: Send + 'static,
    {
        let shared = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let (sender, receiver) = mpsc::sync_channel(2);
        let thread = thread::Builder::new().name(name.into()).spawn(move || {
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
                handle(request.value, request.id, &request.cancelled, &sender);
            }
        })?;
        Ok(Self {
            shared,
            receiver,
            thread: Some(thread),
            next_id: 0,
        })
    }

    pub fn request(&mut self, value: Q) -> io::Result<u64> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("worker request id exhausted"))?;
        let (lock, wake) = &*self.shared;
        let mut pending = lock
            .lock()
            .map_err(|_| io::Error::other("worker state poisoned"))?;
        if let Some(cancelled) = pending.cancelled.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        pending.cancelled = Some(Arc::clone(&cancelled));
        pending.request = Some(Request {
            id: self.next_id,
            value,
            cancelled,
        });
        wake.notify_one();
        Ok(self.next_id)
    }
    pub fn last_request_id(&self) -> u64 {
        self.next_id
    }
    pub fn cancel_current(&self) {
        if let Ok(mut pending) = self.shared.0.lock() {
            if let Some(cancelled) = pending.cancelled.take() {
                cancelled.store(true, Ordering::Relaxed);
            }
            pending.request = None;
        }
    }
    pub fn try_recv(&self) -> Result<R, TryRecvError> {
        self.receiver.try_recv()
    }
}
impl<Q, R> Drop for LatestWorker<Q, R> {
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
