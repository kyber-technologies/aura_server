use crate::config;
use crate::database::Database;
use crate::email::EmailRegister;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ServerState {
    database: Database,
    emails: Arc<EmailRegister>,
    exit: Arc<AtomicBool>,
}

impl ServerState {
    pub async fn new() -> Self {
        Self {
            database: Database::new().await,
            emails: Arc::new(EmailRegister::new()),
            exit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn maintain(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(config::get().rt_maintain_interval));

        while !self.exit.load(Ordering::SeqCst) {
            tracing::info!("Maintaining server state...");
            self.emails.maintain();

            interval.tick().await;
        }
    }

    pub fn set_exit(&self) {
        self.exit.store(true, Ordering::SeqCst);
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn emails(&self) -> &EmailRegister {
        &self.emails
    }
}
