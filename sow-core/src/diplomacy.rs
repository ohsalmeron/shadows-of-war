//! Alliance request lifecycle, cooldowns, and betrayal heuristics.

use crate::player::{Player, PlayerId, PlayerType};
use crate::rng::NextIntExt;
use wyrand::WyRand;

/// Pending request lifetime (~20s at 100ms/tick).
pub const ALLIANCE_REQUEST_TTL_TICKS: u32 = 200;
/// Minimum delay before the same pair can get another outgoing request (~30s).
pub const ALLIANCE_REQUEST_COOLDOWN_TICKS: u32 = 300;
/// Last ~30s of an alliance: renewal proposals may be attempted (throttled by RNG).
pub const ALLIANCE_RENEWAL_WINDOW_TICKS: u32 = 300;
/// Minimum time between betrayals for one bot (~60s).
pub const BETRAYAL_COOLDOWN_TICKS: u32 = 600;
/// Traitor stigma duration for accept logic (~30s).
pub const TRAITOR_STATUS_TICKS: u32 = 300;
/// Bot dagger emoji after breaking an alliance (~10s).
pub const BOT_BETRAYAL_EMOJI_TICKS: u32 = 100;
/// Human dagger emoji after breaking an alliance (~30s).
pub const HUMAN_BETRAYAL_EMOJI_TICKS: u32 = 300;
/// Fresh alliances are protected from betrayal (~60s).
pub const ALLY_GRACE_AFTER_FORM_TICKS: u32 = 600;
/// Alliance duration on accept (must match intent handlers).
pub const ALLIANCE_DURATION_TICKS: u32 = 2400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllianceProposal {
    pub proposer: PlayerId,
    pub target: PlayerId,
    pub created_tick: u32,
}

/// Chaotic diplomats: well below 1% of typical bot id space.
#[inline]
pub fn is_chaotic_diplomat(bot_id: PlayerId) -> bool {
    bot_id.is_multiple_of(512)
}

#[inline]
pub fn is_traitor_active(player: &Player, tick: u32) -> bool {
    player.traitor && player.traitor_tick > tick
}

#[inline]
pub fn alliance_is_young(player: &Player, ally_id: PlayerId) -> bool {
    let timer = player.alliance_timers.get(&ally_id).copied().unwrap_or(0);
    timer > ALLIANCE_DURATION_TICKS.saturating_sub(ALLY_GRACE_AFTER_FORM_TICKS)
}

/// Nations should not spam standard tribes; chaotic bots may rarely try.
pub fn is_valid_alliance_target(
    proposer_id: PlayerId,
    _neighbor_id: PlayerId,
    neighbor_type: PlayerType,
) -> bool {
    match neighbor_type {
        PlayerType::Human | PlayerType::Nation => true,
        // Tribes have one explicit low-IQ tier now; only an explicitly
        // chaotic proposer may form this otherwise unusual alliance.
        PlayerType::Bot => is_chaotic_diplomat(proposer_id),
    }
}

/// Proposal RNG ceiling (0–100 roll must be `<` this value).
pub fn alliance_propose_roll_cap(bot_id: PlayerId, bot_iq: u32, renewing: bool) -> i32 {
    if renewing {
        return 25;
    }
    if is_chaotic_diplomat(bot_id) {
        return 8;
    }
    if bot_iq >= 130 {
        return 4;
    }
    if bot_iq >= 100 {
        return 3;
    }
    2
}

/// ~90% reject traitors when evaluating inbound requests.
pub fn should_reject_traitor_request(requestor: &Player, tick: u32, roll_0_100: i32) -> bool {
    is_traitor_active(requestor, tick) && roll_0_100 >= 10
}

/// Betrayal only when attack logic would target an ally — not on a diplomacy timer.
pub fn maybe_betray_for_attack(
    bot: &Player,
    ally: &Player,
    bordering_player_count: usize,
    current_tick: u32,
    betray_cooldown_until: Option<u32>,
    rng: &mut WyRand,
) -> bool {
    if betray_cooldown_until.is_some_and(|until| current_tick < until) {
        return false;
    }
    if !ally.alive || ally.disconnected {
        return false;
    }
    if bot.team.is_some() && bot.team == ally.team {
        return false;
    }
    if alliance_is_young(bot, ally.id) {
        return false;
    }

    let me_troops = bot.troops.max(1.0);
    let ally_troops = ally.troops.max(1.0);
    let ally_max = ally.max_troops.max(1.0);

    // Critically weak (e.g. post-nuke), and we are stronger.
    if ally.troops < ally_max * 0.2 && me_troops > ally_troops {
        return true;
    }

    // Punish known traitors that are not much stronger.
    if is_traitor_active(ally, current_tick) && ally_troops < me_troops * 1.2 {
        return true;
    }

    // Only bordering polity and we dominate (3×).
    if bordering_player_count == 1 && ally_troops * 3.0 < me_troops {
        return true;
    }

    // Rare extreme opportunism (not the default 2× rule).
    if bot.iq >= 130 && me_troops >= ally_troops * 10.0 && rng.next_int(0, 100) < 2 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_config::GameConfig;

    fn sample_player(id: u16, iq: u32) -> Player {
        let mut p = Player::new_bot(
            id,
            format!("P{id}"),
            [1.0, 0.0, 0.0],
            &GameConfig::default(),
        );
        p.iq = iq;
        p
    }

    #[test]
    fn young_alliance_blocks_betrayal() {
        let mut bot = sample_player(1, 140);
        bot.alliances.push(2);
        bot.alliance_timers.insert(2, ALLIANCE_DURATION_TICKS);
        let ally = sample_player(2, 80);
        let mut rng = WyRand::new(1);
        assert!(!maybe_betray_for_attack(
            &bot, &ally, 1, 100, None, &mut rng,
        ));
    }

    #[test]
    fn only_border_three_x_betrays() {
        let mut bot = sample_player(1, 140);
        bot.troops = 3000.0;
        bot.alliances.push(2);
        bot.alliance_timers.insert(2, 100);
        let mut ally = sample_player(2, 80);
        ally.troops = 500.0;
        let mut rng = WyRand::new(1);
        assert!(maybe_betray_for_attack(
            &bot, &ally, 1, 5000, None, &mut rng,
        ));
    }

    #[test]
    fn standard_tribe_not_valid_target_for_nation() {
        assert!(!is_valid_alliance_target(8, 15, PlayerType::Bot,));
        assert!(is_valid_alliance_target(512, 100, PlayerType::Bot,));
    }

    #[test]
    fn traitor_requests_mostly_rejected() {
        let mut requestor = sample_player(2, 100);
        requestor.traitor = true;
        requestor.traitor_tick = 10_000;
        assert!(should_reject_traitor_request(&requestor, 100, 50));
        assert!(!should_reject_traitor_request(&requestor, 100, 5));
    }
}
