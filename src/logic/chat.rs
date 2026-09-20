use crate::database::DatabaseConnection;
use crate::database::channel as ch_db;
use crate::database::message as msg_db;
use crate::error::Error;
use crate::logic::user;
use crate::logic::user::push_notifications;
use crate::schema::{channel_members, channels, messages};
use crate::types::DatabaseDomainType;
use crate::types::chat::{Channel, ChannelPermission, Message};
use crate::types::common::Timestamp;
use crate::types::user::Notification;
use crate::utils::generate_unique_id;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl};

pub async fn create_channel(
    database: &mut DatabaseConnection,
    mut channel: Channel,
    owner: String,
) -> Result<Channel, Error> {
    database
        .transaction(async |database| {
            channel
                .members
                .insert(owner.clone(), ChannelPermission::Manager);

            let channel_data = ch_db::Channel {
                channel_id: channel.channel_id.clone(),
                name: channel.name.clone(),
                description: channel.description.clone(),
            };

            diesel::insert_into(channels::table)
                .values(&channel_data)
                .execute(database)
                .await?;

            let members = channel
                .members
                .iter()
                .map(|(user_id, permission)| ch_db::ChannelMember {
                    channel_id: channel.channel_id.clone(),
                    user_id: user_id.clone(),
                    permission: *permission,
                })
                .collect::<Vec<_>>();

            if !members.is_empty() {
                diesel::insert_into(channel_members::table)
                    .values(&members)
                    .execute(database)
                    .await?;
            }

            for user_id in channel.members.keys() {
                if user_id == &owner {
                    continue;
                }

                push_notifications(
                    database,
                    user_id,
                    [Notification::Invite {
                        notification_id: generate_unique_id(),
                        timestamp: Timestamp::now(),
                        channel_id: channel.channel_id.clone(),
                        invited_by: owner.clone(),
                        uninvited: false,
                    }],
                )
                .await?;
            }

            Ok::<Channel, Error>(channel)
        })
        .await
}

pub async fn delete_channel(
    database: &mut DatabaseConnection,
    channel_id: String,
    user_id: String,
) -> Result<(), Error> {
    if !channel_exists(database, &channel_id).await? {
        return Err(Error::not_found("Channel not found"));
    }

    let permission = get_channel_member_perm(database, &channel_id, &user_id).await?;

    if permission != ChannelPermission::Manager {
        return Err(Error::restricted(
            "Only channel managers can delete a channel",
        ));
    }

    let deleted = diesel::delete(channels::table.find(channel_id))
        .execute(database)
        .await?;

    if deleted == 0 {
        return Err(Error::not_found("Channel not found"));
    }

    Ok(())
}

pub async fn invite(
    database: &mut DatabaseConnection,
    channel_id: String,
    user_id: String,
    invited_user_id: String,
) -> Result<(), Error> {
    database
        .transaction(async |database| {
            if !channel_exists(database, &channel_id).await? {
                return Err(Error::not_found("Channel not found"));
            }

            let permission = get_channel_member_perm(database, &channel_id, &user_id).await?;

            if permission != ChannelPermission::Manager {
                return Err(Error::restricted("Only channel managers can invite users"));
            }

            if !user::exists(database, &invited_user_id).await? {
                return Err(Error::not_found("User not found"));
            }

            let already_member = channel_members::table
                .filter(channel_members::channel_id.eq(&channel_id))
                .filter(channel_members::user_id.eq(&invited_user_id))
                .select(channel_members::user_id)
                .first::<String>(database)
                .await
                .optional()?
                .is_some();

            if already_member {
                return Err(Error::already_exists("User is already in channel"));
            }

            let member = ch_db::ChannelMember {
                channel_id: channel_id.clone(),
                user_id: invited_user_id.clone(),
                permission: ChannelPermission::ReadWrite,
            };

            diesel::insert_into(channel_members::table)
                .values(&member)
                .execute(database)
                .await
                .map_err(|err| match err {
                    diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        _,
                    ) => Error::already_exists("User is already in channel"),
                    err => err.into(),
                })?;

            push_notifications(
                database,
                &invited_user_id,
                [Notification::Invite {
                    notification_id: generate_unique_id(),
                    timestamp: Timestamp::now(),
                    channel_id,
                    invited_by: user_id,
                    uninvited: false,
                }],
            )
            .await?;

            Ok(())
        })
        .await
}

pub async fn uninvite(
    database: &mut DatabaseConnection,
    channel_id: String,
    user_id: String,
    uninvited_user_id: String,
) -> Result<(), Error> {
    database
        .transaction(async |database| {
            if !channel_exists(database, &channel_id).await? {
                return Err(Error::not_found("Channel not found"));
            }

            let permission = get_channel_member_perm(database, &channel_id, &user_id).await?;

            let is_self_uninvite = user_id == uninvited_user_id;

            if !is_self_uninvite && permission != ChannelPermission::Manager {
                return Err(Error::restricted(
                    "Only channel managers can uninvite users",
                ));
            }

            let target_permission = channel_members::table
                .filter(channel_members::channel_id.eq(&channel_id))
                .filter(channel_members::user_id.eq(&uninvited_user_id))
                .select(channel_members::permission)
                .first::<ChannelPermission>(database)
                .await
                .optional()?
                .ok_or_else(|| Error::not_found("User is not a member of the channel"))?;

            if !is_self_uninvite && target_permission == ChannelPermission::Manager {
                return Err(Error::restricted("Managers cannot uninvite other managers"));
            }

            if is_self_uninvite && target_permission == ChannelPermission::Manager {
                let manager_count = channel_members::table
                    .filter(channel_members::channel_id.eq(&channel_id))
                    .filter(channel_members::permission.eq(ChannelPermission::Manager))
                    .count()
                    .get_result::<i64>(database)
                    .await?;

                if manager_count <= 1 {
                    return Err(Error::restricted(
                        "The only channel manager cannot leave the channel",
                    ));
                }
            }

            diesel::delete(
                channel_members::table
                    .filter(channel_members::channel_id.eq(&channel_id))
                    .filter(channel_members::user_id.eq(&uninvited_user_id)),
            )
            .execute(database)
            .await?;

            push_notifications(
                database,
                &uninvited_user_id,
                [Notification::Invite {
                    notification_id: generate_unique_id(),
                    timestamp: Timestamp::now(),
                    channel_id,
                    invited_by: user_id,
                    uninvited: true,
                }],
            )
            .await?;

            Ok(())
        })
        .await
}

pub async fn set_channel_member_perm(
    database: &mut DatabaseConnection,
    channel_id: String,
    user_id: String,
    target_user_id: String,
    permission: ChannelPermission,
) -> Result<(), Error> {
    database
        .transaction(async |database| {
            if !channel_exists(database, &channel_id).await? {
                return Err(Error::not_found("Channel not found"));
            }

            let issuer_permission =
                get_channel_member_perm(database, &channel_id, &user_id).await?;

            if issuer_permission != ChannelPermission::Manager {
                return Err(Error::restricted(
                    "Only channel managers can change member permissions",
                ));
            }

            let target_permission =
                get_channel_member_perm(database, &channel_id, &target_user_id).await?;

            if target_user_id != user_id && target_permission == ChannelPermission::Manager {
                return Err(Error::restricted(
                    "Managers cannot change another manager's permission",
                ));
            }

            if target_user_id == user_id
                && target_permission == ChannelPermission::Manager
                && permission != ChannelPermission::Manager
            {
                let manager_count = channel_members::table
                    .filter(channel_members::channel_id.eq(&channel_id))
                    .filter(channel_members::permission.eq(ChannelPermission::Manager))
                    .count()
                    .get_result::<i64>(database)
                    .await?;

                if manager_count <= 1 {
                    return Err(Error::restricted(
                        "Cannot remove the only manager from a channel",
                    ));
                }
            }

            diesel::update(
                channel_members::table
                    .filter(channel_members::channel_id.eq(&channel_id))
                    .filter(channel_members::user_id.eq(&target_user_id)),
            )
            .set(channel_members::permission.eq(permission))
            .execute(database)
            .await?;

            Ok(())
        })
        .await
}

pub async fn get_channel(
    database: &mut DatabaseConnection,
    channel_id: &str,
) -> Result<Option<Channel>, Error> {
    let channel = channels::table
        .find(channel_id)
        .select(ch_db::Channel::as_select())
        .first(database)
        .await
        .optional()?;

    let Some(channel) = channel else {
        return Ok(None);
    };

    let members = channel_members::table
        .filter(channel_members::channel_id.eq(channel_id))
        .select(ch_db::ChannelMember::as_select())
        .load(database)
        .await?;

    let data = ch_db::ChannelData { channel, members };

    Ok(Some(Channel::from_db(data)?))
}

pub async fn get_channel_member_perm(
    database: &mut DatabaseConnection,
    channel_id: &str,
    user_id: &str,
) -> Result<ChannelPermission, Error> {
    channel_members::table
        .filter(channel_members::channel_id.eq(channel_id))
        .filter(channel_members::user_id.eq(user_id))
        .select(channel_members::permission)
        .first::<ChannelPermission>(database)
        .await
        .optional()?
        .ok_or(Error::not_found("User not in channel"))
}

pub async fn channel_exists(
    database: &mut DatabaseConnection,
    channel_id: &str,
) -> Result<bool, Error> {
    Ok(channels::table
        .find(channel_id)
        .select(channels::channel_id)
        .first::<String>(database)
        .await
        .optional()?
        .is_some())
}

pub async fn send(database: &mut DatabaseConnection, message: Message) -> Result<Message, Error> {
    database
        .transaction(async |database| {
            let channel = get_channel(database, &message.channel_id)
                .await?
                .ok_or(Error::not_found("Channel not found"))?;

            let message_data = message.clone().into_db()?;

            diesel::insert_into(messages::table)
                .values(&message_data)
                .execute(database)
                .await
                .map_err(|err| match err {
                    diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        _,
                    ) => Error::already_exists("Message already exists"),
                    err => err.into(),
                })?;

            for member in channel.members.keys() {
                if member == &message.user_id {
                    continue;
                }

                push_notifications(
                    database,
                    member,
                    [Notification::Message {
                        notification_id: generate_unique_id(),
                        timestamp: Timestamp::now(),
                        channel_id: channel.channel_id.clone(),
                        sender_id: message.user_id.clone(),
                        message: message.clone(),
                    }],
                )
                .await?;
            }

            Ok::<Message, Error>(message)
        })
        .await
}

pub async fn read_messages(
    database: &mut DatabaseConnection,
    channel_id: &str,
    limit: u32,
    start_at: Timestamp,
) -> Result<Vec<Message>, Error> {
    let rows = messages::table
        .filter(messages::channel_id.eq(channel_id))
        .filter(messages::created_at.lt(start_at.0))
        .order(messages::created_at.desc())
        .limit(limit as i64)
        .select(msg_db::Message::as_select())
        .load::<msg_db::Message>(database)
        .await?;

    rows.into_iter().map(Message::from_db).collect()
}

pub async fn delete_message(
    database: &mut DatabaseConnection,
    message_id: &str,
) -> Result<(), Error> {
    let deleted = diesel::delete(messages::table.find(message_id))
        .execute(database)
        .await?;

    if deleted == 0 {
        return Err(Error::not_found("Message not found"));
    }

    Ok(())
}

pub async fn get_msg(
    database: &mut DatabaseConnection,
    message_id: &str,
) -> Result<Option<Message>, Error> {
    let message = messages::table
        .find(message_id)
        .select(msg_db::Message::as_select())
        .first::<msg_db::Message>(database)
        .await
        .optional()?;

    message.map(Message::from_db).transpose()
}
