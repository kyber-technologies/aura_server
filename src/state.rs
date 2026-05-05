use crate::database::Database;

#[derive(Clone, Debug)]
pub struct ServerState {
    database: Database,
}

impl ServerState {
    pub async fn new() -> Self {
        Self {
            database: Database::new().await,
        }
    }

    pub fn database(&self) -> &Database {
        &self.database
    }
}
