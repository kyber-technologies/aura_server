use crate::auth;
use crate::console::{Command, CommandError};
use crate::logic::resource::{BUILTIN_NAMESPACE, DEFAULT_ICON_KEY};
use crate::logic::user;
use crate::state::ServerState;
use crate::types::common::Timestamp;
use crate::types::resource::ResourceId;
use crate::types::user::{Notifications, User, UserRole};
use no_pico_args::Arguments;
use std::pin::Pin;

pub const CREATE: Command = Command {
    name: "create-user",
    description: "Creates or updates a user",
    usage: "create-user [-update (updated existing user)] <id> <username> <email> <password> <role>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let username: String = args.free_from_str()?;
            let email: String = args.free_from_str()?;
            let password: String = args.free_from_str()?;
            let role: UserRole = args.free_from_fn(|s| match s {
                "user" => Ok(UserRole::User),
                "moderator" => Ok(UserRole::Moderator),
                "admin" => Ok(UserRole::Admin),
                _ => Err(CommandError(
                    "Invalid role. Use: 'user', 'moderator' or 'admin'.".to_string(),
                )),
            })?;

            let user = User {
                user_id: user_id.clone(),
                username,
                email,
                password: auth::hash(password)?,
                role,
                created_at: Timestamp::now(),
                icon: ResourceId {
                    key: DEFAULT_ICON_KEY.to_string(),
                    namespace: BUILTIN_NAMESPACE.to_string(),
                },
                notifications: Notifications(Vec::new()),
                channels: Vec::new(),
            };

            if args.contains("-update") {
                user::update(&mut state.database().await?, user).await?;
            } else {
                user::create(&mut state.database().await?, user).await?;
            }

            tracing::info!("Successfully created user '{user_id}'.");

            Ok(())
        })
    },
};

pub const DELETE: Command = Command {
    name: "delete-user",
    description: "Delete a user",
    usage: "delete-user <user_id>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;

            let mut database = state.database().await?;

            if !user::exists(&mut database, &user_id).await? {
                return Err(CommandError("User not found".to_string()));
            }

            user::delete(&mut database, &user_id).await?;

            tracing::info!("User '{user_id}' deleted.");

            Ok(())
        })
    },
};

pub const SEARCH: Command = Command {
    name: "search-users",
    description: "Search users",
    usage: "search-users <query>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let query: String = args.free_from_str()?;
            let users = user::search(&mut state.database().await?, query).await?;

            for user in users {
                tracing::info!("User '{}' found: {user:#?}", user.user_id);
            }

            Ok(())
        })
    },
};

pub const AUTH: Command = Command {
    name: "auth-user",
    description: "Authenticates a user",
    usage: "auth-user <user_id> <password>",
    execute: |mut args: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let user_id: String = args.free_from_str()?;
            let password: String = args.free_from_str()?;

            let (token, user) = auth::auth(&mut state.database().await?, user_id, password).await?;

            tracing::info!("Token: '{token}'");
            tracing::info!("User: {user:#?}");

            Ok(())
        })
    },
};
