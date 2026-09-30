use crate::database::posting as db;
use crate::error::Error;
use crate::logic::feed::PostInteraction;
use crate::logic::{feed, user};
use crate::schema::{post_reactions, posts, user_blocks};
use crate::state::database::DatabaseConnection;
use crate::state::embedder::TextEmbedder;
use crate::types::common::Timestamp;
use crate::types::posting::{Post, PostReaction};
use crate::types::user::UserSettings;
use crate::types::{DatabaseDomainType, FastMap, FastSet, UniqueId};
use crate::utils;
use crate::utils::escape_like_pattern;
use diesel::{
    ExpressionMethods, OptionalExtension, PgTextExpressionMethods, QueryDsl, SelectableHelper,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use pgvector::Vector;

pub async fn create(
    database: &mut DatabaseConnection,
    embedder: &TextEmbedder,
    post: Post,
    settings: &UserSettings,
) -> Result<Post, Error> {
    database
        .transaction(async |database| {
            let parent_vector: Option<Vector> = if let Some(ref parent_id) = post.parent {
                let parent_vec_opt: Option<Option<Vector>> = posts::table
                    .filter(posts::post_id.eq(*parent_id as UniqueId))
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
                    .embed(vec![text.to_string()])
                    .await?
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
                        settings,
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
                    settings,
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
    post_id: UniqueId,
    user_id: &str,
) -> Result<(), Error> {
    let author = posts::table
        .find(post_id)
        .select(posts::author_id)
        .first::<String>(database)
        .await
        .optional()?
        .ok_or(Error::not_found("Post not found"))?;

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

pub async fn get(
    database: &mut DatabaseConnection,
    post_ids: &[UniqueId],
    requesting_user_id: Option<&str>,
) -> Result<FastMap<UniqueId, Post>, Error> {
    utils::validate_item_length(post_ids.len() as u32)?;

    if post_ids.is_empty() {
        return Ok(FastMap::default());
    }

    let db_posts: Vec<db::Post> = posts::table
        .filter(posts::post_id.eq_any(post_ids))
        .select(db::Post::as_select())
        .load::<db::Post>(database)
        .await?;

    if db_posts.is_empty() {
        return Ok(FastMap::default());
    }

    if let Some(uid) = requesting_user_id {
        let distinct_author_ids: Vec<&str> = db_posts
            .iter()
            .map(|p| p.author_id.as_str())
            .collect::<FastSet<_>>()
            .into_iter()
            .collect();

        for author_id in distinct_author_ids {
            if user::is_blocked_by(database, uid, author_id).await? {
                return Err(Error::unwanted("User blocked this post author"));
            }
        }
    }

    let found_post_ids: Vec<UniqueId> = db_posts.iter().map(|p| p.post_id).collect();

    let raw_counts: Vec<(UniqueId, PostReaction, i64)> = post_reactions::table
        .filter(post_reactions::post_id.eq_any(&found_post_ids))
        .group_by((post_reactions::post_id, post_reactions::reaction))
        .select((
            post_reactions::post_id,
            post_reactions::reaction,
            diesel::dsl::count_star(),
        ))
        .load(database)
        .await?;

    let mut reaction_counts_map: FastMap<UniqueId, Vec<(PostReaction, i64)>> = FastMap::default();
    for (pid, reaction, count) in raw_counts {
        reaction_counts_map
            .entry(pid)
            .or_default()
            .push((reaction, count));
    }

    let mut user_reactions_map: FastMap<UniqueId, PostReaction> = FastMap::default();
    if let Some(uid) = requesting_user_id {
        let user_reactions: Vec<(UniqueId, PostReaction)> = post_reactions::table
            .filter(post_reactions::post_id.eq_any(&found_post_ids))
            .filter(post_reactions::user_id.eq(uid))
            .select((post_reactions::post_id, post_reactions::reaction))
            .load(database)
            .await?;

        for (pid, reaction) in user_reactions {
            user_reactions_map.insert(pid, reaction);
        }
    }

    let raw_comments: Vec<(Option<UniqueId>, UniqueId)> = posts::table
        .filter(posts::parent_id.eq_any(&found_post_ids))
        .select((posts::parent_id, posts::post_id))
        .load(database)
        .await?;

    let mut comments_map: FastMap<UniqueId, Vec<UniqueId>> = FastMap::default();
    for (parent_id, comment_id) in raw_comments {
        if let Some(pid) = parent_id {
            comments_map.entry(pid).or_default().push(comment_id);
        }
    }

    let mut result: FastMap<UniqueId, Post> = FastMap::default();
    result.reserve(db_posts.len());

    for post in db_posts {
        let pid = post.post_id;
        let counts = reaction_counts_map.remove(&pid).unwrap_or_default();
        let user_reaction = user_reactions_map
            .remove(&pid)
            .unwrap_or(PostReaction::None);
        let comments = comments_map.remove(&pid).unwrap_or_default();

        let post_data = db::PostData {
            post,
            reaction_counts: counts,
            user_reaction,
            comments,
        };

        result.insert(pid, Post::from_db(post_data)?);
    }

    Ok(result)
}

pub async fn get_of(
    database: &mut DatabaseConnection,
    author_id: &str,
    limit: u32,
    start_at: Timestamp,
    requesting_user_id: Option<&str>,
) -> Result<Vec<Post>, Error> {
    utils::validate_item_length(limit)?;

    let mut query = posts::table
        .into_boxed()
        .filter(posts::author_id.eq(author_id))
        .filter(posts::timestamp.lt(start_at.0));

    if let Some(uid) = requesting_user_id {
        let blocked_subquery = user_blocks::table
            .filter(user_blocks::user_id.eq(uid))
            .select(user_blocks::blocked_user_id);

        query = query.filter(posts::author_id.ne_all(blocked_subquery));
    }

    let raw_posts = query
        .order(posts::timestamp.desc())
        .limit(limit as i64)
        .select(db::Post::as_select())
        .load::<db::Post>(database)
        .await?;

    hydrate(database, raw_posts, requesting_user_id).await
}

pub async fn get_author(
    database: &mut DatabaseConnection,
    post_id: UniqueId,
) -> Result<Option<String>, Error> {
    posts::table
        .filter(posts::post_id.eq(post_id))
        .select(posts::author_id)
        .first::<String>(database)
        .await
        .optional()
        .map_err(Into::into)
}

pub async fn search(
    database: &mut DatabaseConnection,
    query_str: &str,
    limit: u32,
    start_at: Timestamp,
    requesting_user_id: Option<&str>,
) -> Result<Vec<Post>, Error> {
    utils::validate_item_length(limit)?;

    let pattern = escape_like_pattern(query_str);

    let mut query = posts::table
        .into_boxed()
        .filter(
            posts::content
                .cast::<diesel::sql_types::Text>()
                .ilike(&pattern),
        )
        .filter(posts::timestamp.lt(start_at.0));

    if let Some(uid) = requesting_user_id {
        let blocked_subquery = user_blocks::table
            .filter(user_blocks::user_id.eq(uid))
            .select(user_blocks::blocked_user_id);

        query = query.filter(posts::author_id.ne_all(blocked_subquery));
    }

    let raw_posts = query
        .order(posts::timestamp.desc())
        .limit(limit as i64)
        .select(db::Post::as_select())
        .load::<db::Post>(database)
        .await?;

    hydrate(database, raw_posts, requesting_user_id).await
}

pub async fn react(
    database: &mut DatabaseConnection,
    post_id: UniqueId,
    user_id: &str,
    reaction: PostReaction,
    settings: &UserSettings,
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
                    post_id,
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
                    feed::update_user_vector(
                        database,
                        user_id,
                        &post_vec,
                        interaction,
                        settings,
                        revert,
                    )
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

    let post_ids: Vec<UniqueId> = raw_posts.iter().map(|p| p.post_id).collect();

    let raw_reactions: Vec<(UniqueId, PostReaction, i64)> = post_reactions::table
        .filter(post_reactions::post_id.eq_any(&post_ids))
        .group_by((post_reactions::post_id, post_reactions::reaction))
        .select((
            post_reactions::post_id,
            post_reactions::reaction,
            diesel::dsl::count_star(),
        ))
        .load(database)
        .await?;

    let mut counts_map: FastMap<UniqueId, Vec<(PostReaction, i64)>> = FastMap::default();
    for (pid, reaction, count) in raw_reactions {
        counts_map.entry(pid).or_default().push((reaction, count));
    }

    let user_reactions: FastMap<UniqueId, PostReaction> = if let Some(uid) = requesting_user_id {
        post_reactions::table
            .filter(post_reactions::post_id.eq_any(&post_ids))
            .filter(post_reactions::user_id.eq(uid))
            .select((post_reactions::post_id, post_reactions::reaction))
            .load::<(UniqueId, PostReaction)>(database)
            .await?
            .into_iter()
            .collect()
    } else {
        FastMap::default()
    };

    let raw_comments: Vec<(Option<UniqueId>, UniqueId)> = posts::table
        .filter(posts::parent_id.eq_any(&post_ids))
        .select((posts::parent_id, posts::post_id))
        .load(database)
        .await?;

    let mut comments_map: FastMap<UniqueId, Vec<UniqueId>> = FastMap::default();
    for (parent_id, comment_id) in raw_comments {
        if let Some(pid) = parent_id {
            comments_map.entry(pid).or_default().push(comment_id);
        }
    }

    let mut domain_posts = Vec::with_capacity(raw_posts.len());
    for raw_post in raw_posts {
        let pid = raw_post.post_id;
        let reaction_counts = counts_map.remove(&pid).unwrap_or_default();
        let user_reaction = user_reactions
            .get(&pid)
            .copied()
            .unwrap_or(PostReaction::None);
        let comments = comments_map.remove(&pid).unwrap_or_default();

        let post_data = db::PostData {
            post: raw_post,
            reaction_counts,
            user_reaction,
            comments,
        };

        domain_posts.push(Post::from_db(post_data)?);
    }

    Ok(domain_posts)
}
