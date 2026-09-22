use crate::config;
use crate::error::Error;
use diesel_async::AsyncMigrationHarness;
use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub mod channel;
pub mod message;
pub mod posting;
pub mod resource;
pub mod user;

pub type DatabaseConnection = Object<AsyncPgConnection>;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Clone)]
pub struct Database {
    pool: Pool<AsyncPgConnection>,
}

impl Database {
    pub async fn connect() -> Self {
        let config = config::get();

        let pass = std::fs::read_to_string(&config.database.password)
            .expect("Failed to read database password")
            .trim()
            .to_string();

        let url = format!(
            "postgres://{}:{}@{}:{}/aura",
            config.database.user, pass, config.database.host, config.database.port,
        );

        let pool = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(url))
            .max_size(config.database.max_pool_size)
            .build()
            .expect("Failed to build database connection pool");

        let this = Self { pool };

        this.run_migrations().await;

        this
    }

    pub async fn get(&self) -> Result<DatabaseConnection, Error> {
        Ok(self.pool.get().await?)
    }

    pub async fn run_migrations(&self) {
        let conn = self
            .get()
            .await
            .expect("Failed to establish database connection");

        let mut harness = AsyncMigrationHarness::new(conn);

        harness
            .run_pending_migrations(MIGRATIONS)
            .expect("Failed to run pending migrations");
    }

    pub fn dispose(&self) {
        self.pool.close();
    }

    pub fn print_status(&self) {
        tracing::info!("Database Status: {:#?}", self.pool.status())
    }
}
