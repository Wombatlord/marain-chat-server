use std::sync::{Arc, Mutex};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::{StreamExt, stream::SplitSink, SinkExt};
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use crate::{user::User, RoomMap};

pub async fn global_message_handler(
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
                    .filter_map(|(mapped_user_id, (_, channel))| if mapped_user_id != &user.lock().unwrap().id { Some(channel) } else { None });


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