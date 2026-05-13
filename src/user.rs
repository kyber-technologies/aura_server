use crate::database::Database;
use crate::error::Error;
use crate::resource::ResourceDescriptor;
use crate::utils::VecStream;
use crate::{auth, config, resource, utils};
use aura_rust::common::v1::ErrorCode;
use aura_rust::user::v1::{UserProfile, UserRole};
use aura_rust::{ResourceMeta, User};
use tonic::codegen::tokio_stream::StreamExt;

pub async fn create(database: &Database, user: User) -> Result<(), Error> {
    if exists(database, user.user_id.as_str()).await? {
        return Err(Error::new(ErrorCode::AlreadyExists, "User already exists"));
    }

    // Read and write default user icon
    {
        let mut icon_stream = resource::read(aura_rust::DEFAULT_USER_ICON.clone()).await?;

        let mut chunks: Vec<Result<Vec<u8>, Error>> = Vec::new();
        let mut length = 0;

        while let Some(chunk) = icon_stream.next().await {
            let chunk = chunk?;
            length += chunk.len();
            chunks.push(Ok(chunk));
        }

        let icon_id = resource::build_user_avatar_id(&user.user_id);

        // Create user icon
        resource::create(
            database,
            ResourceDescriptor {
                resource_id: icon_id.clone(),
                meta: ResourceMeta {
                    size: length as i32,
                    timestamp: utils::get_timestamp(),
                    metadata: Default::default(),
                },
                user_id: user.user_id.clone(),
            },
        )
        .await?;

        // Upload default user icon
        resource::write(icon_id, VecStream::new(chunks)).await?;
    }

    let _: Option<User> = database
        .create(("user", user.user_id.as_str()))
        .content(user)
        .await?;

    Ok(())
}

pub async fn delete(database: &Database, userid: &str) -> Result<(), Error> {
    let _: Option<User> = database.delete(("user", userid)).await?;

    Ok(())
}

pub async fn update(database: &Database, user: User) -> Result<(), Error> {
    let _: Option<User> = database
        .update(("user", user.user_id.as_str()))
        .content(user)
        .await?;

    Ok(())
}

pub async fn get(database: &Database, userid: &str) -> Result<Option<User>, Error> {
    let result: Option<User> = database.select(("user", userid)).await?;

    Ok(result)
}

pub async fn search(database: &Database, query: String) -> Result<Vec<UserProfile>, Error> {
    let config = config::get();

    let results = database
        .query(
            r#"
        SELECT * FROM user
        WHERE user_id CONTAINS $query
           OR username CONTAINS $query
        LIMIT $limit
    "#,
        )
        .bind(("query", query))
        .bind(("limit", config.service_max_search_results))
        .await?
        .take::<Vec<User>>(0)?;

    Ok(results.into_iter().map(to_profile).collect())
}

pub async fn block(
    database: &Database,
    user_id: String,
    block_user_id: String,
    unblock: bool,
) -> Result<(), Error> {
    if !exists(database, &block_user_id).await? {
        return Err(Error::new(ErrorCode::NotFound, "User not found"));
    }

    if unblock {
        database
            .query(
                r#"DELETE blocked
            WHERE in = user:$user_id
            AND out = user:$block_user_id;"#,
            )
            .bind(("user_id", user_id))
            .bind(("block_user_id", block_user_id))
            .await?;
    } else {
        database
            .query("RELATE user:$user_id->blocked->user:$block_user_id")
            .bind(("user_id", user_id))
            .bind(("block_user_id", block_user_id))
            .await?;
    }

    Ok(())
}

pub async fn is_blocked_by(
    database: &Database,
    user_id: String,
    block_user_id: String,
) -> Result<bool, Error> {
    if !exists(database, &block_user_id).await? {
        return Err(Error::new(ErrorCode::NotFound, "User not found"));
    }

    let result: Option<()> = database
        .query(
            r#"SELECT * FROM blocked
        WHERE in = user:$user_id
        AND out = user:$block_user_id;"#,
        )
        .bind(("user_id", user_id))
        .bind(("block_user_id", block_user_id))
        .await?
        .take(0)?;

    Ok(result.is_some())
}

pub async fn exists(database: &Database, userid: &str) -> Result<bool, Error> {
    Ok(get(database, userid).await?.is_some())
}

pub fn to_profile(user: User) -> UserProfile {
    UserProfile {
        user_id: user.user_id,
        username: user.username,
        role: user.role,
        icon: Some(user.icon.try_into().unwrap()),
    }
}

pub async fn create_admin(database: &Database) -> Result<(), Error> {
    if exists(database, "admin").await? {
        match auth::auth(database, "admin".to_string(), "admin".to_string()).await {
            Ok(_) => tracing::warn!(
                "The initial 'admin' user has an unsecure password. Please change this immediately!"
            ),

            Err(err) => {
                if err.code() != ErrorCode::Unauthorized {
                    return Err(err);
                }
            }
        }
    } else {
        create(
            database,
            User {
                user_id: "admin".to_string(),
                username: "admin".to_string(),
                email: "".to_string(),
                password: auth::hash("admin".to_string()).expect("Failed to hash password"),
                role: UserRole::Admin as i32,
                icon: resource::build_user_avatar_id("admin"),
            },
        )
        .await?;

        tracing::info!(
            "Created setup administrator 'admin' with password 'admin'. Please change this immediately!"
        );
    }

    Ok(())
}
