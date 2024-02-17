use std::{collections::hash_map::DefaultHasher, hash::{Hash, Hasher}};

pub fn hash(to_be_hashed: String) -> u64 {
    let mut hasher = DefaultHasher::new();
    to_be_hashed.hash(&mut hasher);
    hasher.finish()
}

pub fn rgb_string(r: u8, g: u8, b: u8) -> String {
    format!(
        "\x1b[38;2;{};{};{}m",
        r.to_string(),
        g.to_string(),
        b.to_string()
    )
}