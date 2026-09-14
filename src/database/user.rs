use crate::database::channel::ChannelData;
use crate::schema::{user_blocks, users};
use crate::types::user::UserRole;
use chrono::{DateTime, Utc};
use diesel::{Insertable, Queryable, Selectable};

#[derive(Debug)]
pub struct UserData {
    pub user: User,
    pub channels: Vec<ChannelData>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
    pub icon: serde_json::Value,
    pub notifications: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = user_blocks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserBlock {
    pub user_id: String,
    pub blocked_user_id: String,
}
