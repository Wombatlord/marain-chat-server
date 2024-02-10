use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use chrono::Utc;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;
use log::{error, info};
use tokio_tungstenite::tungstenite::Message;

use crate::domain::{types::RoomMap, user::User};

pub async fn command_handler(
    mut cmd_source: UnboundedReceiver<Message>,
    room_sink: UnboundedSender<Message>,
    user: Arc<Mutex<User>>,
    room: RoomMap,
) {
    while let Some(cmd) = cmd_source.next().await {
        let room_map = room.lock().unwrap();
        let current_room = room_map.get(&user.lock().unwrap().room);

        match current_room {
            Some(rm) => {
                let locked_occupants = rm.occupants.lock();
                find_commander(locked_occupants, &user, cmd, &room_sink);
            }
            None => {
                error!(
                    "Unwraped None when trying to access current Room of sender: {:#?}",
                    user
                )
            }
        }
    }
}

fn find_commander(
    locked_occupants: Result<
        MutexGuard<HashMap<u128, (Arc<Mutex<User>>, UnboundedSender<Message>)>>,
        PoisonError<MutexGuard<HashMap<u128, (Arc<Mutex<User>>, UnboundedSender<Message>)>>>,
    >,
    user: &Arc<Mutex<User>>,
    cmd: Message,
    room_sink: &UnboundedSender<Message>,
) {
    match locked_occupants {
        Ok(occupants) => {
            let commander = occupants
                .iter()
                .find_map(|(user_id, (_, c))| {
                    if user_id == &user.lock().unwrap().id {
                        Some(c.clone())
                    } else {
                        None
                    }
                })
                .unwrap();

            parse_command(cmd, commander, room_sink, occupants, user);
        }
        Err(e) => {
            error!("{e}")
        }
    }
}

fn parse_command(
    cmd: Message,
    commander: UnboundedSender<Message>,
    room_sink: &UnboundedSender<Message>,
    occupants: MutexGuard<HashMap<u128, (Arc<Mutex<User>>, UnboundedSender<Message>)>>,
    user: &Arc<Mutex<User>>,
) {
    if cmd.is_text() {
        let cmd_str: Vec<&str> = cmd.to_text().unwrap_or("").split(" ").collect();
        match cmd_str[0] {
            "/time" => {
                let m = Message::Binary(Utc::now().to_string().as_bytes().to_vec());
                commander
                    .unbounded_send(m)
                    .unwrap_or_else(|e| error!("{}", e));
            }
            "/mv" => {
                info!("forwarding to room handler");
                room_sink
                    .unbounded_send(Message::Binary(cmd_str[1].as_bytes().to_vec()))
                    .unwrap_or_else(|e| error!("{}", e));
            }
            "/who" => {
                println!("Occupants: {:#?}", occupants);
            }
            "/crm" => {
                println!("Room hash: {}", user.lock().unwrap().room);
            }
            _ => commander
                .unbounded_send(Message::Binary("No such command".as_bytes().to_vec()))
                .unwrap_or_else(|e| error!("{}", e)),
        }
    } else {
        error!("Non Text command: {cmd}")
    }
}
