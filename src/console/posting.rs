use crate::console::{Command, CommandError};
use crate::error::Error;
use crate::logic::{feed, posting};
use crate::state::ServerState;
use crate::types::FastMap;
use crate::types::common::Timestamp;
use crate::types::posting::{Post, PostReaction};
use crate::types::resource::Content;
use crate::utils::generate_unique_id;
use chrono::DateTime;
use no_pico_args::Arguments;
use std::pin::Pin;
use std::slice;

pub const PUBLISH: Command = Command {
    name: "publish",
    description: "Publish a post",
    usage: "publish [--parent | -p <parent_id> (optional parent post ID)] <user_id> <content>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let content: String = args.free_from_str()?;
            let parent: Option<String> = args.opt_value_from_str(["--parent", "-p"])?;

            let mut database = state.database().await?;

            posting::create(
                &mut database,
                state.embedder(),
                Post {
                    post_id: generate_unique_id(),
                    author_id: user_id,
                    content: Content::Text(content),
                    timestamp: Timestamp::now(),
                    parent,
                    reactions: FastMap::default(),
                    reaction: PostReaction::None,
                },
            )
            .await?;

            Ok(())
        })
    },
};

pub const UNPUBLISH: Command = Command {
    name: "unpublish",
    description: "Unpublish/delete a post",
    usage: "unpublish <post_id>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let post_id: String = args.free_from_str()?;

            let mut database = state.database().await?;

            let (_, post) = posting::get(&mut database, slice::from_ref(&post_id), None)
                .await?
                .into_iter()
                .next()
                .ok_or(Error::not_found("Post not found"))?;

            posting::delete(&mut database, &post_id, &post.author_id).await?;

            tracing::info!("Unpublished post '{post_id}'.");

            Ok(())
        })
    },
};

pub const GET: Command = Command {
    name: "get-post",
    description: "Get a post",
    usage: "get-post <post_id>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let post_id: String = args.free_from_str()?;

            let mut database = state.database().await?;

            let post = posting::get(&mut database, &[post_id], None)
                .await?
                .into_iter()
                .next()
                .ok_or(Error::not_found("Post not found"))?;

            tracing::info!("Found Post: {post:#?}");

            Ok(())
        })
    },
};

pub const GET_OF: Command = Command {
    name: "get-posts-of",
    description: "Get posts of a user with limit and start date (RFC3339)",
    usage: "get-posts-of <user_id> <limit> <start_at>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let limit: u32 = args.free_from_str()?;
            let start_at: Timestamp = args.free_from_fn::<Timestamp, CommandError>(|s| {
                Ok(Timestamp(
                    DateTime::parse_from_rfc3339(s)
                        .map_err(|err| CommandError::Other(err.to_string()))?
                        .to_utc(),
                ))
            })?;

            let mut database = state.database().await?;

            let post = posting::get_of(&mut database, &user_id, limit, start_at, None).await?;

            tracing::info!("Found Posts: {post:#?}");

            Ok(())
        })
    },
};

pub const SEARCH: Command = Command {
    name: "search-posts",
    description: "Search posts with limit and start date (RFC3339)",
    usage: "search-posts <query> <limit> <start_at>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let query: String = args.free_from_str()?;
            let limit: u32 = args.free_from_str()?;
            let start_at: Timestamp = args.free_from_fn::<Timestamp, CommandError>(|s| {
                Ok(Timestamp(
                    DateTime::parse_from_rfc3339(s)
                        .map_err(|err| CommandError::Other(err.to_string()))?
                        .to_utc(),
                ))
            })?;

            let mut database = state.database().await?;

            let post = posting::search(&mut database, &query, limit, start_at, None).await?;

            tracing::info!("Found Posts: {post:#?}");

            Ok(())
        })
    },
};

pub const REACT: Command = Command {
    name: "react-to-post",
    description: "React to a post as a user",
    usage: "react-to-post <user_id> <post_id> <reaction ('like' or 'dislike' or 'none')>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let post_id: String = args.free_from_str()?;
            let reaction: PostReaction = args.free_from_fn(|s| match s {
                "like" => Ok(PostReaction::Like),
                "dislike" => Ok(PostReaction::Dislike),
                "none" => Ok(PostReaction::None),
                _ => Err(Error::invalid_format(
                    "Invalid reaction. Valid: 'like', 'dislike', 'none'.",
                )),
            })?;

            let mut database = state.database().await?;

            posting::react(&mut database, &post_id, &user_id, reaction).await?;

            Ok(())
        })
    },
};

pub const FEED: Command = Command {
    name: "feed",
    description: "Get the feed of a user with limit",
    usage: "feed [--fetch | -f (fetch and print the posts)] <user_id> <limit>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let limit: usize = args.free_from_str()?;
            let fetch = args.contains(["--fetch", "-f"]);

            let mut database = state.database().await?;

            let vector = feed::fetch_user_vector(&mut database, &user_id)
                .await?
                .ok_or(Error::not_found("User not found"))?;
            let post_ids = feed::fetch_feed(&mut database, &user_id, Some(vector), limit).await?;

            if fetch {
                let mut posts = Vec::with_capacity(post_ids.len());

                for id in post_ids {
                    posts.push(
                        posting::get(&mut database, &[id], Some(&user_id))
                            .await?
                            .into_iter()
                            .next()
                            .ok_or(Error::not_found("Post not found"))?,
                    );
                }

                tracing::info!("User Feed: {posts:#?}");
            } else {
                tracing::info!("User Feed: {post_ids:#?}");
            }

            Ok(())
        })
    },
};
