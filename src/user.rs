use std::net::SocketAddr;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct User {
    addr: SocketAddr,
}

impl User {
    pub fn new(addr: SocketAddr) -> Self {
        User { addr }
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.addr
    }
}
