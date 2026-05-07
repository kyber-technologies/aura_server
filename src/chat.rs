use crate::database::Database;
use crate::error::Error;
use aura_rust::chat::v1::ChannelPermission;
use aura_rust::common::v1::ErrorCode;
use aura_rust::{Channel, Message, Timestamp};

pub const ID_LENGTH: usize = 10;

pub async fn create_channel(database: &Database, channel: Channel) -> Result<Channel, Error> {
    let channel: Option<Channel> = database
        .create(("channel", channel.channel_id.as_str()))
        .content(channel)
        .await?;

    channel.ok_or(Error::new(ErrorCode::Internal, "Failed to create channel"))
}

pub async fn get_channel(database: &Database, channel_id: &str) -> Result<Option<Channel>, Error> {
    let channel: Option<Channel> = database.select(("channel", channel_id)).await?;

    Ok(channel)
}

pub async fn get_channel_member_perm(
    database: &Database,
    channel_id: &str,
    user_id: &str,
) -> Result<ChannelPermission, Error> {
    let channel = get_channel(database, channel_id)
        .await?
        .ok_or(Error::new(ErrorCode::NotFound, "Channel not found"))?;

    let perm = *channel
        .members
        .get(user_id)
        .ok_or(Error::new(ErrorCode::NotFound, "User not in channel"))?;

    ChannelPermission::try_from(perm)
        .map_err(|_| Error::new(ErrorCode::Internal, "Failed to parse channel permission"))
}

pub async fn channel_exists(database: &Database, channel_id: &str) -> Result<bool, Error> {
    Ok(get_channel(database, channel_id).await?.is_some())
}

pub async fn build_channel_id(database: &Database) -> Result<String, Error> {
    let mut id = nanoid::nanoid!(ID_LENGTH);

    while channel_exists(database, &id).await? {
        id = nanoid::nanoid!(ID_LENGTH);
    }

    Ok(id)
}

pub async fn send(database: &Database, message: Message) -> Result<Message, Error> {
    if !channel_exists(database, &message.channel_id).await? {
        return Err(Error::new(ErrorCode::NotFound, "Channel not found"));
    }

    let message: Option<Message> = database
        .create(("message", message.message_id.as_str()))
        .content(message)
        .await?;
    let message = message.ok_or(Error::new(ErrorCode::Internal, "Failed to create message"))?;

    Ok(message)
}

pub async fn read_messages(
    database: &Database,
    channel_id: String,
    limit: u32,
    start_at: Timestamp,
) -> Result<Vec<Message>, Error> {
    let messages: Vec<Message> = database
        .query(
            r#"
SELECT *
FROM message
WHERE channel_id = $channel
  AND content.created_at.millis < $cursor
ORDER BY timestamp DESC
LIMIT $limit;
"#,
        )
        .bind(("channel", channel_id))
        .bind(("cursor", start_at))
        .bind(("limit", limit))
        .await?
        .take(0)?;

    Ok(messages)
}

pub async fn delete_message(database: &Database, message_id: &str) -> Result<(), Error> {
    if msg_exists(database, message_id).await? {
        let _: Option<Message> = database.delete(("message", message_id)).await?;

        Ok(())
    } else {
        Err(Error::new(ErrorCode::NotFound, "Message not found"))
    }
}

pub async fn get_msg(database: &Database, message_id: &str) -> Result<Option<Message>, Error> {
    let channel: Option<Message> = database.select(("message", message_id)).await?;

    Ok(channel)
}

pub async fn msg_exists(database: &Database, message_id: &str) -> Result<bool, Error> {
    Ok(get_msg(database, message_id).await?.is_some())
}

pub async fn build_message_id(database: &Database) -> Result<String, Error> {
    let mut id = nanoid::nanoid!(ID_LENGTH);

    while msg_exists(database, &id).await? {
        id = nanoid::nanoid!(ID_LENGTH);
    }

    Ok(id)
}
