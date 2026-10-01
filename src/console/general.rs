use crate::console::{Command, CommandError};
use crate::state::ServerState;
use aura_rust::types::FileDescriptorSet;
use aura_rust::{FILE_DESCRIPTOR_SET, Message};
use no_pico_args::Arguments;
use std::pin::Pin;

pub const EXIT: Command = Command {
    name: "exit",
    description: "Exits the server application",
    usage: "exit",
    execute: |_: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            state.set_exit();

            Ok(())
        })
    },
};

pub const STATUS: Command = Command {
    name: "status",
    description: "Prints the server status",
    usage: "status",
    execute: |_: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let status = state
                .status()
                .await?
                .into_iter()
                .map(|(k, v)| format!("  {k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n");

            tracing::info!("Server Status:\n{status}");
            tracing::info!("Testing Mode: {}", cfg!(feature = "testing"));

            Ok(())
        })
    },
};

pub const CLEAR_CONSOLE: Command = Command {
    name: "clear-console",
    description: "Clears the console",
    usage: "clear-console",
    execute: |_: Arguments,
              _: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            print!("\x1B[2J\x1B[1;1H");
            Ok(())
        })
    },
};

pub const SERVICES: Command = Command {
    name: "services",
    description: "List active services",
    usage: "services",
    execute: |_: Arguments,
              _: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            let services = FileDescriptorSet::decode(FILE_DESCRIPTOR_SET)
                .expect("Failed to decode file descriptor set")
                .file
                .into_iter()
                .flat_map(|desc| desc.service)
                .map(|serv| aura_rust::general::v1::ServiceDescriptor {
                    name: serv.name.unwrap_or_else(|| "<unknown>".to_string()),
                    methods: serv
                        .method
                        .into_iter()
                        .map(|meth| meth.name.unwrap_or_else(|| "<unknown>".to_string()))
                        .collect(),
                })
                .collect::<Vec<_>>();

            tracing::info!("Services: {services:#?}");

            Ok(())
        })
    },
};

#[cfg(feature = "testing")]
pub const CLEAR_STATE: Command = Command {
    name: "clear-state",
    description: "Clears the database state (testing mode only)",
    usage: "clear-state",
    execute: |_: Arguments,
              state: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            state.clear_state().await?;

            Ok(())
        })
    },
};
