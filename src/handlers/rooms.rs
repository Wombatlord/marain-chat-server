use std::{
    collections::{hash_map::DefaultHasher, HashMap, VecDeque},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;
use log::info;
use tokio_tungstenite::tungstenite::Message;

use crate::domain::{room::Room, types::RoomMap, user::User};

pub async fn room_handler(
    mut room_source: UnboundedReceiver<Message>,
    user: Arc<Mutex<User>>,
    room_map: RoomMap,
) {
    while let Some(cmd) = room_source.next().await {
        let mut hasher = DefaultHasher::new();
        cmd.to_text().unwrap().to_string().hash(&mut hasher);
        let room_hash = hasher.finish();
        // let mut rooms = room_map.lock().unwrap();
        let mut rooms: std::sync::MutexGuard<'_, HashMap<u64, Room>> = room_map.lock().unwrap();
        if rooms.contains_key(&room_hash) {
            move_rooms(&rooms, &user, room_hash);
        } else {
            info!("attempting to create room: {} : {}", cmd, room_hash);
            rooms.insert(
                room_hash,
                Room::new(
                    Arc::new(Mutex::new(HashMap::new())),
                    Arc::new(Mutex::new(VecDeque::new())),
                ),
            );
            move_rooms(&rooms, &user, room_hash);
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
    let u = rooms
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
        .insert(user.lock().unwrap().id, (user.clone(), u.1 .1));
}
