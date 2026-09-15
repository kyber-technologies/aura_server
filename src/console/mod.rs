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
    general::STATUS,
    general::CLEAR,
    user::CREATE,
    user::DELETE,
    user::AUTH,
    user::SEARCH,
    chat::CREATE,
    chat::GET,
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
                state.set_exit();
                break;
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
pub struct CommandError(String);

impl From<crate::error::Error> for CommandError {
    fn from(value: crate::error::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<no_pico_args::Error> for CommandError {
    fn from(value: no_pico_args::Error) -> Self {
        Self(value.to_string())
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CommandError {}
