use std::time::Instant;

/// A successful empty result is still a snapshot. Age never resets on failure.
#[derive(Debug)]
pub struct Freshness {
    pub refreshing: bool,
    pub last_success: Option<Instant>,
    pub last_error: Option<String>,
}
impl Default for Freshness {
    fn default() -> Self {
        Self {
            refreshing: true,
            last_success: None,
            last_error: None,
        }
    }
}
impl Freshness {
    pub fn begin(&mut self) {
        self.refreshing = true;
    }
    pub fn succeeded(&mut self) {
        self.refreshing = false;
        self.last_success = Some(Instant::now());
        self.last_error = None;
    }
    pub fn failed(&mut self, message: String) {
        self.refreshing = false;
        self.last_error = Some(message);
    }
    pub fn is_stale(&self) -> bool {
        self.last_success.is_some() && (self.refreshing || self.last_error.is_some())
    }
    pub fn summary(&self) -> String {
        self.summary_at(Instant::now())
    }
    pub fn summary_at(&self, now: Instant) -> String {
        match self.last_success {
            Some(success) => format!(
                "{}Last success {}s ago{}",
                if self.is_stale() { "STALE | " } else { "" },
                now.saturating_duration_since(success).as_secs(),
                if self.refreshing {
                    " | Refreshing..."
                } else if self.last_error.is_some() {
                    " | refresh failed"
                } else {
                    ""
                }
            ),
            None if self.refreshing => "Loading...".into(),
            None => "No successful result".into(),
        }
    }
}
