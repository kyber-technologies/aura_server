use crate::schema::messages;
use chrono::{DateTime, Utc};
use diesel::{Insertable, Queryable, Selectable};
use serde_json::Value;

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Message {
    pub message_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
}
