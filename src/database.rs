use crate::config;
use std::ops::Deref;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

#[derive(Clone, Debug)]
pub struct Database {
    surreal: Surreal<Client>,
}

impl Database {
    pub async fn new() -> Self {
        let config = config::get();

        let surreal = Surreal::new::<Ws>(config.db_address.as_str())
            .await
            .expect("Failed to connect to database");

        surreal
            .signin(Root {
                username: config.db_user.to_string(),
                password: config.database_password(),
            })
            .await
            .expect("Failed to login to database");

        surreal
            .use_ns("aura")
            .use_db("database")
            .await
            .expect("Failed to get into database");

        let this = Self { surreal };

        this.setup().await;

        this
    }

    pub async fn setup(&self) {
        self.query(
            r#"
DEFINE TABLE IF NOT EXISTS user SCHEMALESS;
DEFINE TABLE IF NOT EXISTS channel SCHEMALESS;
DEFINE TABLE IF NOT EXISTS message SCHEMALESS;
DEFINE TABLE IF NOT EXISTS resource SCHEMALESS;
DEFINE TABLE IF NOT EXISTS blocked TYPE RELATION;
"#,
        )
        .await
        .expect("Failed to setup user table");
    }
}

impl Deref for Database {
    type Target = Surreal<Client>;

    fn deref(&self) -> &Self::Target {
        &self.surreal
    }
}
