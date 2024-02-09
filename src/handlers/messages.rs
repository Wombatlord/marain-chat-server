use std::sync::{Arc, Mutex};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::domain::{types::RoomMap, user::User};

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
                let mut msg_bus = rooms.get(&user.lock().unwrap().room).unwrap().message_bus.lock().unwrap();
                msg_bus.push_back(broadcast_msg_from_usr.clone().unwrap());
                let history = msg_bus.iter().map(|m| format!("{}\n", m.to_text().unwrap()));
                let mut history_str = String::from("");
                for s in history {
                    history_str += &s;
                }
                let occupants = rooms.get(&user.lock().unwrap().room).unwrap().occupants.lock().unwrap();
                let receipients = occupants
                    .iter()
                    .filter_map(|(mapped_user_id, (mapped_user, channel))| if mapped_user_id != &user.lock().unwrap().id { Some((mapped_user, channel)) } else { None });
                info!("{:?}", msg_bus);

                for (receipient_user, receipient) in receipients {
                    let utd =  receipient_user.lock().unwrap().up_to_date;
                    match utd {
                        true => receipient.unbounded_send(broadcast_msg_from_usr.clone().unwrap()).unwrap(),
                        false => {
                            receipient.unbounded_send(Message::Text(history_str.to_string())).unwrap();
                        }
                    }
                    receipient_user.lock().unwrap().up_to_date = true;



                    // match broadcast_msg_from_usr.clone() {
                    //     Some(m) => receipient.unbounded_send(m).unwrap(),
                    //     None => {}
                    // }
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
