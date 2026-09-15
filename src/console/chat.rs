use crate::console::{Command, CommandError};
use crate::logic::chat;
use crate::state::ServerState;
use crate::types::chat::Channel;
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

            let channel_id = chat::build_channel_id(&mut database).await?;

            let channel = chat::create_channel(
                &mut database,
                Channel {
                    channel_id,
                    name,
                    description,
                    members: Default::default(),
                },
                owner,
            )
            .await?;

            tracing::info!("Created channel: {channel:#?}");

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
