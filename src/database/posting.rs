use crate::schema::{post_reactions, posts};
use crate::types::posting::PostReaction;
use chrono::{DateTime, Utc};
use diesel::pg::sql_types::Timestamptz;
use diesel::sql_types::Double;
use diesel::sql_types::Text;
use diesel::{Insertable, Queryable, QueryableByName, Selectable};
use pgvector::Vector;
use serde_json::Value;

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = posts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Post {
    pub post_id: String,
    pub author_id: String,
    pub content: Value,
    pub timestamp: DateTime<Utc>,
    pub parent_id: Option<String>,
    pub embedding: Option<Vector>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = post_reactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PostReactionRow {
    pub post_id: String,
    pub user_id: String,
    pub reaction: PostReaction,
}

#[derive(Debug)]
pub struct PostData {
    pub post: Post,
    pub reaction_counts: Vec<(PostReaction, i64)>,
    pub user_reaction: PostReaction,
}

#[derive(Clone, Debug, QueryableByName)]
pub struct RecommendationCandidateRow {
    #[diesel(sql_type = Text)]
    pub post_id: String,
    #[diesel(sql_type = Double)]
    pub social_weight: f64,
    #[diesel(sql_type = Text)]
    pub author_id: String,
    #[diesel(sql_type = Timestamptz)]
    pub created_at: DateTime<Utc>,
    #[diesel(sql_type = Double)]
    pub vector_sim: f64,
}
