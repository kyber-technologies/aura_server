use crate::database::DatabaseConnection;
use crate::database::channel as ch_db;
use crate::database::user as db;
use crate::error::Error;
use crate::logic::resource;
use crate::schema::{channel_members, channels, user_blocks, users};
use crate::types::DatabaseDomainType;
use crate::types::common::Timestamp;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceMeta, ResourceNamespace};
use crate::types::user::{Notification, Notifications, User, UserProfile, UserRole};
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
        return Err(Error::new(ErrorCode::InvalidFormat, "Invalid user ID"));
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
                Some("users_email_unique") => {
                    Error::new(ErrorCode::AlreadyExists, "Email already registered")
                }

                Some("users_user_id_unique") => {
                    Error::new(ErrorCode::AlreadyExists, "User ID already exists")
                }

                _ => Error::new(ErrorCode::Internal, "Database unique constraint violation"),
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

pub async fn get(database: &mut DatabaseConnection, user_id: &str) -> Result<Option<User>, Error> {
    let user = users::table
        .find(user_id)
        .select(db::User::as_select())
        .first::<db::User>(database)
        .await
        .optional()?;

    let Some(user) = user else {
        return Ok(None);
    };

    let channel_rows = channels::table
        .inner_join(channel_members::table.on(channel_members::channel_id.eq(channels::channel_id)))
        .filter(channel_members::user_id.eq(user_id))
        .select(ch_db::Channel::as_select())
        .load::<ch_db::Channel>(database)
        .await?;

    let mut channel_data = Vec::with_capacity(channel_rows.len());

    for channel in channel_rows {
        let members = channel_members::table
            .filter(channel_members::channel_id.eq(&channel.channel_id))
            .select(ch_db::ChannelMember::as_select())
            .load::<ch_db::ChannelMember>(database)
            .await?;

        channel_data.push(ch_db::ChannelData { channel, members });
    }

    let data = db::UserData {
        user,
        channels: channel_data,
    };

    Ok(Some(User::from_db(data)?))
}

pub async fn search(
    database: &mut DatabaseConnection,
    query: String,
) -> Result<Vec<UserProfile>, Error> {
    let config = config::get();

    // TODO: Investigate into escape characters
    let pattern = format!("%{}%", query);

    let results = users::table
        .filter(
            users::user_id
                .ilike(&pattern)
                .or(users::username.ilike(&pattern)),
        )
        .select((
            users::user_id,
            users::username,
            users::role,
            users::icon,
            users::created_at,
        ))
        .limit(config.service.max_search_results)
        .load::<(
            String,
            String,
            UserRole,
            serde_json::Value,
            chrono::DateTime<chrono::Utc>,
        )>(database)
        .await?;

    results
        .into_iter()
        .map(|(user_id, username, role, icon, created_at)| {
            Ok(UserProfile {
                user_id,
                username,
                role,
                icon: ResourceId::from_db(icon)?,
                created_at: Timestamp(created_at),
            })
        })
        .collect()
}

pub async fn block(
    database: &mut DatabaseConnection,
    user_id: &str,
    block_user_id: &str,
    block: bool,
) -> Result<(), Error> {
    if !exists(database, block_user_id).await? {
        return Err(Error::new(ErrorCode::NotFound, "User not found"));
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
        return Err(Error::new(ErrorCode::NotFound, "User not found"));
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
        .ok_or(Error::new(ErrorCode::NotFound, "User not found"))?;

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
            },
        )
        .await?;

        tracing::info!(
            "Created setup administrator 'admin' with password 'admin'. Please change this immediately!"
        );
    }

    Ok(())
}
