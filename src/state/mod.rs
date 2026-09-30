use crate::config;
use crate::error::Error;
use crate::state::cooldowns::CooldownManager;
use crate::state::database::{Database, DatabaseConnection};
use crate::state::id_factory::IdFactory;
use email::EmailRegistry;
use embedder::TextEmbedder;
use std::sync::Arc;
use tokio::sync::Notify;
use tonic::Request;

pub mod cooldowns;
pub mod database;
pub mod email;
pub mod embedder;
pub mod id_factory;

#[derive(Clone)]
pub struct ServerState {
    database: Database,
    emails: Arc<EmailRegistry>,
    exit: Arc<Notify>,
    embedder: TextEmbedder,
    id_factory: IdFactory,
    cooldown: CooldownManager,
}

impl ServerState {
    pub async fn create() -> Self {
        let embedder = TextEmbedder::new();

        tracing::info!("Testing text embedder...");
        // Embed test data to download any missing models
        embedder
            .embed(vec!["test".to_string()])
            .await
            .expect("Failed to embed test data");

        let machine_id = config::get().machine_id.as_str();

        if machine_id.is_empty() {
            panic!("No machine ID specified!");
        }

        Self {
            database: Database::connect().await,
            emails: Arc::new(EmailRegistry::new().await),
            exit: Arc::new(Notify::new()),
            embedder,
            id_factory: IdFactory::new(machine_id.parse().expect("Invalid machine ID")),
            cooldown: CooldownManager::new(),
        }
    }

    pub async fn maintain(&self) {
        let mut interval = tokio::time::interval(config::get().runtime.maintain_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    tracing::info!("Maintaining server state...");
                    // TODO: Maintain governor
                    self.emails.maintain();
                    self.cooldown.maintain();
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

    pub fn emails(&self) -> &EmailRegistry {
        &self.emails
    }

    pub fn embedder(&self) -> &TextEmbedder {
        &self.embedder
    }

    pub fn id_factory(&self) -> &IdFactory {
        &self.id_factory
    }

    pub fn throttle<T>(&self, req: &Request<T>) -> Result<(), Error> {
        self.cooldown.throttle(req)
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
                users,
                user_follows,
                user_blocks,
                channels,
                channel_members,
                messages,
                resources,
                posts,
                post_reactions
            CASCADE
    "#,
        )
        .execute(&mut database)
        .await?;

        tracing::info!("Re-applying migrations...");
        self.database.run_migrations().await;

        tracing::info!("Creating test user with role user...");

        crate::user::create(&mut database, crate::testing::test_user(true))
            .await
            .expect("Failed to create new test user");

        tracing::info!("Creating test user with role moderator...");

        crate::user::create(&mut database, crate::testing::moderator_user(true))
            .await
            .expect("Failed to create moderator test user");

        tracing::info!("Creating test user with role admin...");

        crate::user::create(&mut database, crate::testing::admin_user(true))
            .await
            .expect("Failed to create admin test user");

        tracing::info!("Cleaning resource directory...");
        let mut res_entries = tokio::fs::read_dir(&config::get().service.resource_dir)
            .await
            .map_err(|e| Error::internal(format!("Failed to read resource directory: {}", e)))?;

        while let Some(entry) = res_entries.next_entry().await.map_err(|err| {
            Error::internal(format!("Failed to read resource directory entry: {err}"))
        })? {
            // Don't delete the built-in "aura" directory
            if entry.file_name() != "aura" {
                tokio::fs::remove_dir_all(entry.path()).await.map_err(|e| {
                    Error::internal(format!("Failed to remove resource directory: {}", e))
                })?;
            }
        }

        Ok(())
    }

    pub fn print_status(&self) {
        self.database.print_status();
        self.emails.print_status();
        tracing::info!("Config: {:#?}", config::get());
    }
}
