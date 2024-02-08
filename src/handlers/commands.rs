use std::{collections::HashMap, sync::{Arc, Mutex}};

use chrono::Utc;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;
use futures_util::StreamExt;
use crate::user::User;
type PeerMap = Arc<Mutex<HashMap<u64, (Arc<Mutex<User>>, UnboundedSender<Message>)>>>;
type RoomMap = Arc<Mutex<HashMap<u64, PeerMap>>>;

pub async fn command_handler(
    mut cmd_source: UnboundedReceiver<Message>,
    room_sink: UnboundedSender<Message>,
    user: Arc<Mutex<User>>,
    room: RoomMap,
) {
    while let Some(cmd) = cmd_source.next().await {
        // let u = state.lock().unwrap();
        let room_map = room.lock().unwrap();
        let room_members = room_map
            .get(&user.lock().unwrap().room)
            .unwrap()
            .lock()
            .unwrap();

        let commander = room_members
            .iter()
            .filter_map(|(u, c)| {
                if u == &user.lock().unwrap().id {
                    Some(c.1.clone())
                } else {
                    None
                }
            })
            .next()
            .unwrap();

        if cmd.is_text() {
            let cmd_str: Vec<&str> = cmd.to_text().unwrap().split(" ").collect();
            match cmd_str[0] {
                "/time" => {
                    let m = Message::Binary(Utc::now().to_string().as_bytes().to_vec());
                    commander.unbounded_send(m).unwrap()
                }
                "/mv" => room_sink
                    .unbounded_send(Message::Binary(cmd_str[1].as_bytes().to_vec()))
                    .unwrap(),
                "/rms" => {
                    println!("Occupants: {:#?}", room_members);
                }
                "/crm" => {
                    println!("{:}", user.lock().unwrap().room);
                }
                _ => commander
                    .unbounded_send(Message::Binary("No such command".as_bytes().to_vec()))
                    .unwrap(),
            }
        }
    }
}