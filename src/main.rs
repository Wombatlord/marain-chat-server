//! A simple echo server.
//!
//! You can test this out by running:
//!
//!     cargo run --example echo-server 127.0.0.1:12345
//!
//! And then in another window run:
//!
//!     cargo run --example client ws://127.0.0.1:12345/
//!
//! Type a message into the client window, press enter to send it and
//! see it echoed back.

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
    borrow::Cow, char, collections::HashMap, env, hash::Hash, io::Error, sync::{Arc, Mutex}
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message, Result,
    },
    WebSocketStream,
};

use chrono::Utc;

// This should maybe be Arc<Mutex<Hashmap<String, Mutex<HashMap<User, UnboundedSender<Message>>>>>>
type RoomMap = Arc<Mutex<HashMap<String, Mutex<HashMap<User, UnboundedSender<Message>>>>>>;
use crate::user::User;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let rooms = RoomMap::new(Mutex::new(HashMap::new()));
    rooms
        .lock()
        .unwrap()
        .insert(String::from("Global"), Mutex::new(HashMap::new()));
    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);
    while let Ok((stream, _)) = listener.accept().await {
        let user = User::new(stream.peer_addr()?);

        info!("TCP connection from: {}", user.get_addr().to_string());
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");
        info!("Upgraded {} to websocket.", user.get_addr().to_string());
        // you can do stuff here
        let user_inbox = register_user(user, rooms.clone());
        let (ws_sink, ws_source) = ws_stream.split();
        let (cmd_sink, cmd_source) = unbounded::<Message>();
        let (msg_sink, msg_source) = unbounded::<Message>();

        tokio::spawn(recv_routing(ws_source, user, cmd_sink, msg_sink, rooms.clone()));
        tokio::spawn(commands(cmd_source, user, rooms.clone()));
        tokio::spawn(handle_global_messaging(
            ws_sink,
            msg_source,
            rooms.clone(),
            user,
            user_inbox,
        ));
    }

    Ok(())
}

fn register_user(user: User, room: RoomMap) -> UnboundedReceiver<Message> {
    // Creates a channel
    // Locks the RoomMap Mutex
    // Gets the "Global" room members Mutex<vec>
    // Locks the room members mutex and unwraps the vec
    // Push a tuple of (User, UnboundedReceiver<Message>) into the vec
    // The user is now in the Global room and can receive from / broadcast to others in the same room.

    let (user_postbox, user_inbox) = unbounded::<Message>();
    room.lock()
        .unwrap()
        .get("Global")
        .unwrap()
        .lock()
        .unwrap()
        .insert(user, user_postbox);

    user_inbox
}

async fn recv_routing(
    ws_source: SplitStream<WebSocketStream<TcpStream>>,
    user: User,
    command_pipe: UnboundedSender<Message>,
    message_pipe: UnboundedSender<Message>,
    room_map: RoomMap,
) {
    let incoming = ws_source.try_for_each(|msg| {
        if msg.is_close() {
            info!("{} disconnecting", user.get_addr());
            let rooms = room_map.lock().unwrap();
            let mut members = rooms.get("Global").unwrap().lock().unwrap();
            members.remove(&user);
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

async fn commands(
    mut cmd_source: UnboundedReceiver<Message>,
    user: User,
    room: RoomMap,
) {
    while let Some(cmd) = cmd_source.next().await {
        // let u = state.lock().unwrap();
        let room_map = room.lock().unwrap();
        let room_members = room_map.get("Global").unwrap().lock().unwrap();
        
        let user = room_members
            .iter()
            .filter_map(|(u, c)| if u == &user { Some(c) } else { None })
            .next();

        if cmd.is_binary() {
            let cmd_str: Vec<&str> = cmd.to_text().unwrap().split(" ").collect();
            match cmd_str[0].trim() {
                "/time" => {
                    let m = Message::Binary(Utc::now().to_string().as_bytes().to_vec());
                    user.unwrap().unbounded_send(m).unwrap();
                },
                "/mv" => {todo!("Room handler required? Pass User and cmd_str[1], lock mutex in that task to avoid mutable borrow after locks")},
                _ => info!("non-implemented command"),
            }
        }
    }
}

async fn handle_global_messaging(
    mut ws_sink: SplitSink<WebSocketStream<TcpStream>, Message>,
    mut message: UnboundedReceiver<Message>,
    room_map: RoomMap,
    user: User,
    mut user_inbox: UnboundedReceiver<Message>,
) {
    loop {
        tokio::select! {
            broadcast_msg_from_usr = message.next() => {
                let rooms = room_map.lock().unwrap();
                let room_members = rooms.get("Global").unwrap().lock().unwrap();
                let receipients = room_members
                    .iter()
                    .filter_map(|(mapped_user, channel)| if mapped_user != &user { Some(channel) } else { None });
                
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
