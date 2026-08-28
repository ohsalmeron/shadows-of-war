pub mod avatar;
pub mod dev;
#[cfg(not(target_arch = "wasm32"))]
pub mod endgame;
pub mod leaderboard;
pub mod nameplate;
pub mod tutorial;
