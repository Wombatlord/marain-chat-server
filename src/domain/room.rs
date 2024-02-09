use std::{collections::VecDeque, sync::{Arc, Mutex}};

use tokio_tungstenite::tungstenite::Message;

use super::types::PeerMap;

pub struct Room {
    pub occupants: PeerMap,
    pub message_bus: Arc<Mutex<VecDeque<Message>>>
}

impl Room {
    pub fn new(occupants: PeerMap, message_bus: Arc<Mutex<VecDeque<Message>>>) -> Self {
        Room {
            occupants,
            message_bus
        }
    }

    pub fn new_message(&mut self, msg: Message) {
        self.message_bus.lock().unwrap().push_front(msg);
    }
}