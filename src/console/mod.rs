use crate::state::ServerState;
use crate::types::FastMap;
use no_pico_args::Arguments;
use rustyline_async::{Readline, ReadlineEvent};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::pin::Pin;

mod chat;
mod general;
mod user;
mod tracing_writer {
    use rustyline_async::SharedWriter;
    use tracing_subscriber::fmt::MakeWriter;

    pub struct TracingWriter(pub SharedWriter);

    impl MakeWriter<'_> for TracingWriter {
        type Writer = SharedWriter;

        fn make_writer(&self) -> Self::Writer {
            self.0.clone()
        }
    }
}

pub type CommandFn = fn(
    args: Arguments,
    state: ServerState,
) -> Pin<Box<dyn Future<Output = Result<(), CommandError>>>>;

static COMMANDS: &[Command] = &[
    general::EXIT,
    general::STATUS,
    general::CLEAR_CONSOLE,
    #[cfg(feature = "testing")]
    general::CLEAR_STATE,
    user::CREATE,
    user::DELETE,
    user::AUTH,
    user::SEARCH,
    chat::CREATE,
    chat::DELETE,
    chat::GET,
    chat::INVITE,
    chat::SET_PERM,
    chat::SEND,
    chat::READ,
    chat::DELETE_MSG,
];

pub fn create() -> (Readline, tracing_writer::TracingWriter) {
    let (rl, writer) = Readline::new("> ".to_string()).expect("Failed to create read-line");

    (rl, tracing_writer::TracingWriter(writer))
}

pub async fn run(readline: &mut Readline, state: ServerState) {
    let commands = FastMap::from_iter(COMMANDS.iter().map(|command| (command.name, command)));

    loop {
        match readline.readline().await.expect("Failed to read line") {
            ReadlineEvent::Line(line) => {
                readline.add_history_entry(line.clone());

                let (command, args) = line.split_once(' ').unwrap_or((line.as_str(), ""));

                if command.is_empty() {
                    continue;
                }

                if let Some(command) = commands.get(command) {
                    match (command.execute)(Arguments::from_string(args.to_string()), state.clone())
                        .await
                    {
                        Ok(()) => {}
                        Err(e) => {
                            tracing::error!("Failed to execute command: {}", e);

                            if let CommandError::Args(_) = e {
                                tracing::info!("Command Usage: {}", command.usage);
                            }
                        }
                    }
                } else if command == "help" {
                    let help = COMMANDS
                        .iter()
                        .map(|cmd| format!("'{}' - {}", cmd.usage, cmd.description))
                        .collect::<Vec<_>>()
                        .join("\n");

                    tracing::info!("Available commands:\n\n{help}\n");
                } else {
                    tracing::error!(
                        "Command '{command}' not found. \
                         Use 'help' for a list of commands."
                    );

                    if command == "stop" || command == "exit" {
                        tracing::info!("If you want to exit the application, use Ctrl+C.");
                    }
                }
            }

            ReadlineEvent::Interrupted | ReadlineEvent::Eof => {
                tracing::info!("Use 'exit' to exit the application.");
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    pub execute: CommandFn,
}

#[derive(Debug, Clone)]
pub enum CommandError {
    Aura(crate::error::Error),
    Args(no_pico_args::Error),
    Other(String),
}

impl From<crate::error::Error> for CommandError {
    fn from(value: crate::error::Error) -> Self {
        Self::Aura(value)
    }
}

impl From<no_pico_args::Error> for CommandError {
    fn from(value: no_pico_args::Error) -> Self {
        Self::Args(value)
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::Aura(e) => write!(f, "Failed operation: {}", e),
            CommandError::Args(e) => write!(f, "Failed parsing arguments: {}", e),
            CommandError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl Error for CommandError {}
