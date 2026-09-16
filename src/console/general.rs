use crate::console::{Command, CommandError};
use crate::state::ServerState;
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
            state.print_status();

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
