use crate::console::{Command, CommandError};
use crate::logic::chat;
use crate::state::ServerState;
use crate::types::chat::{Channel, ChannelPermission, Message};
use crate::types::common::Timestamp;
use crate::types::resource::Content;
use crate::utils::generate_unique_id;
use chrono::DateTime;
use no_pico_args::Arguments;
use std::pin::Pin;

pub const CREATE: Command = Command {
    name: "create-channel",
    description: "Creates a new channel",
    usage: "create-channel <owner> <name> <description>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let owner: String = args.free_from_str()?;
            let name: String = args.free_from_str()?;
            let description: String = args.free_from_str()?;

            let mut database = state.database().await?;

            let channel_id = generate_unique_id();

            let channel = chat::create_channel(
                &mut database,
                Channel {
                    channel_id,
                    name,
                    description,
                    members: Default::default(),
                },
                &owner,
            )
            .await?;

            tracing::info!("Created channel: {channel:#?}");

            Ok(())
        })
    },
};

pub const DELETE: Command = Command {
    name: "delete-channel",
    description: "Deletes a channel",
    usage: "delete-channel <channel_id> <user>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;
            let user: String = args.free_from_str()?;

            let mut database = state.database().await?;

            chat::delete_channel(&mut database, &channel_id, &user).await?;

            tracing::info!("Deleted channel '{channel_id}'.");

            Ok(())
        })
    },
};

pub const GET: Command = Command {
    name: "get-channel",
    description: "Gets a channel",
    usage: "get-channel <channel_id>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;

            let mut database = state.database().await?;

            if let Some(channel) = chat::get_channel(&mut database, &channel_id).await? {
                tracing::info!("Found Channel: {channel:#?}");
            } else {
                tracing::error!("Channel not found.");
            }

            Ok(())
        })
    },
};

pub const INVITE: Command = Command {
    name: "invite",
    description: "Un/invite a user from/to a channel",
    usage: "invite [--uninvite | -u (uninvite the user)] <channel_id> <user> <target_user>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;
            let user: String = args.free_from_str()?;
            let target_user: String = args.free_from_str()?;

            let uninvite = args.contains(["-u", "--uninvite"]);

            let mut database = state.database().await?;

            if uninvite {
                chat::uninvite(&mut database, &channel_id, &user, &target_user).await?;
                tracing::info!("Uninvited user '{target_user}' from '{channel_id}'.");
            } else {
                chat::invite(&mut database, &channel_id, &user, &target_user).await?;
                tracing::info!("Invited user '{target_user}' to '{channel_id}'.");
            }

            Ok(())
        })
    },
};

pub const SET_PERM: Command = Command {
    name: "set-channel-perm",
    description: "Sets a channel members permission status",
    usage: "invite <channel_id> <user> <target_user> <perm>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;
            let user: String = args.free_from_str()?;
            let target_user: String = args.free_from_str()?;
            let perm: ChannelPermission = args.free_from_fn(|s| match s {
                "read" => Ok(ChannelPermission::ReadOnly),
                "write" => Ok(ChannelPermission::ReadWrite),
                "manager" => Ok(ChannelPermission::Manager),
                _ => Err(CommandError::Other(
                    "Invalid channel permission. Valid: 'read', 'write', 'manager'.".to_string(),
                )),
            })?;

            let mut database = state.database().await?;

            chat::set_channel_member_perm(&mut database, &channel_id, &user, &target_user, perm)
                .await?;

            tracing::info!(
                "Set permission for user '{target_user}' in channel '{channel_id}' to '{perm:?}'."
            );

            Ok(())
        })
    },
};

pub const SEND: Command = Command {
    name: "send",
    description: "Send a message as a user to a channel",
    usage: "send <channel_id> <user_id> <message>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;
            let user_id: String = args.free_from_str()?;
            let message: String = args.free_from_str()?;

            let mut database = state.database().await?;

            let message_id = generate_unique_id();

            chat::send(
                &mut database,
                Message {
                    message_id: message_id.clone(),
                    channel_id,
                    user_id,
                    content: Content::Text(message),
                    created_at: Timestamp::now(),
                },
            )
            .await?;

            tracing::info!("Sent message '{message_id}'.");

            Ok(())
        })
    },
};

pub const READ: Command = Command {
    name: "read",
    description: "Read messages from a channel using a limit and start date (RFC3339)",
    usage: "read <channel_id> <limit> <start_at>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let channel_id: String = args.free_from_str()?;
            let limit: u32 = args.free_from_str()?;
            let start_at: Timestamp = args.free_from_fn::<Timestamp, CommandError>(|s| {
                Ok(Timestamp(
                    DateTime::parse_from_rfc3339(s)
                        .map_err(|err| CommandError::Other(err.to_string()))?
                        .to_utc(),
                ))
            })?;

            let mut database = state.database().await?;

            let messages = chat::read(&mut database, &channel_id, limit, start_at).await?;

            tracing::info!("Read Messages: {messages:#?}");

            Ok(())
        })
    },
};

pub const DELETE_MSG: Command = Command {
    name: "delete-message",
    description: "Deletes a message.",
    usage: "delete-message <message_id>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let message_id: String = args.free_from_str()?;

            let mut database = state.database().await?;

            chat::delete_message(&mut database, &message_id).await?;

            tracing::info!("Deleted message '{message_id}'.");

            Ok(())
        })
    },
};
