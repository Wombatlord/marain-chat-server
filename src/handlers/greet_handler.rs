use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;

use super::command_bus::{Command, CommandHandler, CommandKind};


pub struct GreetHandler {
    sink: UnboundedSender<Command>
}

impl GreetHandler {
    async fn worker(mut source: UnboundedReceiver<Command>) {
        loop {
            tokio::select! {
                maybe_msg = source.next() => {
                    match maybe_msg {
                        Some(Command::Greet(greeting)) => {
                            println!("Hello {greeting}");
                        },
                        Some(_) => {},
                        None => break,
                    }
                }
            }
        }
    }

    pub fn init() -> Self {
        let (greet_cmd_sink, greet_cmd_src) = unbounded::<Command>();
        tokio::spawn(Self::worker(greet_cmd_src));
        Self {
            sink: greet_cmd_sink
        }
    }
}

impl CommandHandler for GreetHandler {
    fn get_kind(&self) -> CommandKind {
        CommandKind::Greet
    }

    fn get_sink(&self) -> UnboundedSender<Command> {
        self.sink.clone()
    }
}
