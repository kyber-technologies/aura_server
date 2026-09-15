use crate::console::{Command, CommandError};
use crate::state::ServerState;
use no_pico_args::Arguments;
use std::pin::Pin;

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

pub const CLEAR: Command = Command {
    name: "clear",
    description: "Clears the console",
    usage: "clear",
    execute: |_: Arguments,
              _: ServerState|
     -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>> {
        Box::pin(async move {
            print!("\x1B[2J\x1B[1;1H");
            Ok(())
        })
    },
};
