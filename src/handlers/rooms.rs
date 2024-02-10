use std::{
    collections::{hash_map::DefaultHasher, HashMap, VecDeque},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;
use log::{error, info};
use tokio_tungstenite::tungstenite::Message;

use crate::domain::{room::Room, types::RoomMap, user::User, util::hash};

pub async fn room_handler(
    mut room_source: UnboundedReceiver<Message>,
    user: Arc<Mutex<User>>,
    room_map: RoomMap,
) {
    while let Some(cmd) = room_source.next().await {

        let room_hash = hash(cmd.to_text().unwrap().to_string());

        let mut rooms: std::sync::MutexGuard<'_, HashMap<u64, Room>> = room_map.lock().unwrap();
        if rooms.contains_key(&room_hash) {
            move_rooms(&rooms, &user, room_hash);
        } else {
            info!("attempting to create room: {} : {}", cmd, room_hash);
            let created = rooms.insert(
                room_hash,
                Room::new(
                    Arc::new(Mutex::new(HashMap::new())),
                    Arc::new(Mutex::new(VecDeque::new())),
                ),
            );
            match created {
                Some(_) => move_rooms(&rooms, &user, room_hash),
                None => error!("failed to create room.")
            }
        }
    }
}

fn move_rooms(
    rooms: &std::sync::MutexGuard<'_, HashMap<u64, Room>>,
    user: &Arc<Mutex<User>>,
    room_hash: u64,
) {
    info!(
        "Moving user_id: {} to {}",
        user.lock().unwrap().id,
        room_hash
    );
    let (_usr_id, (_u, channel)) = rooms
        .iter()
        .filter_map(|(_, room)| {
            room.occupants
                .lock()
                .unwrap()
                .remove_entry(&user.lock().unwrap().id)
        })
        .next()
        .unwrap();
    user.lock().unwrap().room = room_hash;
    user.lock().unwrap().up_to_date = false;
    rooms
        .get(&room_hash)
        .unwrap()
        .occupants
        .lock()
        .unwrap()
        .insert(user.lock().unwrap().id, (user.clone(), channel.clone()));

    let msg_bus = rooms.get(&room_hash).unwrap().message_bus.lock().unwrap();
    let history = prep_message_history(msg_bus.clone());
    channel.unbounded_send(history).unwrap();
    user.lock().unwrap().up_to_date = true;
}

fn prep_message_history(msg_bus: VecDeque<Message>) -> Message {
    let history = msg_bus
        .iter()
        .map(|m| format!("{}\n", m.to_text().unwrap()));
    let mut history_str = String::from("");
    for s in history {
        history_str += &s;
    }
    Message::Text(history_str)
}
