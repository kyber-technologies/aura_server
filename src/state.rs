use crate::config;
use crate::database::{Database, DatabaseConnection};
use crate::email::EmailRegister;
use crate::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone)]
pub struct ServerState {
    database: Database,
    emails: Arc<EmailRegister>,
    exit: Arc<Notify>,
}

impl ServerState {
    pub async fn create() -> Self {
        Self {
            database: Database::connect().await,
            emails: Arc::new(EmailRegister::new()),
            exit: Arc::new(Notify::new()),
        }
    }

    pub async fn maintain(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(config::get().runtime.maintain_interval));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    tracing::info!("Maintaining server state...");
                    self.emails.maintain();
                }

                _ = self.exit.notified() => {
                    break;
                }
            }
        }
    }

    pub fn set_exit(&self) {
        self.exit.notify_waiters();
    }

    pub async fn wait_for_exit(&self) {
        self.exit.notified().await;
    }

    pub async fn database(&self) -> Result<DatabaseConnection, Error> {
        self.database.get().await
    }

    pub fn emails(&self) -> &EmailRegister {
        &self.emails
    }

    pub fn dispose(self) {
        self.database.dispose();
    }

    #[cfg(feature = "testing")]
    pub async fn clear_state(&self) -> Result<(), Error> {
        use diesel_async::RunQueryDsl;

        tracing::info!("Detected test environment. Clearing database...");

        let mut database = self.database().await?;

        diesel::sql_query(
            r#"
            TRUNCATE TABLE
                messages,
                channel_members,
                channels,
                resources,
                users
            CASCADE
    "#,
        )
        .execute(&mut database)
        .await?;

        tracing::info!("Re-applying migrations...");
        self.database.run_migrations().await;

        tracing::info!("Creating test user with role user...");

        crate::user::create(&mut database, crate::testing::test_user())
            .await
            .expect("Failed to create new test user");

        tracing::info!("Creating test user with role moderator...");

        crate::user::create(&mut database, crate::testing::moderator_user())
            .await
            .expect("Failed to create moderator test user");

        tracing::info!("Creating test user with role admin...");

        crate::user::create(&mut database, crate::testing::admin_user())
            .await
            .expect("Failed to create admin test user");

        Ok(())
    }

    pub fn print_status(&self) {
        self.database.print_status();
        self.emails.print_status();
        tracing::info!("Config: {:#?}", config::get());
    }
}
