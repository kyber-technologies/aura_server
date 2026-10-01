use std::time::Duration;
use tokio::time::Instant;

pub struct ServerStatus {
    start: Instant,
    last_maintain: Instant,
    last_maintain_duration: Duration,
}

impl ServerStatus {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            last_maintain: Instant::now(),
            last_maintain_duration: Duration::ZERO,
        }
    }

    pub fn start_maintain(&mut self) {
        self.last_maintain = Instant::now();
    }

    pub fn end_maintain(&mut self) {
        self.last_maintain_duration = self.last_maintain.elapsed();
    }

    pub fn uptime(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn last_maintain_duration(&self) -> Duration {
        self.last_maintain_duration
    }
}
