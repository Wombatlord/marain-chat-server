mod handlers;
mod user;
use env_logger;
use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use handlers::{
    commands::command_handler, messages::global_message_handler,
    recv_routing::recv_routing_handler, rooms::room_handler,
};
use log::info;
use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, HashMap},
    env,
    hash::{Hash, Hasher},
    io::Error,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message, Result,
    },
    WebSocketStream,
};

use crate::user::User;

type PeerMap = Arc<Mutex<HashMap<u64, (Arc<Mutex<User>>, UnboundedSender<Message>)>>>;
type RoomMap = Arc<Mutex<HashMap<u64, PeerMap>>>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let rooms = RoomMap::new(Mutex::new(HashMap::new()));

    let mut hasher = DefaultHasher::new();
    let global_room = String::from("hub");
    global_room.hash(&mut hasher);
    let global_room_hash = hasher.finish();

    rooms
        .lock()
        .unwrap()
        .insert(global_room_hash, Arc::new(Mutex::new(HashMap::new())));
    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);
    let mut i = 0u64;
    while let Ok((stream, _)) = listener.accept().await {
        let user = Arc::new(Mutex::new(User::new(
            stream.peer_addr()?,
            global_room_hash,
            i,
        )));
        i += 1;

        info!(
            "TCP connection from: {}",
            user.lock().unwrap().get_addr().to_string()
        );
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");
        info!(
            "Upgraded {} to websocket.",
            user.lock().unwrap().get_addr().to_string()
        );
        // you can do stuff here
        let user_inbox = register_user(user.clone(), rooms.clone(), global_room_hash);
        let (ws_sink, ws_source) = ws_stream.split();
        let (cmd_sink, cmd_source) = unbounded::<Message>();
        let (msg_sink, msg_source) = unbounded::<Message>();
        let (room_sink, room_source) = unbounded::<Message>();
        tokio::spawn(recv_routing_handler(
            ws_source,
            user.clone(),
            cmd_sink,
            msg_sink,
            rooms.clone(),
        ));
        tokio::spawn(command_handler(
            cmd_source,
            room_sink,
            user.clone(),
            rooms.clone(),
        ));
        tokio::spawn(room_handler(room_source, user.clone(), rooms.clone()));
        tokio::spawn(global_message_handler(
            ws_sink,
            msg_source,
            rooms.clone(),
            user.clone(),
            user_inbox,
        ));
    }

    Ok(())
}

fn register_user(
    user: Arc<Mutex<User>>,
    room: RoomMap,
    room_hash: u64,
) -> UnboundedReceiver<Message> {
    // Creates an unbounded futures_util::mpsc channel
    // Locks the RoomMap Mutex<HashMap<room_id: ...>>
    // Gets, unwraps, and locks the "hub" room members Mutex<HashMap<usr_id: (user, user_sink)>>
    // Insert a tuple of (User, user_sink) under key of user.id
    // The user is now in the "hub" room and can receive from / broadcast to others in the same room.

    let (user_postbox, user_inbox) = unbounded::<Message>();
    room.lock()
        .unwrap()
        .get(&room_hash)
        .unwrap()
        .lock()
        .unwrap()
        .insert(user.lock().unwrap().id, (user.clone(), user_postbox));

    user_inbox
}
