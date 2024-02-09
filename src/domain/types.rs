use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

use super::{room::Room, user::User};

pub type PeerMap = Arc<Mutex<HashMap<u64, (Arc<Mutex<User>>, UnboundedSender<Message>)>>>;
pub type RoomMap = Arc<Mutex<HashMap<u64, Room>>>;
