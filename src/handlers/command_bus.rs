use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::stream::SplitStream;
use futures_util::StreamExt;
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use std::collections::HashMap;
use anyhow::Result;
use anyhow::anyhow;



#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub enum CommandKind {
    Greet,
    Quit,
}

#[derive(Clone, Debug)]
pub enum Command {
    Greet(String),
    Quit,
}

impl Command {
    fn get_kind(&self) -> CommandKind {
        match self {
            Self::Greet(_) => CommandKind::Greet,
            Self::Quit => CommandKind::Quit,
        }
    }

    fn parse(input: String) -> Self {
        if input == "q".to_string() {
            Self::Quit
        } else {
            Self::Greet(input)
        }
    }
}

pub trait CommandHandler {
    fn get_sink(&self) -> UnboundedSender<Command>;
    fn get_kind(&self) -> CommandKind;
}

#[derive(Clone)]
pub struct CommandBus {
    handlers: HashMap<CommandKind, UnboundedSender<Command>>,
}

impl CommandBus {
    pub fn init() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    fn register(&mut self, command_kind: CommandKind, sender: UnboundedSender<Command>) {
        self.handlers
        .entry(command_kind.clone())
        .and_modify(|_| {
            panic!("Tried to re-register handler for {command_kind:?}");
        })
        .or_insert(sender.clone());
    }

    pub fn register_handler(&mut self, handler: impl CommandHandler) {
        self.register(handler.get_kind(), handler.get_sink());
    }

    fn publish(&self, cmd: &Command) -> Result<()> {
        if let Some(sender) = self.handlers.get(&cmd.get_kind()) {
            info!("publishing {cmd:?}");
            sender.unbounded_send(cmd.clone())?;
            
        } else {
            return Err(anyhow!("Could not find handler"));
        }; 
 
        Ok(())
    }
}


pub async fn command_parser(command_bus: CommandBus, mut source: SplitStream<WebSocketStream<TcpStream>>) {
    info!("Started buf worker");
    loop {
        let next_msg = source.next();
        tokio::select! {
            msg_maybe = next_msg => {
                match msg_maybe {
                    Some(s) => {
                        let cmd = Command::parse(s.unwrap().to_string());
                        command_bus.publish(&cmd).expect(&format!("Could not publish command {:?}", cmd));
                    },
                    None => {
                        info!("exiting, upstream closed");
                        let cmd = Command::Quit;
                        command_bus.publish(&cmd).expect(&format!("Could not publish command {:?}", cmd));
                        break;
                    },
                    
                }
            },
        }
    }
}