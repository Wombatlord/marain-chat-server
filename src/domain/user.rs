#[derive(Clone, Debug)]
pub struct User {
    pub room: u64,
    pub id: u128,
    pub up_to_date: bool,
    pub name: String,
}

impl User {
    pub fn new(room: u64, id: u128, up_to_date: bool, name: String) -> Self {
        User {
            room,
            id,
            up_to_date,
            name,
        }
    }
}
