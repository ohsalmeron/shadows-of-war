use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::Team;

pub const INITIAL_VISIBLE_LIMIT: usize = 10;

#[derive(Clone, Debug)]
pub struct LeaderboardRanking {
    pub id: u16,
    pub tiles: u32,
    pub troops: f64,
    pub name: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
}

#[derive(Clone, Debug)]
pub struct LeaderboardRowDisplay {
    pub name: String,
    pub player_type: PlayerType,
    pub leader: Leader,
    pub color: [f32; 3],
    pub active_emoji: Option<String>,
    pub team: Option<Team>,
}

#[derive(Clone, Debug)]
pub struct TeamRanking {
    pub team: Team,
    pub tiles: u32,
    pub member_count: u32,
    pub color: [f32; 3],
}
