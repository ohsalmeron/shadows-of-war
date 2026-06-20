pub const TRIBE_ANIMALS: [&str; 40] = [
    "🦁", "🐯", "🐆", "🐺", "🦊", "🦝", "🐻", "🐨", "🐼", "🐗", "🦄", "🦅", "🦉", "🐊", "🦖", "🐉",
    "🦈", "🦂", "🐃", "🐏", "🐘", "🦏", "🦍", "🐎", "🦌", "🦇", "🦢", "🦩", "🐍", "🐢", "🐙", "🐬",
    "🐝", "🦋", "🕷️", "🦦", "🦫", "🐫", "🦘", "🦡",
];

pub fn animal_for_name(name: &str) -> &'static str {
    let mut h = 0u32;
    for b in name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    TRIBE_ANIMALS[h as usize % TRIBE_ANIMALS.len()]
}

pub fn animal_for_id(id: u16) -> &'static str {
    TRIBE_ANIMALS[(id as usize) % TRIBE_ANIMALS.len()]
}

mod names;
pub use names::{FALLBACK_TRIBES, HISTORICAL_CIVILIZATIONS};
