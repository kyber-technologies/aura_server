use crate::database::DatabaseConnection;
use crate::database::posting as db;
use crate::embedder::TextEmbedder;
use crate::error::Error;
use crate::logic::feed::PostInteraction;
use crate::logic::{feed, user};
use crate::schema::{post_reactions, posts};
use crate::types::common::Timestamp;
use crate::types::posting::{Post, PostReaction};
use crate::types::{DatabaseDomainType, FastMap};
use crate::utils::escape_like_pattern;
use diesel::{
    ExpressionMethods, OptionalExtension, PgTextExpressionMethods, QueryDsl, SelectableHelper,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use pgvector::Vector;

pub async fn create(
    database: &mut DatabaseConnection,
    embedder: &mut TextEmbedder,
    post: Post,
) -> Result<Post, Error> {
    database
        .transaction(async |database| {
            let parent_vector: Option<Vector> = if let Some(ref parent_id) = post.parent {
                let parent_vec_opt: Option<Option<Vector>> = posts::table
                    .filter(posts::post_id.eq(&**parent_id))
                    .select(posts::embedding)
                    .first::<Option<Vector>>(database)
                    .await
                    .optional()?;

                match parent_vec_opt {
                    Some(vec) => vec,
                    None => return Err(Error::not_found("Parent post not found")),
                }
            } else {
                None
            };

            let vector = if let Some(text) = post.content.as_text() {
                embedder
                    .embed(&[text])?
                    .into_iter()
                    .next()
                    .ok_or(Error::internal("No embedding generated"))?
            } else {
                Vector::from(Vec::new())
            };

            let mut post_data = post.clone().into_db()?;
            post_data.post.embedding = Some(vector.clone());

            diesel::insert_into(posts::table)
                .values(&post_data.post)
                .execute(database)
                .await
                .map_err(|err| match err {
                    diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        _,
                    ) => Error::already_exists("Post ID already exists"),
                    err => err.into(),
                })?;

            if let Some(parent_vec) = parent_vector {
                if !parent_vec.as_slice().is_empty() {
                    feed::update_user_vector(
                        database,
                        &post.author_id,
                        &parent_vec,
                        PostInteraction::Comment,
                        false,
                    )
                    .await?;
                }
            } else if !vector.as_slice().is_empty() {
                feed::update_user_vector(
                    database,
                    &post.author_id,
                    &vector,
                    PostInteraction::React,
                    false,
                )
                .await?;
            }

            Ok(post)
        })
        .await
}

pub async fn delete(
    database: &mut DatabaseConnection,
    post_id: &str,
    user_id: &str,
) -> Result<(), Error> {
    let author = posts::table
        .find(post_id)
        .select(posts::author_id)
        .first::<String>(database)
        .await
        .optional()?
        .ok_or_else(|| Error::not_found("Post not found"))?;

    if author != user_id {
        return Err(Error::restricted(
            "Only the author of the post can delete it",
        ));
    }

    diesel::delete(posts::table.find(post_id))
        .execute(database)
        .await?;

    Ok(())
}

// TODO: Add way to fetch multiple.
pub async fn get(
    database: &mut DatabaseConnection,
    post_id: &str,
    requesting_user_id: Option<&str>,
) -> Result<Option<Post>, Error> {
    let post = posts::table
        .find(post_id)
        .select(db::Post::as_select())
        .first::<db::Post>(database)
        .await
        .optional()?;

    let Some(post) = post else {
        return Ok(None);
    };

    if let Some(uid) = requesting_user_id
        && user::is_blocked_by(database, uid, &post.author_id).await?
    {
        return Err(Error::unwanted("User blocked this post author"));
    }

    let raw_counts: Vec<(PostReaction, i64)> = post_reactions::table
        .filter(post_reactions::post_id.eq(post_id))
        .group_by(post_reactions::reaction)
        .select((post_reactions::reaction, diesel::dsl::count_star()))
        .load(database)
        .await?;

    let user_reaction = if let Some(uid) = requesting_user_id {
        post_reactions::table
            .filter(post_reactions::post_id.eq(post_id))
            .filter(post_reactions::user_id.eq(uid))
            .select(post_reactions::reaction)
            .first::<PostReaction>(database)
            .await
            .optional()?
            .unwrap_or(PostReaction::None)
    } else {
        PostReaction::None
    };

    let post_data = db::PostData {
        post,
        reaction_counts: raw_counts,
        user_reaction,
    };

    Ok(Some(Post::from_db(post_data)?))
}

// TODO: skip posts of blocked users
pub async fn get_of(
    database: &mut DatabaseConnection,
    author_id: &str,
    limit: u32,
    start_at: Timestamp,
    requesting_user_id: Option<&str>,
) -> Result<Vec<Post>, Error> {
    let raw_posts = posts::table
        .filter(posts::author_id.eq(author_id))
        .filter(posts::timestamp.lt(start_at.0))
        .order(posts::timestamp.desc())
        .limit(limit as i64)
        .select(db::Post::as_select())
        .load::<db::Post>(database)
        .await?;

    hydrate(database, raw_posts, requesting_user_id).await
}

// TODO: skip posts of blocked users
pub async fn search(
    database: &mut DatabaseConnection,
    query: &str,
    limit: u32,
    start_at: Timestamp,
    requesting_user_id: Option<&str>,
) -> Result<Vec<Post>, Error> {
    let pattern = escape_like_pattern(query);

    let raw_posts = posts::table
        .filter(
            posts::content
                .cast::<diesel::sql_types::Text>()
                .ilike(&pattern),
        )
        .filter(posts::timestamp.lt(start_at.0))
        .order(posts::timestamp.desc())
        .limit(limit as i64)
        .select(db::Post::as_select())
        .load::<db::Post>(database)
        .await?;

    hydrate(database, raw_posts, requesting_user_id).await
}

pub async fn react(
    database: &mut DatabaseConnection,
    post_id: &str,
    user_id: &str,
    reaction: PostReaction,
) -> Result<(), Error> {
    database
        .transaction(async |database| {
            let post_vector_opt: Option<Option<Vector>> = posts::table
                .filter(posts::post_id.eq(post_id))
                .select(posts::embedding)
                .first::<Option<Vector>>(database)
                .await
                .optional()?;

            let post_vector = match post_vector_opt {
                Some(vec) => vec,
                None => return Err(Error::not_found("Post not found")),
            };

            let previous_reaction: Option<PostReaction> = post_reactions::table
                .filter(post_reactions::post_id.eq(post_id))
                .filter(post_reactions::user_id.eq(user_id))
                .select(post_reactions::reaction)
                .first::<PostReaction>(database)
                .await
                .optional()?;

            if reaction == PostReaction::None {
                diesel::delete(
                    post_reactions::table
                        .filter(post_reactions::post_id.eq(post_id))
                        .filter(post_reactions::user_id.eq(user_id)),
                )
                .execute(database)
                .await?;
            } else {
                let reaction_row = db::PostReactionRow {
                    post_id: post_id.to_string(),
                    user_id: user_id.to_string(),
                    reaction,
                };

                diesel::insert_into(post_reactions::table)
                    .values(&reaction_row)
                    .on_conflict((post_reactions::post_id, post_reactions::user_id))
                    .do_update()
                    .set(post_reactions::reaction.eq(reaction))
                    .execute(database)
                    .await?;
            }

            if let Some(post_vec) = post_vector {
                let action = match reaction {
                    PostReaction::None => match previous_reaction.unwrap_or(PostReaction::None) {
                        PostReaction::None => None,
                        PostReaction::Like | PostReaction::Dislike => {
                            Some((PostInteraction::React, false))
                        }
                    },
                    PostReaction::Like | PostReaction::Dislike => {
                        match previous_reaction.unwrap_or(PostReaction::None) {
                            PostReaction::None => Some((PostInteraction::React, true)),
                            PostReaction::Like | PostReaction::Dislike => None,
                        }
                    }
                };

                if let Some((interaction, revert)) = action {
                    feed::update_user_vector(database, user_id, &post_vec, interaction, revert)
                        .await?;
                }
            }

            Ok(())
        })
        .await
}

async fn hydrate(
    database: &mut DatabaseConnection,
    raw_posts: Vec<db::Post>,
    requesting_user_id: Option<&str>,
) -> Result<Vec<Post>, Error> {
    if raw_posts.is_empty() {
        return Ok(Vec::new());
    }

    let post_ids: Vec<String> = raw_posts.iter().map(|p| p.post_id.clone()).collect();

    let raw_reactions: Vec<(String, PostReaction, i64)> = post_reactions::table
        .filter(post_reactions::post_id.eq_any(&post_ids))
        .group_by((post_reactions::post_id, post_reactions::reaction))
        .select((
            post_reactions::post_id,
            post_reactions::reaction,
            diesel::dsl::count_star(),
        ))
        .load(database)
        .await?;

    let user_reactions: FastMap<String, PostReaction> = if let Some(uid) = requesting_user_id {
        post_reactions::table
            .filter(post_reactions::post_id.eq_any(&post_ids))
            .filter(post_reactions::user_id.eq(uid))
            .select((post_reactions::post_id, post_reactions::reaction))
            .load::<(String, PostReaction)>(database)
            .await?
            .into_iter()
            .collect()
    } else {
        FastMap::default()
    };

    let mut counts_map: FastMap<String, Vec<(PostReaction, i64)>> = FastMap::default();
    for (pid, reaction, count) in raw_reactions {
        counts_map.entry(pid).or_default().push((reaction, count));
    }

    let mut domain_posts = Vec::with_capacity(raw_posts.len());
    for raw_post in raw_posts {
        let pid = raw_post.post_id.clone();
        let reaction_counts = counts_map.remove(&pid).unwrap_or_default();
        let user_reaction = user_reactions
            .get(&pid)
            .copied()
            .unwrap_or(PostReaction::None);

        let post_data = db::PostData {
            post: raw_post,
            reaction_counts,
            user_reaction,
        };

        domain_posts.push(Post::from_db(post_data)?);
    }

    Ok(domain_posts)
}
