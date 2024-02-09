use std::net::SocketAddr;

#[derive(Clone, Copy, Debug)]
pub struct User {
    addr: SocketAddr,
    pub room: u64,
    pub id: u64,
    pub up_to_date: bool,
}

impl User {
    pub fn new(addr: SocketAddr, room: u64, id: u64, up_to_date: bool) -> Self {
        User { addr, room, id , up_to_date}
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.addr
    }
}
