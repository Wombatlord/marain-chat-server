mod user;

use env_logger;
use futures_channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures_util::{
    future,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt, TryStreamExt,
};
use log::info;
use std::{
    borrow::Cow,
    char,
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
use chrono::Utc;

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
    // Locks the RoomMap Mutex
    // Gets the "Global" room members Mutex<vec>
    // Locks the room members mutex and unwraps the vec
    // Push a tuple of (User, UnboundedReceiver<Message>) into the vec
    // The user is now in the Global room and can receive from / broadcast to others in the same room.

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

async fn recv_routing_handler(
    ws_source: SplitStream<WebSocketStream<TcpStream>>,
    user: Arc<Mutex<User>>,
    command_pipe: UnboundedSender<Message>,
    message_pipe: UnboundedSender<Message>,
    room_map: RoomMap,
) {
    let incoming = ws_source.try_for_each(|msg| {
        if msg.is_close() {
            info!("{} disconnecting", user.lock().unwrap().get_addr());
            let rooms = room_map.lock().unwrap();
            let mut members = rooms
                .get(&user.lock().unwrap().room)
                .unwrap()
                .lock()
                .unwrap();
            members.remove(&user.lock().unwrap().id);
        }

        if msg.is_binary() {
            let msg_str = msg.to_text().unwrap();
            let chars: Vec<char> = msg_str.chars().collect();

            if chars[0] == '/' {
                info!("forwarding to command worker");
                command_pipe.unbounded_send(msg).unwrap();
            } else {
                info!("forwarding global message worker");
                message_pipe.unbounded_send(msg).unwrap()
            }
        }

        future::ok(())
    });

    incoming.await.unwrap();
}

async fn command_handler(
    mut cmd_source: UnboundedReceiver<Message>,
    room_sink: UnboundedSender<Message>,
    user: Arc<Mutex<User>>,
    room: RoomMap,
) {
    while let Some(cmd) = cmd_source.next().await {
        // let u = state.lock().unwrap();
        let room_map = room.lock().unwrap();
        println!("Room Map: {:#?}", room_map);
        let room_members = room_map
            .get(&user.lock().unwrap().room)
            .unwrap()
            .lock()
            .unwrap();

        let commander = room_members
            .iter()
            .filter_map(|(u, c)| {
                if u == &user.lock().unwrap().id {
                    Some(c)
                } else {
                    None
                }
            })
            .next()
            .unwrap();

        if cmd.is_binary() {
            let cmd_str: Vec<&str> = cmd.to_text().unwrap().split(" ").collect();
            match cmd_str[0].trim() {
                "/time" => {
                    let m = Message::Binary(Utc::now().to_string().as_bytes().to_vec());
                    // commander.unwrap().unbounded_send(m).unwrap();
                    commander.1.unbounded_send(m).unwrap()
                }
                "/mv" => room_sink
                    .unbounded_send(Message::Binary(cmd_str[1].trim().as_bytes().to_vec()))
                    .unwrap(),
                "/rms" => {
                    println!("Occupants: {:#?}", room_members);
                }
                "/crm" => {
                    println!("{:}", user.lock().unwrap().room);
                }
                _ => commander
                    .1
                    .unbounded_send(Message::Binary("No such command".as_bytes().to_vec()))
                    .unwrap(),
            }
        }
    }
}

async fn room_handler(
    mut room_source: UnboundedReceiver<Message>,
    mut user: Arc<Mutex<User>>,
    room_map: RoomMap,
) {
    while let Some(cmd) = room_source.next().await {
        let mut hasher = DefaultHasher::new();
        cmd.to_text().unwrap().to_string().hash(&mut hasher);
        let room_hash = hasher.finish();
        info!("room handler");
        let mut rooms = room_map.lock().unwrap();
        if rooms.contains_key(&room_hash) {
            let mut u = rooms
                .iter()
                .filter_map(|(_, y)| y.lock().unwrap().remove_entry(&user.lock().unwrap().id))
                .next()
                .unwrap();
            user.lock().unwrap().room = room_hash;
            let _ = rooms
                .get(&room_hash)
                .unwrap()
                .lock()
                .unwrap()
                .insert(user.lock().unwrap().id, (user.clone(), u.1.1));
        } else {
            info!("attempting to create new room");
            rooms.insert(room_hash, Arc::new(Mutex::new(HashMap::new())));
            let mut u = rooms
                .iter()
                .filter_map(|(_, y)| y.lock().unwrap().remove_entry(&user.lock().unwrap().id))
                .next()
                .unwrap();
            println!("!USER! {:?}", user.lock().unwrap().room);
            user.lock().unwrap().room = room_hash;
            println!("!USER CHANGED! {:?}", user.lock().unwrap().room);
            let _ = rooms
                .get(&room_hash)
                .unwrap()
                .lock()
                .unwrap()
                .insert(user.lock().unwrap().id, (user.clone(), u.1.1));
            println!("{:#?}", &rooms);
        }
    }
}

async fn global_message_handler(
    mut ws_sink: SplitSink<WebSocketStream<TcpStream>, Message>,
    mut message: UnboundedReceiver<Message>,
    room_map: RoomMap,
    user: Arc<Mutex<User>>,
    mut user_inbox: UnboundedReceiver<Message>,
) {
    loop {
        tokio::select! {
            broadcast_msg_from_usr = message.next() => {
                if broadcast_msg_from_usr == None {
                    break
                }
                let rooms = room_map.lock().unwrap();
                let room = rooms.get(&user.lock().unwrap().room).unwrap().lock().unwrap();
                info!("Sender Room: {}", user.lock().unwrap().room);
                info!("Sender: {:#?}", user);
                println!("Sender Peers: {room:#?}");
                let receipients = room
                    .iter()
                    .filter_map(|(mapped_user_id, (mapped_user, channel))| if mapped_user_id != &user.lock().unwrap().id { Some(channel) } else { None });


                for receipient in receipients {
                    match broadcast_msg_from_usr.clone() {
                        Some(m) => receipient.unbounded_send(m).unwrap(),
                        None => {}
                    }
                }
            }

            broacst_msg_to_usr = user_inbox.next() => {
                match broacst_msg_to_usr.clone() {
                    Some(m) => ws_sink.send(m).await.unwrap(),
                    None => {}
                }
            }
        }
    }
}

async fn determine_response(
    sink: UnboundedSender<Message>,
    mut source: SplitStream<WebSocketStream<TcpStream>>,
) -> Result<()> {
    loop {
        if let Some(msg) = source.next().await {
            info!("Message received.");
            if let Ok(msg) = msg {
                if msg.is_close() {
                    info!("Disconnecting");
                    let cf = CloseFrame {
                        code: CloseCode::Normal,
                        reason: Cow::Borrowed("finished"),
                    };
                    sink.unbounded_send(Message::Close(Some(cf))).unwrap();
                } else {
                    let msg_str = msg.to_text().unwrap();
                    match msg_str.trim() {
                        "lol" => sink
                            .unbounded_send(Message::binary("very funny\n"))
                            .unwrap(),
                        "help" => sink
                            .unbounded_send(Message::text("absolutely not".to_owned()))
                            .unwrap(),
                        _ => sink
                            .unbounded_send(Message::Text("fine be like that\n".to_string()))
                            .unwrap(),
                    }
                }
            }
        }
    }
}

async fn outbound(
    mut sink: SplitSink<WebSocketStream<TcpStream>, Message>,
    mut source: UnboundedReceiver<Message>,
) -> Result<()> {
    // We should not forward messages other than text or binary.
    // source
    //     .try_filter(|msg| future::ready(msg.is_text() || msg.is_binary()))
    //     .forward(sink)
    //     .await
    //     .expect("Failed to forward messages")

    while let Some(msg) = source.next().await {
        // if msg.is_binary() || msg.is_text() {
        info!("Sending Message");
        sink.send(msg).await?;
        // }
    }

    Ok(())
}
