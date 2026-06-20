//! In-match HUD overlays shared between the client shell and UI layers.

pub mod leaderboard;

pub use leaderboard::{
    LeaderboardRanking, LeaderboardRowDisplay, TeamRanking, INITIAL_VISIBLE_LIMIT,
};
