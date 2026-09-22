use crate::schema::{user_blocks, user_follows, users};
use crate::types::user::UserRole;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use pgvector::Vector;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Queryable, Selectable, Insertable, Identifiable)]
#[diesel(table_name = users, primary_key(user_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
    pub icon: Value,
    pub notifications: Value,
    pub created_at: DateTime<Utc>,
    pub embedding: Option<Vector>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = user_blocks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserBlock {
    pub user_id: String,
    pub blocked_user_id: String,
}

#[derive(
    Clone, Debug, PartialEq, Queryable, Selectable, Insertable, Identifiable, Associations,
)]
#[diesel(table_name = user_follows, primary_key(follower_id, followed_id))]
#[diesel(belongs_to(User, foreign_key = follower_id))]
pub struct UserFollow {
    pub follower_id: String,
    pub followed_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserData {
    pub user: User,
    pub channels: Vec<crate::database::channel::ChannelData>,
    pub followers: Vec<String>,
    pub following: Vec<String>,
}
