use crate::config;
use crate::database::{Database, DatabaseConnection};
use crate::email::EmailRegister;
use crate::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[cfg(feature = "testing")]
pub const TEST_NEW_USER_NAME: &str = "user";
#[cfg(feature = "testing")]
pub const TEST_NEW_USER_PASS: &str = "user";

#[cfg(feature = "testing")]
pub const TEST_MODERATOR_NAME: &str = "moderator";
#[cfg(feature = "testing")]
pub const TEST_MODERATOR_PASS: &str = "moderator";

#[cfg(feature = "testing")]
pub const TEST_ADMIN_NAME: &str = "admin";
#[cfg(feature = "testing")]
pub const TEST_ADMIN_PASS: &str = "admin";

#[derive(Clone)]
pub struct ServerState {
    database: Database,
    emails: Arc<EmailRegister>,
    exit: Arc<AtomicBool>,
}

impl ServerState {
    pub async fn create() -> Self {
        Self {
            database: Database::connect().await,
            emails: Arc::new(EmailRegister::new()),
            exit: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn maintain(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(config::get().runtime.maintain_interval));

        while !self.exit.load(Ordering::SeqCst) {
            tracing::info!("Maintaining server state...");
            self.emails.maintain();

            interval.tick().await;
        }
    }

    pub fn set_exit(&self) {
        self.exit.store(true, Ordering::SeqCst);
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
        use crate::types::common::Timestamp;
        use crate::types::user::{Notifications, User, UserRole};
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

        crate::user::create(
            &mut database,
            User {
                user_id: TEST_NEW_USER_NAME.to_string(),
                username: TEST_NEW_USER_NAME.to_string(),
                email: "user@foo.bar".to_string(),
                password: crate::auth::hash(TEST_NEW_USER_PASS.to_string())
                    .expect("Failed to hash new user password"),
                role: UserRole::User,
                icon: crate::logic::resource::build_user_avatar_id(TEST_NEW_USER_NAME),
                notifications: Notifications(Vec::new()),
                channels: Vec::new(),
                created_at: Timestamp::now(),
            },
        )
        .await
        .expect("Failed to create new test user");

        tracing::info!("Creating test user with role moderator...");

        crate::user::create(
            &mut database,
            User {
                user_id: TEST_MODERATOR_NAME.to_string(),
                username: TEST_MODERATOR_NAME.to_string(),
                email: "moderator@foo.bar".to_string(),
                password: crate::auth::hash(TEST_MODERATOR_PASS.to_string())
                    .expect("Failed to hash moderator password"),
                role: UserRole::Moderator,
                icon: crate::logic::resource::build_user_avatar_id(TEST_MODERATOR_NAME),
                notifications: Notifications(Vec::new()),
                channels: Vec::new(),
                created_at: Timestamp::now(),
            },
        )
        .await
        .expect("Failed to create moderator test user");

        tracing::info!("Creating test user with role admin...");

        crate::user::create(
            &mut database,
            User {
                user_id: TEST_ADMIN_NAME.to_string(),
                username: TEST_ADMIN_NAME.to_string(),
                email: "admin@foo.bar".to_string(),
                password: crate::auth::hash(TEST_ADMIN_PASS.to_string())
                    .expect("Failed to hash admin password"),
                role: UserRole::Admin,
                icon: crate::logic::resource::build_user_avatar_id(TEST_ADMIN_NAME),
                notifications: Notifications(Vec::new()),
                channels: Vec::new(),
                created_at: Timestamp::now(),
            },
        )
        .await
        .expect("Failed to create admin test user");

        Ok(())
    }
}
