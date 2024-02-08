use std::net::SocketAddr;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct User {
    addr: SocketAddr,
    pub room: u64,
    pub id: u64,
}

impl User {
    pub fn new(addr: SocketAddr, room: u64, id: u64) -> Self {
        User { addr, room, id }
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.addr
    }
}
