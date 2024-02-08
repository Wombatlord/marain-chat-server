use std::{collections::{hash_map::DefaultHasher, HashMap}, hash::{Hash, Hasher}, sync::{Arc, Mutex}};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;
use log::info;
use tokio_tungstenite::tungstenite::Message;

use crate::{user::User, RoomMap};


pub async fn room_handler(
    mut room_source: UnboundedReceiver<Message>,
    user: Arc<Mutex<User>>,
    room_map: RoomMap,
) {
    while let Some(cmd) = room_source.next().await {
        let mut hasher = DefaultHasher::new();
        cmd.to_text().unwrap().to_string().hash(&mut hasher);
        let room_hash = hasher.finish();
        info!("room handler");
        let mut rooms = room_map.lock().unwrap();
        if rooms.contains_key(&room_hash) {
            move_rooms(&rooms, &user, room_hash);
        } else {
            info!("attempting to create new room");
            rooms.insert(room_hash, Arc::new(Mutex::new(HashMap::new())));
            move_rooms(&rooms, &user, room_hash);
        }
    }
}

fn move_rooms(rooms: &std::sync::MutexGuard<'_, HashMap<u64, Arc<Mutex<HashMap<u64, (Arc<Mutex<User>>, UnboundedSender<Message>)>>>>>, user: &Arc<Mutex<User>>, room_hash: u64) {
    let u = rooms
        .iter()
        .filter_map(|(_, y)| y.lock().unwrap().remove_entry(&user.lock().unwrap().id))
        .next()
        .unwrap();
    user.lock().unwrap().room = room_hash;
    
    rooms
        .get(&room_hash)
        .unwrap()
        .lock()
        .unwrap()
        .insert(user.lock().unwrap().id, (user.clone(), u.1 .1));
        // .unwrap();
}