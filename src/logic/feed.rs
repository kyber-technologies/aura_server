use crate::database::DatabaseConnection;
use crate::database::posting::FeedCandidateRow;
use crate::error::Error;
use crate::schema::{post_reactions, posts, users};
use crate::types::posting::PostReaction;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_async::RunQueryDsl;
use pgvector::Vector;

pub async fn fetch_feed(
    database: &mut DatabaseConnection,
    user_id: &str,
    user_vector: Option<Vector>,
    limit: usize,
) -> Result<Vec<String>, Error> {
    let now = Utc::now();

    let vector_param = match user_vector {
        Some(v) => v,
        None => {
            let fetched: Option<Option<Vector>> = users::table
                .filter(users::user_id.eq(user_id))
                .select(users::embedding)
                .first::<Option<Vector>>(database)
                .await
                .optional()?;

            fetched
                .flatten()
                .unwrap_or_else(|| Vector::from(vec![0.0; 384]))
        }
    };

    let candidates = fetch_candidates(database, user_id, &vector_param).await?;

    let mut scored_posts: Vec<ScoredPost> = candidates
        .into_iter()
        .filter(|candidate| candidate.author_id != user_id) // Exclude own posts
        .map(|candidate| {
            let total_interactions = (candidate.likes + candidate.dislikes) as f32;
            let balance_penalty = if total_interactions > 0.0 {
                1.0 - ((candidate.likes - candidate.dislikes).abs() as f32
                    / (total_interactions + 1.0))
            } else {
                0.5
            };

            let controversy_score = total_interactions * balance_penalty;
            let base_content_score = (candidate.vector_sim * 2.0) + controversy_score;
            let weighted_score = candidate.social_weight * base_content_score;

            let hours_old = (now - candidate.created_at).num_minutes() as f32 / 60.0;
            let time_decay = (hours_old + 2.0).powf(1.6);

            let final_score = weighted_score / time_decay;

            ScoredPost {
                post_id: candidate.post_id,
                score: final_score,
            }
        })
        .collect();

    scored_posts.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(scored_posts
        .into_iter()
        .take(limit)
        .map(|p| p.post_id)
        .collect())
}

pub async fn fetch_candidates(
    database: &mut DatabaseConnection,
    user_id: &str,
    user_vector: &Vector,
) -> Result<Vec<FeedCandidate>, Error> {
    let sql = r#"
        WITH
        blocked_users AS (
            SELECT blocked_id FROM user_blocks WHERE blocker_id = $1
        ),
        direct_follows AS (
            SELECT followed_id
            FROM user_follows
            WHERE follower_id = $1
              AND followed_id NOT IN (SELECT blocked_id FROM blocked_users)
        ),
        two_hop_graph AS (
            SELECT uf2.followed_id AS user_id FROM user_follows uf1
            JOIN user_follows uf2 ON uf1.followed_id = uf2.follower_id
            WHERE uf1.follower_id = $1
              AND uf2.followed_id <> $1
              AND uf2.followed_id NOT IN (SELECT blocked_id FROM blocked_users)
            UNION
            SELECT uf4.follower_id AS user_id FROM user_follows uf3
            JOIN user_follows uf4 ON uf3.follower_id = uf4.followed_id
            WHERE uf3.followed_id = $1
              AND uf4.follower_id <> $1
              AND uf4.follower_id NOT IN (SELECT blocked_id FROM blocked_users)
        ),
        candidate_posts AS (
            SELECT p.post_id, 1.5::float8 AS social_weight
            FROM posts p
            JOIN direct_follows df ON p.author_id = df.followed_id
            WHERE p.created_at >= NOW() - INTERVAL '7 days'

            UNION ALL

            SELECT p.post_id, 1.25::float8 AS social_weight
            FROM posts p
            JOIN two_hop_graph th ON p.author_id = th.user_id
            WHERE p.created_at >= NOW() - INTERVAL '7 days'

            UNION ALL

            SELECT p.post_id, 1.0::float8 AS social_weight
            FROM posts p
            WHERE p.embedding IS NOT NULL
              AND p.author_id NOT IN (SELECT blocked_id FROM blocked_users)
            ORDER BY p.embedding <=> $2
            LIMIT 50
        )
        SELECT DISTINCT ON (cp.post_id)
            cp.post_id,
            cp.social_weight,
            p.author_id,
            p.created_at,
            COALESCE(1.0 - (p.embedding <=> $2), 0.0)::float8 AS vector_sim
        FROM candidate_posts cp
        JOIN posts p ON cp.post_id = p.post_id
        WHERE p.author_id NOT IN (SELECT blocked_id FROM blocked_users)
          AND cp.post_id NOT IN (
              SELECT post_id FROM post_reactions WHERE user_id = $1
          )
        LIMIT 200;
    "#;

    let raw_rows = diesel::sql_query(sql)
        .bind::<Text, _>(user_id)
        .bind::<pgvector::sql_types::Vector, _>(user_vector)
        .load::<FeedCandidateRow>(database)
        .await?;

    if raw_rows.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_ids: Vec<String> = raw_rows.iter().map(|r| r.post_id.clone()).collect();

    let reactions = post_reactions::table
        .filter(post_reactions::post_id.eq_any(&candidate_ids))
        .select((post_reactions::post_id, post_reactions::reaction))
        .load::<(String, PostReaction)>(database)
        .await?;

    use std::collections::HashMap;
    let mut reaction_counts: HashMap<String, (i64, i64)> = HashMap::new();

    for (post_id, reaction) in reactions {
        let entry = reaction_counts.entry(post_id).or_insert((0, 0));
        match reaction {
            PostReaction::Like => entry.0 += 1,
            PostReaction::Dislike => entry.1 += 1,
            _ => {}
        }
    }

    let candidates = raw_rows
        .into_iter()
        .map(|row| {
            let (likes, dislikes) = reaction_counts.get(&row.post_id).copied().unwrap_or((0, 0));

            FeedCandidate {
                post_id: row.post_id,
                created_at: row.created_at,
                social_weight: row.social_weight as f32,
                vector_sim: row.vector_sim as f32,
                likes,
                dislikes,
                author_id: row.author_id,
            }
        })
        .collect();

    Ok(candidates)
}

pub async fn fetch_user_vector(
    database: &mut DatabaseConnection,
    user_id: &str,
) -> Result<Option<Vector>, Error> {
    let liked_embeddings = posts::table
        .inner_join(post_reactions::table.on(post_reactions::post_id.eq(posts::post_id)))
        .filter(post_reactions::user_id.eq(user_id))
        .filter(post_reactions::reaction.eq(PostReaction::Like))
        .filter(posts::embedding.is_not_null())
        .select(posts::embedding)
        .order_by(posts::timestamp.desc())
        .limit(20)
        .load::<Option<Vector>>(database)
        .await?;

    let valid_vectors: Vec<Vector> = liked_embeddings.into_iter().flatten().collect();

    if valid_vectors.is_empty() {
        return Ok(None);
    }

    let dim = 384;
    let mut sum_vec = vec![0.0f32; dim];
    let count = valid_vectors.len() as f32;

    for vec in &valid_vectors {
        for (i, val) in vec.as_slice().iter().enumerate() {
            sum_vec[i] += val;
        }
    }

    let avg_vec: Vec<f32> = sum_vec.into_iter().map(|val| val / count).collect();
    Ok(Some(Vector::from(avg_vec)))
}

pub async fn update_user_vector(
    database: &mut DatabaseConnection,
    user_id: &str,
    post_vector: &Vector,
    interaction: PostInteraction,
    revert: bool,
) -> Result<(), Error> {
    let mut weight = interaction.weight();

    if revert {
        weight = -weight;
    }

    let current_vec_opt: Option<Option<Vector>> = users::table
        .filter(users::user_id.eq(user_id))
        .select(users::embedding)
        .first::<Option<Vector>>(database)
        .await
        .optional()?;

    let current_vec = match current_vec_opt {
        Some(vec) => vec,
        None => return Err(Error::not_found("User not found")),
    };

    let post_slice = post_vector.as_slice();

    let updated_vector_data: Vec<f32> = match current_vec {
        None => {
            if weight > 0.0 {
                post_slice.to_vec()
            } else {
                return Ok(());
            }
        }
        Some(user_vector) => {
            let mut user_slice = user_vector.as_slice().to_vec();

            if user_slice.len() != post_slice.len() {
                return Err(Error::invalid_format("Vector dimension mismatch"));
            }

            let alpha = (0.05 * weight).clamp(-0.5, 0.5);
            for (u_val, p_val) in user_slice.iter_mut().zip(post_slice.iter()) {
                *u_val = (1.0 - alpha) * (*u_val) + alpha * p_val;
            }

            let magnitude: f32 = user_slice.iter().map(|v| v * v).sum::<f32>().sqrt();
            if magnitude > 0.0 {
                for val in user_slice.iter_mut() {
                    *val /= magnitude;
                }
            }

            user_slice
        }
    };

    let new_pg_vector = Vector::from(updated_vector_data);

    diesel::update(users::table.filter(users::user_id.eq(user_id)))
        .set(users::embedding.eq(Some(new_pg_vector)))
        .execute(database)
        .await?;

    Ok(())
}

pub struct FeedCandidate {
    pub post_id: String,
    pub created_at: DateTime<Utc>,
    pub social_weight: f32,
    pub vector_sim: f32,
    pub likes: i64,
    pub dislikes: i64,
    pub author_id: String,
}

pub struct ScoredPost {
    pub post_id: String,
    pub score: f32,
}

#[derive(Debug, PartialEq)]
pub enum PostInteraction {
    React,
    Comment,
}

impl PostInteraction {
    pub fn weight(&self) -> f32 {
        match self {
            PostInteraction::React => 0.5,
            PostInteraction::Comment => 0.8,
        }
    }
}
