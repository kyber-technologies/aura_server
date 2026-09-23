use crate::database::DatabaseConnection;
use crate::database::channel as ch_db;
use crate::database::user as db;
use crate::error::Error;
use crate::logic::resource;
use crate::schema::{channel_members, channels, user_blocks, user_follows, users};
use crate::types::common::Timestamp;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceMeta, ResourceNamespace};
use crate::types::user::{Notification, Notifications, User, UserProfile, UserRole};
use crate::types::{DatabaseDomainType, FastMap};
use crate::utils::escape_like_pattern;
use crate::{auth, config, utils};
use aura_rust::common::v1::ErrorCode;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, OptionalExtension,
    PgTextExpressionMethods, QueryDsl, SelectableHelper,
};
use diesel_async::RunQueryDsl;
use tonic::codegen::tokio_stream::StreamExt;

pub async fn create(database: &mut DatabaseConnection, user: User) -> Result<(), Error> {
    if !utils::is_valid_ident(&user.user_id) {
        return Err(Error::invalid_format("Invalid User ID"));
    }

    let data = user.clone().into_db()?;

    diesel::insert_into(users::table)
        .values(&data.user)
        .execute(database)
        .await
        .map_err(|err| match err {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                info,
            ) => match info.constraint_name() {
                Some("users_email_unique") => Error::already_exists("Email already registered"),

                Some("users_user_id_unique") => Error::already_exists("User ID already exists"),

                _ => Error::already_exists("Database unique constraint violation"),
            },
            err => err.into(),
        })?;

    let mut icon_stream = resource::read(ResourceId::default_user_icon()).await?;

    let mut chunks = Vec::new();
    let mut length = 0;

    while let Some(chunk) = icon_stream.next().await {
        let chunk = chunk?;
        length += chunk.len();
        chunks.push(Ok(chunk));
    }

    let icon_id = ResourceId {
        namespace: ResourceNamespace::UserIcon,
        key: user.user_id.clone(),
    };

    resource::create(
        database,
        ResourceDescriptor {
            resource_id: icon_id.clone(),
            meta: ResourceMeta {
                size: length as u32,
                name: "icon".to_string(),
                timestamp: Timestamp::now(),
                metadata: Default::default(),
            },
            user_id: user.user_id.clone(),
        },
    )
    .await?;

    resource::write(icon_id, tokio_stream::iter(chunks)).await?;

    Ok(())
}

pub async fn delete(database: &mut DatabaseConnection, user_id: &str) -> Result<(), Error> {
    diesel::delete(users::table.find(user_id))
        .execute(database)
        .await?;

    Ok(())
}

pub async fn update(database: &mut DatabaseConnection, user: User) -> Result<(), Error> {
    let icon = user.icon.into_db()?;
    let notifications = user.notifications.into_db()?;

    diesel::update(users::table.find(&user.user_id))
        .set((
            users::username.eq(&user.username),
            users::email.eq(&user.email),
            users::password.eq(&user.password),
            users::role.eq(&user.role),
            users::icon.eq(icon),
            users::notifications.eq(notifications),
        ))
        .execute(database)
        .await?;

    Ok(())
}

pub async fn get(
    database: &mut DatabaseConnection,
    user_ids: &[String],
) -> Result<Vec<User>, Error> {
    utils::validate_item_length(user_ids.len() as u32)?;

    if user_ids.is_empty() {
        return Ok(Vec::new());
    }

    let user_rows = users::table
        .filter(users::user_id.eq_any(user_ids))
        .select(db::User::as_select())
        .load::<db::User>(database)
        .await?;

    if user_rows.is_empty() {
        return Ok(Vec::new());
    }

    let found_user_ids: Vec<&str> = user_rows.iter().map(|u| u.user_id.as_str()).collect();

    let user_channel_tuples = channels::table
        .inner_join(channel_members::table.on(channel_members::channel_id.eq(channels::channel_id)))
        .filter(channel_members::user_id.eq_any(&found_user_ids))
        .select((channel_members::user_id, ch_db::Channel::as_select()))
        .load::<(String, ch_db::Channel)>(database)
        .await?;

    let all_channel_ids: Vec<String> = user_channel_tuples
        .iter()
        .map(|(_, ch)| ch.channel_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let channel_members_map = if !all_channel_ids.is_empty() {
        let members = channel_members::table
            .filter(channel_members::channel_id.eq_any(&all_channel_ids))
            .select(ch_db::ChannelMember::as_select())
            .load::<ch_db::ChannelMember>(database)
            .await?;

        let mut map: FastMap<String, Vec<ch_db::ChannelMember>> = FastMap::default();
        for member in members {
            map.entry(member.channel_id.clone())
                .or_default()
                .push(member);
        }
        map
    } else {
        FastMap::default()
    };

    let mut user_channels_map: FastMap<String, Vec<ch_db::ChannelData>> = FastMap::default();
    for (uid, channel) in user_channel_tuples {
        let members = channel_members_map
            .get(&channel.channel_id)
            .cloned()
            .unwrap_or_default();

        user_channels_map
            .entry(uid)
            .or_default()
            .push(ch_db::ChannelData { channel, members });
    }

    let follower_tuples = user_follows::table
        .filter(user_follows::followed_id.eq_any(&found_user_ids))
        .select((user_follows::followed_id, user_follows::follower_id))
        .load::<(String, String)>(database)
        .await?;

    let mut followers_map: FastMap<String, Vec<String>> = FastMap::default();
    for (followed, follower) in follower_tuples {
        followers_map.entry(followed).or_default().push(follower);
    }

    let following_tuples = user_follows::table
        .filter(user_follows::follower_id.eq_any(&found_user_ids))
        .select((user_follows::follower_id, user_follows::followed_id))
        .load::<(String, String)>(database)
        .await?;

    let mut following_map: FastMap<String, Vec<String>> = FastMap::default();
    for (follower, followed) in following_tuples {
        following_map.entry(follower).or_default().push(followed);
    }

    let mut result = Vec::with_capacity(user_rows.len());

    for user in user_rows {
        let uid = user.user_id.clone();

        let data = db::UserData {
            user,
            channels: user_channels_map.remove(&uid).unwrap_or_default(),
            followers: followers_map.remove(&uid).unwrap_or_default(),
            following: following_map.remove(&uid).unwrap_or_default(),
        };

        result.push(User::from_db(data)?);
    }

    Ok(result)
}

pub async fn search(
    database: &mut DatabaseConnection,
    query: &str,
    limit: i64,
) -> Result<Vec<UserProfile>, Error> {
    utils::validate_item_length(limit as u32)?;

    let pattern = escape_like_pattern(query);

    let results = users::table
        .filter(
            users::user_id
                .ilike(&pattern)
                .or(users::username.ilike(&pattern)),
        )
        .left_join(user_follows::table.on(user_follows::followed_id.eq(users::user_id)))
        .select((
            users::user_id,
            users::username,
            users::role,
            users::icon,
            users::created_at,
        ))
        .limit(limit)
        .load::<(
            String,
            String,
            UserRole,
            serde_json::Value,
            chrono::DateTime<chrono::Utc>,
        )>(database)
        .await?;

    let mut profiles = Vec::with_capacity(results.len());

    for (user_id, username, role, icon, created_at) in results {
        let followers_count = user_follows::table
            .filter(user_follows::followed_id.eq(&user_id))
            .count()
            .get_result::<i64>(database)
            .await? as u32;

        let following_count = user_follows::table
            .filter(user_follows::follower_id.eq(&user_id))
            .count()
            .get_result::<i64>(database)
            .await? as u32;

        profiles.push(UserProfile {
            user_id,
            username,
            role,
            icon: ResourceId::from_db(icon)?,
            created_at: Timestamp(created_at),
            followers: followers_count,
            following: following_count,
        });
    }

    Ok(profiles)
}

pub async fn block(
    database: &mut DatabaseConnection,
    user_id: &str,
    block_user_id: &str,
    block: bool,
) -> Result<(), Error> {
    if !exists(database, block_user_id).await? {
        return Err(Error::not_found("User not found"));
    }

    if block {
        let data = db::UserBlock {
            user_id: user_id.to_owned(),
            blocked_user_id: block_user_id.to_owned(),
        };

        diesel::insert_into(user_blocks::table)
            .values(&data)
            .on_conflict_do_nothing()
            .execute(database)
            .await?;
    } else {
        diesel::delete(
            user_blocks::table
                .filter(user_blocks::user_id.eq(user_id))
                .filter(user_blocks::blocked_user_id.eq(block_user_id)),
        )
        .execute(database)
        .await?;
    }

    Ok(())
}

pub async fn is_blocked_by(
    database: &mut DatabaseConnection,
    user_id: &str,
    block_user_id: &str,
) -> Result<bool, Error> {
    if !exists(database, block_user_id).await? {
        return Err(Error::not_found("User not found"));
    }

    Ok(user_blocks::table
        .filter(user_blocks::user_id.eq(user_id))
        .filter(user_blocks::blocked_user_id.eq(block_user_id))
        .select(user_blocks::user_id)
        .first::<String>(database)
        .await
        .optional()?
        .is_some())
}

pub async fn push_notifications(
    database: &mut DatabaseConnection,
    user_id: &str,
    new_notifications: impl IntoIterator<Item = Notification>,
) -> Result<(), Error> {
    let config = config::get();
    let now = Timestamp::now();

    let existing = users::table
        .find(user_id)
        .select(users::notifications)
        .first::<serde_json::Value>(database)
        .await
        .optional()?
        .ok_or(Error::not_found("User not found"))?;

    let mut notifications = Notifications::from_db(existing)?;

    notifications.0.extend(new_notifications);

    notifications.0.retain(|notification| {
        now.0.timestamp_millis() - notification.timestamp().0.timestamp_millis()
            < config.service.notification_expiration_time * 60 * 60 * 1000
    });

    let notifications = notifications.into_db()?;

    diesel::update(users::table.find(user_id))
        .set(users::notifications.eq(notifications))
        .execute(database)
        .await?;

    Ok(())
}

pub async fn exists(database: &mut DatabaseConnection, user_id: &str) -> Result<bool, Error> {
    Ok(users::table
        .find(user_id)
        .select(users::user_id)
        .first::<String>(database)
        .await
        .optional()?
        .is_some())
}

pub async fn follow(
    database: &mut DatabaseConnection,
    follower_id: &str,
    followed_id: &str,
) -> Result<(), Error> {
    if follower_id == followed_id {
        return Err(Error::invalid_format("You cannot follow yourself"));
    }

    let target_exists = users::table
        .find(followed_id)
        .select(users::user_id)
        .first::<String>(database)
        .await
        .optional()?
        .is_some();

    if !target_exists {
        return Err(Error::not_found("User to follow not found"));
    }

    let follow_row = db::UserFollow {
        follower_id: follower_id.to_string(),
        followed_id: followed_id.to_string(),
        created_at: chrono::Utc::now(),
    };

    diesel::insert_into(user_follows::table)
        .values(&follow_row)
        .on_conflict((user_follows::follower_id, user_follows::followed_id))
        .do_nothing()
        .execute(database)
        .await?;

    Ok(())
}

pub async fn unfollow(
    database: &mut DatabaseConnection,
    follower_id: &str,
    followed_id: &str,
) -> Result<(), Error> {
    diesel::delete(
        user_follows::table
            .filter(user_follows::follower_id.eq(follower_id))
            .filter(user_follows::followed_id.eq(followed_id)),
    )
    .execute(database)
    .await?;

    Ok(())
}

pub async fn create_admin(database: &mut DatabaseConnection) -> Result<(), Error> {
    if exists(database, "admin").await? {
        match auth::auth(database, "admin".to_string(), "admin".to_string()).await {
            Ok(_) => tracing::warn!(
                "The initial 'admin' user has an unsecure password. Please change this immediately!"
            ),

            Err(err) => {
                if err.code != ErrorCode::Unauthorized {
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
                role: UserRole::Admin,
                created_at: Timestamp::now(),
                icon: ResourceId {
                    namespace: ResourceNamespace::UserIcon,
                    key: "admin".to_string(),
                },
                notifications: Notifications(Vec::new()),
                channels: Vec::new(),
                followers: Vec::new(),
                following: Vec::new(),
            },
        )
        .await?;

        tracing::info!(
            "Created setup administrator 'admin' with password 'admin'. Please change this immediately!"
        );
    }

    Ok(())
}
