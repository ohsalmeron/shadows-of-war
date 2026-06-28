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

/// Empire/nation symbols — a distinct category from the tribe animals (power, statecraft,
/// civilization). All must exist in the GPU emoji atlas.
pub const EMPIRE_EMOJIS: [&str; 16] = [
    "🏛️", "👑", "⚔️", "🛡️", "🏹", "📜", "⚖️", "🏆", "🎖️", "💎", "🪖", "⚓", "🔥", "💣", "🚀", "🤖",
];

pub fn empire_emoji_for_name(name: &str) -> &'static str {
    let mut h = 0u32;
    for b in name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    EMPIRE_EMOJIS[h as usize % EMPIRE_EMOJIS.len()]
}

pub fn empire_emoji_for_id(id: u16) -> &'static str {
    EMPIRE_EMOJIS[(id as usize) % EMPIRE_EMOJIS.len()]
}

mod names;
pub use names::{FALLBACK_TRIBES, HISTORICAL_CIVILIZATIONS};
