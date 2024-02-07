use std::{collections::VecDeque, fmt::Error};

use futures_util::stream::SplitSink;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

pub struct Room {
    pub messages: VecDeque<Message>,
    pub sinks: Vec<SplitSink<WebSocketStream<TcpStream>, Message>>
}

impl Room {
    pub fn new() -> Self {
        Room {
            messages: vec![].into(),
            sinks: vec![]
        }
    }

    pub async fn next_msg(&mut self) -> Result<Message, Error> {
        match self.messages.pop_front() {
            Some(m) => Ok(m),
            _ => Err(Error),
        }
    }
}