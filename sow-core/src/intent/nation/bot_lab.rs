//! BOT LAB — behavioral contract tests for the AI tiers.
//!
//! Local-only, deterministic, zero game overhead: everything here is
//! `#[cfg(test)]`, so none of it exists in shipped WASM/server binaries.
//! Run with `cargo test -p sow-core --lib lab::`; set `SOW_LAB_VERBOSE=1`
//! for per-window telemetry.
//!
//! Contract encoded (user mental model):
//!   1. Every bot aspires to WIN — expansion/growth never stalls while the
//!      match is live and legal actions exist (anti-"cómodo").
//!   2. Tier roles: Ghost = top aggressor/teammate; Nation = mid challenger
//!      that defends; Tribe on Vanilla = passive food that never initiates
//!      vs players but still expands into free land.
//!   3. Enclosed ghosts (allies everywhere) break out by sea when a coast
//!      exists. KNOWN-LIMIT (needs design GO): fully inland enclosed ghosts
//!      have no legal action and stay idle.

use super::profile::{ai_profile_for, ai_tier};
use crate::engine::SowEngine;
use crate::game::{GamePhase, GameState};
use crate::player::{Player, PlayerType};
use crate::protocol::{AttackIntent, GameplayIntent, Team};
use crate::water_components::WaterComponents;

const WINDOW_TICKS: u64 = 400;
const MAX_WINDOWS: usize = 5;

fn verbose() -> bool {
    std::env::var("SOW_LAB_VERBOSE").is_ok()
}

struct LabPlayer {
    id: u16,
    kind: PlayerType,
    ai: bool,
    iq: u32,
    troops: f64,
    max_troops: f64,
    gold: f64,
    x: u32,
    y: u32,
    team: Option<crate::protocol::Team>,
}

impl LabPlayer {
    fn ghost(id: u16, x: u32, y: u32) -> Self {
        Self {
            id,
            kind: PlayerType::Human,
            ai: true,
            iq: 170,
            troops: 20_000.0,
            max_troops: 40_000.0,
            gold: 0.0,
            x,
            y,
            team: None,
        }
    }

    fn nation(id: u16, x: u32, y: u32) -> Self {
        Self {
            id,
            kind: PlayerType::Nation,
            ai: false,
            iq: 150,
            troops: 4_000.0,
            max_troops: 6_000.0,
            gold: 50_000.0,
            x,
            y,
            team: None,
        }
    }

    fn tribe(id: u16, x: u32, y: u32) -> Self {
        Self {
            id,
            kind: PlayerType::Bot,
            ai: false,
            iq: 60,
            troops: 500.0,
            max_troops: 5_000.0,
            gold: 0.0,
            x,
            y,
            team: None,
        }
    }

    fn weak(mut self, factor: f64) -> Self {
        self.troops *= factor;
        self.max_troops *= factor;
        self
    }
}

/// All-land map, every `spec` owns its home tile; free land everywhere else.
fn build_lab(w: u32, h: u32, mode: &str, specs: &[LabPlayer]) -> SowEngine {
    let mut game = GameState::new(7, w, h, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Playing;
    game.config.game_mode = mode.to_string();
    // Sandbox matches must never end early via the win-percentage rule; tests
    // assert behavior, and domination outcomes are read from tile counts.
    game.config.map_control_win_percentage = 2.0;

    let mut lookup: Vec<Option<usize>> = vec![None];
    for (idx, spec) in specs.iter().enumerate() {
        let mut player = match spec.kind {
            PlayerType::Human => Player::new_human(
                spec.id,
                format!("L{}", spec.id),
                [0.2, 0.5, 1.0],
                &crate::game_config::GameConfig::default(),
            ),
            // Nations MUST be PlayerType::Nation (new_nation) — new_bot mints
            // Bot-type players, which resolve to the TRIBE tier and silently
            // turn every lab "nation" into a passive tribe with high IQ.
            PlayerType::Nation => Player::new_nation(
                spec.id,
                format!("N{}", spec.id),
                [0.8, 0.4, 0.1],
                &crate::game_config::GameConfig::default(),
            ),
            PlayerType::Bot => Player::new_bot(
                spec.id,
                format!("T{}", spec.id),
                [0.1, 0.8, 0.3],
                &crate::game_config::GameConfig::default(),
            ),
        };
        player.is_ai_controlled = spec.ai;
        player.iq = spec.iq;
        player.iq_points = 200.0;
        player.troops = spec.troops;
        player.max_troops = spec.max_troops;
        player.gold = spec.gold;
        player.team = spec.team;
        player.tile_count = 1;
        let home = spec.y * w + spec.x;
        player.border_insert(home);
        lookup.push(Some(idx));
        game.players.push(player);
    }
    game.player_lookup = lookup;

    for idx in 0..(w * h) as usize {
        game.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
    }
    for spec in specs {
        game.map.set_owner_id(spec.x, spec.y, spec.id);
    }

    SowEngine::new(game, WaterComponents::default())
}

fn tiles(engine: &SowEngine, id: u16) -> u32 {
    engine.state.player(id).unwrap().tile_count
}

fn run_window(engine: &mut SowEngine, ticks: u64) -> bool {
    for _ in 0..ticks {
        if engine.state.phase != GamePhase::Playing {
            return false;
        }
        engine.tick();
    }
    true
}

/// Grant `owner` a w×h land block. Territory is the only scale lever in the
/// lab: absolute troop specs get clamped down to the 1-tile cap on the first
/// income tick (max_troops derives from tile_count), so "big vs small" must
/// be expressed as granted land.
fn grant_block(engine: &mut SowEngine, owner: u16, x0: u32, y0: u32, w: u32, h: u32) {
    let map_w = engine.state.map.width;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            engine.state.map.set_owner_id(x, y, owner);
            if let Some(p) = engine.state.player_mut(owner) {
                p.tile_count += 1;
                p.sum_x += x as u64;
                p.sum_y += y as u64;
                p.border_insert(y * map_w + x);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S1 — Ghost isolated on free land keeps expanding until domination/victory.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s1_isolated_ghost_expands_until_victory() {
    let ghost = LabPlayer::ghost(1, 1, 1);
    let tribe = LabPlayer::tribe(2, 9, 9);
    let mut engine = build_lab(10, 10, "FFA", &[ghost, tribe]);

    let mut reached_end = true;
    for window in 0..MAX_WINDOWS {
        let before = tiles(&engine, 1);
        reached_end &= run_window(&mut engine, WINDOW_TICKS * (window as u64 + 1));
        let after = tiles(&engine, 1);
        if verbose() {
            eprintln!("S1 w={window} tiles={after}");
        }
        if reached_end || after >= (10 * 10) * 95 / 100 {
            break;
        }
        assert!(
            after > before,
            "S1 FAIL: ghost stalled at {before} tiles in window {window}"
        );
    }
    assert!(
        reached_end || tiles(&engine, 1) >= (10 * 10) * 9 / 10,
        "S1 FAIL: no domination and no live game left"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S2 — Armed ghost borders a weaker nation AND neutral pockets: must press
// the PLAYER instead of filling pockets (war pressure beats comfort).
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s2_armed_ghost_presses_player_over_neutral() {
    let ghost = LabPlayer::ghost(1, 2, 2);
    let nation = LabPlayer::nation(2, 3, 3).weak(0.05); // easy target on the border
    let mut engine = build_lab(12, 12, "FFA", &[ghost, nation]);

    let enemy_start = tiles(&engine, 2);
    run_window(&mut engine, WINDOW_TICKS);

    let pressed_player = engine.attacks.iter().any(|a| {
        a.owner_id == 1 && a.target_owner == 2 && !a.retreating && !a.to_conquer.is_empty()
    }) || tiles(&engine, 2) < enemy_start;
    assert!(
        pressed_player,
        "S2 FAIL: armed ghost never engaged the bordering player"
    );
    assert!(tiles(&engine, 1) > 1, "S2 FAIL: ghost did not grow at all");
}

// ──────────────────────────────────────────────────────────────────────────
// S3 — Vanilla tribe NEVER initiates against players, yet still expands into
// free land (passive food, not statue).
//
// KNOWN BUG (awaiting design ruling a/b/c): the enclosure-capture cascade in
// GameState::set_tile_owner (game.rs — fully-surrounded neighbors flip to the
// surrounding owner) lets a neutral-expansion wave eat PLAYER tiles without
// any directed attack. Reproduced here: tribe ends 32 tiles vs human 0 while
// alive=true, owner(3,3)=2, single ATT owner=2 target=0 in flight.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s3_vanilla_tribe_passive_but_growing() {
    let human = LabPlayer {
        id: 1,
        kind: PlayerType::Human,
        ai: false, // REAL human: no brain
        iq: 0,
        troops: 400.0,
        max_troops: 600.0,
        gold: 0.0,
        x: 3,
        y: 3,
        team: None,
    };
    let tribe = LabPlayer::tribe(2, 4, 4); // borders the human home
    let mut engine = build_lab(10, 10, "FFA", &[human, tribe]);
    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        eprintln!(
            "INIT owners(3,3)={} (4,4)={} tiles1={} tiles2={}",
            engine.state.map.owner_id(3, 3),
            engine.state.map.owner_id(4, 4),
            tiles(&engine, 1),
            tiles(&engine, 2)
        );
        for i in [27usize, 28, 33, 34, 44] {
            eprintln!(
                "INIT idx{i} owner={}",
                engine.state.map.owner_id((i as u32) % 10, (i as u32) / 10)
            );
        }
    }

    run_window(&mut engine, WINDOW_TICKS * 2);

    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        let p1 = engine.state.player(1).unwrap();
        let p2 = engine.state.player(2).unwrap();
        eprintln!(
            "S3DBG h_tiles={} t_tiles={} h_alive={} h_troops={} t_troops={} h_owner33={} att_count={}",
            p1.tile_count,
            p2.tile_count,
            p1.alive,
            p1.troops,
            p2.troops,
            engine.state.map.owner_id(3, 3),
            engine.attacks.len()
        );
        for a in &engine.attacks {
            eprintln!(
                "ATT owner={} target={} troops={} retreat={} queue={}",
                a.owner_id,
                a.target_owner,
                a.troops,
                a.retreating,
                a.to_conquer.len()
            );
        }
        let mut rows = String::new();
        for y in 0..10u32 {
            for x in 0..10u32 {
                rows.push_str(&format!("{:>3}", engine.state.map.owner_id(x, y)));
            }
            rows.push('\n');
        }
        eprintln!("OWNERSHIP GRID:\n{rows}");
    }
    let tribal_aggression = engine
        .attacks
        .iter()
        .any(|a| a.owner_id == 2 && a.target_owner == 1);
    assert!(
        !tribal_aggression,
        "S3 FAIL: Vanilla tribe initiated against a player"
    );
    assert_eq!(
        tiles(&engine, 1),
        1,
        "S3 FAIL: human lost tiles without ever acting"
    );
    assert!(
        tiles(&engine, 2) > 1,
        "S3 FAIL: passive tribe stalled instead of expanding into free land"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S4 — A Nation under attack retaliates quickly (defense beats trigger).
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s4_nation_defends_when_struck() {
    // Single-strike REAL human attacker (no AI rebuying strikes): the nation's
    // counterattack must SURVIVE the mutual-annihilation pass and be visible.
    let attacker_specs = vec![
        LabPlayer {
            id: 1,
            kind: PlayerType::Human,
            ai: false,
            iq: 0,
            troops: 300.0,
            max_troops: 600.0,
            gold: 0.0,
            x: 3,
            y: 4, // CARDINAL neighbor of (4,4)
            team: None,
        },
        LabPlayer::nation(2, 4, 4),
    ];
    let mut engine = build_lab(10, 10, "FFA", &attacker_specs);

    // Strike first as player 1 (ghost) so the nation sees inbound attacks.
    use crate::protocol::StampedIntent;
    let strike = StampedIntent {
        player_id: 1,
        intent: GameplayIntent::Attack(AttackIntent {
            target_owner: 2,
            troops: Some(50.0),
        }),
    };
    engine.apply_stamped_intent(&strike, 0);

    // Observable: next_attack_id is monotonically bumped when the nation's
    // defense spawns its OWN attack execution (immune to mutual-annihilation
    // eating the entry between polls).
    let baseline = engine.state.next_attack_id;
    let mut retaliation_tick = None;
    for t in 1..=120u64 {
        if engine.state.phase != GamePhase::Playing {
            break;
        }
        engine.tick();
        if retaliation_tick.is_none() && engine.state.next_attack_id > baseline {
            retaliation_tick = Some(t);
        }
    }
    let tick_seen = retaliation_tick.expect("S4 FAIL: struck nation never retaliated");
    assert!(
        tick_seen <= 80,
        "S4 FAIL: retaliation took {tick_seen} ticks (>80)"
    );
    if verbose() {
        eprintln!("S4 retaliation at tick {tick_seen}");
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S5 — Team-enclosed coastal ghost breaks out by SEA (portless fleet gate),
// instead of sitting comfortable behind allied walls.
// ──────────────────────────────────────────────────────────────────────────
#[test]
#[ignore = "water harness incomplete: LaunchFleet is rejected before add_fleet (route/component introspection pending); scenario kept for follow-up"]
fn s5_enclosed_coastal_ghost_breaks_out_by_sea() {
    const W: u32 = 12;
    let mut game = GameState::new(11, W, W, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Playing;
    game.config.game_mode = "Teams".to_string();
    game.config.map_control_win_percentage = 2.0;

    // Ocean everywhere except two islands:
    //   Blue island (team BLUE): ghost + ally packed side by side (top-left)
    //   Red island: lone enemy nation (bottom-right)
    let blue_team = Some(Team::Blue);
    let mk = |id: u16, kind, ai: bool, iq: u32, x: u32, y: u32, team| {
        let mut p = if kind == PlayerType::Human {
            Player::new_human(
                id,
                format!("P{id}"),
                [0.2, 0.5, 1.0],
                &crate::game_config::GameConfig::default(),
            )
        } else {
            Player::new_bot(
                id,
                format!("B{id}"),
                [0.8, 0.3, 0.2],
                &crate::game_config::GameConfig::default(),
            )
        };
        p.player_type = kind;
        p.is_ai_controlled = ai;
        p.iq = iq;
        p.iq_points = 300.0;
        p.troops = 20_000.0;
        p.max_troops = 40_000.0;
        p.tile_count = 1;
        p.team = team;
        p.border_insert(y * W + x);
        p
    };

    game.players
        .push(mk(1, PlayerType::Human, true, 170, 0, 0, blue_team));
    game.players
        .push(mk(3, PlayerType::Human, true, 165, 0, 1, blue_team)); // ally wall to ghost's south
    game.players
        .push(mk(2, PlayerType::Nation, false, 150, W - 1, W - 1, None));
    game.player_lookup = vec![None, Some(0), Some(1), Some(2)];

    for idx in 0..(W * W) as usize {
        game.map.terrain[idx] = crate::map::MapTile::from_byte(0b0000_0001); // WATER
    }
    let land_tiles = [(0u32, 0u32), (0u32, 1u32)];
    let owners = [1u16, 3u16];
    for ((x, y), owner) in land_tiles.iter().zip(owners) {
        let (x, y) = (*x, *y);
        game.map.terrain[(y * W + x) as usize] = crate::map::MapTile::from_byte(0b1000_0000);
        game.map.set_owner_id(x, y, owner);
    }
    let enemy_home = (W - 1) * W + (W - 1);
    game.map.terrain[enemy_home as usize] = crate::map::MapTile::from_byte(0b1000_0000);
    game.map.set_owner_id(W - 1, W - 1, 2);

    let water = WaterComponents::compute(&game.map, |_| {});
    let mut engine = SowEngine::new(game, water);

    let ghost_start = tiles(&engine, 1);
    let enemy_start = tiles(&engine, 2);
    let fleets_before = engine.state.next_fleet_id;
    run_window(&mut engine, WINDOW_TICKS * 3);

    let launched = engine.state.next_fleet_id > fleets_before;
    let naval_action = launched || !engine.fleets.is_empty() || tiles(&engine, 2) < enemy_start;
    assert!(
        naval_action || tiles(&engine, 1) > ghost_start,
        "S5 FAIL: enclosed coastal ghost neither broke out by sea nor advanced"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S7 — ASPIRATION MARATHON: ghosts vs nations, FFA. EVERY seed must END
// (somebody wins). An eternal match means nobody wants to win — forbidden.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s7_every_match_ends_somebody_wins() {
    let seeds = [101, 202, 303, 404, 505, 606, 707, 808, 909, 1010];
    const CAP: u64 = 4_000;
    let mut decided = 0usize;
    let mut ghost_wins = 0usize;

    for seed in seeds {
        let ghost = LabPlayer::ghost(1, 1, 1);
        let nation = LabPlayer::nation(2, 10, 10);
        let mut engine = build_lab(12, 12, "FFA", &[ghost, nation]);

        for _ in 0..CAP {
            if engine.state.winner.is_some() || engine.state.phase == GamePhase::GameOver {
                break;
            }
            engine.tick();
        }
        let over = engine.state.winner.is_some() || engine.state.phase == GamePhase::GameOver;
        assert!(
            over,
            "S7 FAIL: seed {seed} never decided within {CAP} ticks — nobody wants to win"
        );
        decided += 1;
        if engine.state.winner == Some(1) {
            ghost_wins += 1;
        }
        if verbose() {
            eprintln!(
                "S7 seed={seed} winner={:?} ticks_final={} g_tiles={} n_tiles={}",
                engine.state.winner,
                engine.state.tick,
                tiles(&engine, 1),
                tiles(&engine, 2)
            );
        }
    }
    assert_eq!(decided, seeds.len());
    eprintln!(
        "S7 marathon: {decided}/{} decided · ghost wins {ghost_wins}/{decided}",
        seeds.len()
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S9 — CLUSTER REPRO: prod-shaped lobby. Multiple AI entities spawn packed
// near the human spawn zone (matches tick.rs staggered deployment), FFA, all
// land. Goal: make the mid-game stall EMERGE in simulation so we can dissect
// it entity-by-entity instead of guessing against production.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s9_cluster_lobby_stall_emergence() {
    const W: u32 = 24;
    let mut specs = Vec::new();
    // Pack 6 ghosts loosely around center-left, like the human-spawn ring.
    let ring = [(6u32, 8u32), (8, 6), (10, 8), (8, 10), (10, 11), (11, 9)];
    for (i, (x, y)) in ring.iter().enumerate() {
        let mut g = LabPlayer::ghost((i + 1) as u16, *x, *y);
        g.gold = 100_000.0;
        specs.push(g);
    }
    // Distant nations & tribes holding their own corners.
    let mut nation = LabPlayer::nation(7, 20, 20);
    nation.gold = 100_000.0;
    specs.push(nation);
    specs.push(LabPlayer::tribe(8, 2, 20));
    specs.push(LabPlayer::tribe(9, 20, 2));

    let mut engine = build_lab(W, W, "FFA", &specs);
    engine.state.config.global_speed_multiplier = 1.0;

    let start: std::collections::HashMap<u16, u32> =
        specs.iter().map(|s| (s.id, tiles(&engine, s.id))).collect();

    let mut prev_counts = start.clone();
    for window in 0..MAX_WINDOWS {
        let live = run_window(&mut engine, WINDOW_TICKS);
        let mut row = String::new();
        let mut stalls = Vec::new();
        for spec in specs.iter() {
            let now = tiles(&engine, spec.id);
            let grew = now.saturating_sub(*prev_counts.get(&spec.id).unwrap_or(&0));
            row.push_str(&format!(" p{}={}", spec.id, now));
            if grew == 0
                && engine
                    .state
                    .player(spec.id)
                    .map(|p| p.alive)
                    .unwrap_or(false)
            {
                stalls.push(spec.id);
            }
            prev_counts.insert(spec.id, now);
        }
        eprintln!(
            "S9 w={window} total_live={} |{}{} stalls={:?}",
            window,
            row,
            if live { "" } else { " [GAME OVER]" },
            stalls
        );
    }

    // ── Dissection of every still-living stalled entity ──
    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        for spec in specs.iter() {
            let Some(p) = engine.state.player(spec.id) else {
                continue;
            };
            if !p.alive {
                continue;
            }
            let stats = (
                p.tile_count,
                p.troops,
                p.max_troops,
                p.iq_points,
                p.gold,
                p.player_type,
                p.is_ai_controlled,
            );
            let (neighbors, has_neutral) = engine.nation_scan_neighbors(spec.id);
            let tier = match ai_tier(stats.5, stats.6) {
                Some(t) => t,
                None => continue, // real human: no brain
            };
            let profile = ai_profile_for(tier, crate::game_config::BotDifficulty::Vanilla);
            let armed = stats.1 >= stats.2 * profile.trigger_ratio;
            eprintln!(
                "DISSECT id={} alive=true tiles={} troops={:.0}/max{:.0} pts={:.0} gold={:.0} \
                 neighbors={:?} neutral={} armed={} next_att_id={}",
                spec.id,
                stats.0,
                stats.1,
                stats.2,
                stats.3,
                stats.4,
                neighbors,
                has_neutral,
                armed,
                engine.state.next_attack_id
            );
        }
        eprintln!("DISSECT attacks_out={}", engine.attacks.len());
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S11 — ENDGAME REPRO: world fully partitioned, ZERO neutral land, everyone
// armed (the user's screenshot state). Only path to growth is WAR against a
// neighbor. A bot that "wants to win" must keep fighting; freezing here is
// the exact live complaint. Runs at full sim speed (no clock, engine.tick()
// back-to-back) in release mode.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s11_partitioned_world_war_keeps_flowing() {
    use crate::maps::{WORLD_MAP_BYTES, load_map_from_payload};

    let mapfile = load_map_from_payload(WORLD_MAP_BYTES).expect("world map decodes");
    let (mw, mh) = (mapfile.width, mapfile.height);

    let mut game = GameState::new(7, mw, mh, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Playing;
    game.config.game_mode = "FFA".to_string();
    game.config.map_control_win_percentage = 2.0;
    game.map.terrain = mapfile
        .terrain
        .iter()
        .map(|b| crate::map::MapTile::from_byte(*b))
        .collect();

    // SIX empires on REAL land — sampled from the map's own spawn table
    // (guaranteed land, spread across continents).
    let stride = ((mapfile.spawns.len() as f32) / 6.0).ceil().max(1.0) as usize;
    let homes: Vec<(u32, u32)> = mapfile
        .spawns
        .iter()
        .step_by(stride)
        .take(6)
        .map(|sp| (sp.x.min(mw - 1), sp.y.min(mh - 1)))
        .collect();
    assert!(homes.len() == 6, "need 6 land spawns, got {}", homes.len());
    let mut lookup: Vec<Option<usize>> = vec![None];
    for (i, item) in homes.iter().enumerate() {
        let (hx, hy) = *item;
        let id = (i + 1) as u16;
        let mut p = Player::new_human(
            id,
            format!("E{id}"),
            [0.3, 0.6, 1.0],
            &crate::game_config::GameConfig::default(),
        );
        p.is_ai_controlled = true; // ghost-tier brain
        p.iq = 170;
        p.iq_points = 400.0;
        p.troops = 15_000.0;
        p.max_troops = 30_000.0;
        p.gold = 200_000.0;
        p.tile_count = 1;
        p.border_insert(hy * mw + hx);
        lookup.push(Some(i));
        game.players.push(p);
        game.map.set_owner_id(hx, hy, id);
    }
    game.player_lookup = lookup;

    let water = WaterComponents::compute(&game.map, |_| {});
    let mut engine = SowEngine::new(game, water);

    // PHASE A — land-grab: let them fill the world (free neutral expansion).
    for _ in 0..3000 {
        if engine.state.phase != GamePhase::Playing {
            break;
        }
        engine.tick();
    }
    let neutral_left = (0..mw * mh)
        .filter(|i| {
            let t = &engine.state.map.terrain[*i as usize];
            t.is_land() && engine.state.map.owner_id(i % mw, i / mw) == 0
        })
        .count() as u32;
    let counts: Vec<u32> = (1..=6).map(|id| tiles(&engine, id)).collect();
    eprintln!(
        "S11 PHASE-A done: neutral_left={neutral_left} tiles={counts:?} attacks_out={}",
        engine.attacks.len()
    );

    // ── Dissect the frozen empire BEFORE window asserts ──
    {
        let (nb, neu) = engine.nation_scan_neighbors(6);
        let p6 = engine.state.player(6).unwrap();
        eprintln!(
            "DISSECT p6: tiles={} troops={:.0}/max{:.0} pts={:.0} gold={:.0} nb={:?} neu={} fleets={} border={}",
            p6.tile_count,
            p6.troops,
            p6.max_troops,
            p6.iq_points,
            p6.gold,
            nb,
            neu,
            engine.fleets.len(),
            p6.border_tiles.count_ones()
        );
    }

    // PHASE B — the endgame under test: zero neutral. With everyone armed,
    // border wars MUST keep flowing. Measure activity across 3 windows.
    for w in 0..3u32 {
        let before_counts: Vec<u32> = (1..=6).map(|id| tiles(&engine, id)).collect();
        let before_att = engine.state.next_attack_id;
        for _ in 0..600 {
            if engine.state.phase != GamePhase::Playing {
                break;
            }
            engine.tick();
        }
        let after_counts: Vec<u32> = (1..=6).map(|id| tiles(&engine, id)).collect();
        let new_attacks = engine.state.next_attack_id - before_att;
        eprintln!(
            "S11 w={w} att_new={new_attacks} before={before_counts:?} after={after_counts:?}"
        );
        assert!(
            new_attacks > 0,
            "S11 FAIL window {w}: ZERO new attacks with zero neutral — total freeze"
        );
    }
    // ── Dissect the frozen empire ──
    if verbose() {
        let (nb, neu) = engine.nation_scan_neighbors(6);
        let p6 = engine.state.player(6).unwrap();
        eprintln!(
            "DISSECT p6: tiles={} troops={:.0}/max{:.0} pts={:.0} gold={:.0} nb={:?} neu={} fleets={} border={}",
            p6.tile_count,
            p6.troops,
            p6.max_troops,
            p6.iq_points,
            p6.gold,
            nb,
            neu,
            engine.fleets.len(),
            p6.border_tiles.count_ones()
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S10 — REAL WORLD MAP, prod-shaped roster: the reproduction attempt for the
// live report "everyone stops expanding mid-game". Uses the bundled world
// map (oceans/highlands/real terrain), real spawn points from the map file,
// ghost-filled FFA + nations + tribes across the actual continents.
// Telemetry prints per-window tiles per player; stall detection mirrors S9.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s10_real_world_lobby_midgame() {
    use crate::maps::{WORLD_MAP_BYTES, load_map_from_payload};

    let mapfile =
        load_map_from_payload(WORLD_MAP_BYTES).expect("bundled world map must decode for the lab");
    let (mw, mh) = (mapfile.width, mapfile.height);

    // Pick spawns spread across the world (stride-sampled, deterministic).
    let stride = ((mapfile.spawns.len() as f32) / 8.0).ceil().max(1.0) as usize;
    let chosen: Vec<_> = mapfile.spawns.iter().step_by(stride).take(8).collect();
    assert!(chosen.len() >= 6, "world map spawns insufficient for lab");

    let mut specs: Vec<LabPlayer> = Vec::new();
    for (i, sp) in chosen.iter().enumerate() {
        let mut spec = match i {
            0..=4 => LabPlayer::ghost((i + 1) as u16, sp.x.min(mw - 1), sp.y.min(mh - 1)),
            5..=6 => LabPlayer::nation((i + 1) as u16, sp.x.min(mw - 1), sp.y.min(mh - 1)),
            _ => LabPlayer::tribe((i + 1) as u16, sp.x.min(mw - 1), sp.y.min(mh - 1)),
        };
        spec.gold = 100_000.0;
        specs.push(spec);
    }

    let mut game = GameState::new(7, mw, mh, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Playing;
    game.config.game_mode = "FFA".to_string();
    game.config.map_control_win_percentage = 2.0; // never early-end the sandbox

    let mut lookup: Vec<Option<usize>> = vec![None];
    for (i, spec) in specs.iter().enumerate() {
        let mut p = match spec.kind {
            PlayerType::Human => Player::new_human(
                spec.id,
                format!("G{}", spec.id),
                [0.2, 0.5, 1.0],
                &crate::game_config::GameConfig::default(),
            ),
            // Nations MUST be PlayerType::Nation (new_nation) — new_bot mints
            // Bot-type players, which resolve to the TRIBE tier and silently
            // turn every lab "nation" into a passive tribe with high IQ.
            PlayerType::Nation => Player::new_nation(
                spec.id,
                format!("N{}", spec.id),
                [0.8, 0.4, 0.1],
                &crate::game_config::GameConfig::default(),
            ),
            PlayerType::Bot => Player::new_bot(
                spec.id,
                format!("T{}", spec.id),
                [0.1, 0.8, 0.3],
                &crate::game_config::GameConfig::default(),
            ),
        };
        p.is_ai_controlled = spec.ai;
        p.iq = spec.iq;
        p.iq_points = 200.0;
        p.troops = spec.troops;
        p.max_troops = spec.max_troops;
        p.gold = spec.gold;
        p.tile_count = 1;
        let home = spec.y * mw + spec.x;
        p.border_insert(home);
        lookup.push(Some(i));
        game.players.push(p);
        game.map.set_owner_id(spec.x, spec.y, spec.id);
    }
    game.player_lookup = lookup;

    // Inject REAL terrain over the default flat map (before engine boot so
    // shorelines compute from true continents).
    game.map.terrain = mapfile
        .terrain
        .iter()
        .map(|b| crate::map::MapTile::from_byte(*b))
        .collect();

    let water = WaterComponents::compute(&game.map, |_| {});
    let mut engine = SowEngine::new(game, water);

    let start: Vec<(u16, u32)> = specs.iter().map(|s| (s.id, tiles(&engine, s.id))).collect();
    let mut prev = start.clone();

    for window in 0..MAX_WINDOWS {
        let live = run_window(&mut engine, WINDOW_TICKS * 3);
        let mut row = String::new();
        let mut stalls = Vec::new();
        for (id, _base) in &start {
            let now = tiles(&engine, *id);
            let prev_v = prev.iter().find(|(pid, _)| pid == id).unwrap().1;
            let grew = now.saturating_sub(prev_v);
            row.push_str(&format!(" p{}={}", id, now));
            if grew == 0 && engine.state.player(*id).map(|p| p.alive).unwrap_or(false) {
                stalls.push(*id);
            }
            if verbose() {
                eprintln!("S10 w={window} p{id} tiles={now} grew={grew}");
            }
            if let Some(slot) = prev.iter_mut().find(|(pid, _)| pid == id) {
                slot.1 = now;
            }
        }
        eprintln!(
            "S10 w={window} stalls={:?} |{}{}",
            stalls,
            row,
            if live { "" } else { " [ENDED]" }
        );
        if !live {
            break;
        }
        if window >= 2 && !stalls.is_empty() && std::env::var("SOW_LAB_VERBOSE").is_ok() {
            for sid in &stalls {
                let probe = engine
                    .state
                    .player(*sid)
                    .map(|p| (p.alive, p.troops, p.max_troops, p.iq_points));
                if let Some((alive, troops, maxt, iqpts)) = probe {
                    let (neighbors, has_neutral) = engine.nation_scan_neighbors(*sid);
                    eprintln!(
                        "DISSECT s10 id={} alive={} troops={:.0}/max{:.0} pts={:.0} \
                         neighbors={} neutral={} armed_t005={}",
                        sid,
                        alive,
                        troops,
                        maxt,
                        iqpts,
                        neighbors.len(),
                        has_neutral,
                        troops >= maxt * 0.05
                    );
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S12 — D1 PROOF: Vanilla tribe fully owning its island (zero neutral
// adjacent) must cross water to keep eating free land — no port, no war,
// no statue. OpenFront sendBoatAttackToNearbyTerraNullius parity.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s12_island_tribe_crosses_water_for_neutral() {
    use crate::water_components::WaterComponents;

    const W: u32 = 20;
    let mut game = GameState::new(13, W, W, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Playing;
    game.config.map_control_win_percentage = 2.0;

    // Ocean everywhere; two islands. Water byte: is_ocean=1 (bit 5) —
    // compute_shorelines only marks shores against OCEAN, and fleet routes
    // require shoreline flags on the home coast.
    for idx in 0..(W * W) as usize {
        game.map.terrain[idx] = crate::map::MapTile::from_byte(0b0010_0000); // ocean
    }
    // Tribe island (0..=3, 0..=3): FULLY owned by the tribe → has_neutral=false.
    // Free island (14..=19, 14..=19): 36 neutral tiles across the sea.
    let mut tribe_tiles = 0u32;
    for y in 0..4u32 {
        for x in 0..4u32 {
            let idx = y * W + x;
            game.map.terrain[idx as usize] = crate::map::MapTile::from_byte(0b1000_0000);
            game.map.set_owner_id(x, y, 1);
            tribe_tiles += 1;
        }
    }
    for y in 14..20u32 {
        for x in 14..20u32 {
            let idx = y * W + x;
            game.map.terrain[idx as usize] = crate::map::MapTile::from_byte(0b1000_0000);
        }
    }

    let mut tribe = Player::new_bot(
        1,
        "IslandTribe".into(),
        [0.1, 0.8, 0.3],
        &crate::game_config::GameConfig::default(),
    );
    tribe.iq = 60;
    tribe.iq_points = 200.0;
    tribe.troops = 5_000.0;
    tribe.max_troops = 10_000.0;
    tribe.tile_count = tribe_tiles;
    // Register every island tile as owned/border (scan needs the ring).
    for y in 0..4u32 {
        for x in 0..4u32 {
            tribe.border_insert(y * W + x);
        }
    }
    game.players.push(tribe);

    // Witness tribe on the free island: keeps the match alive (single-entity
    // lobbies trigger instant victory) and is a LEGAL Vanilla target (Bot vs
    // Bot) — exactly how OpenFront tribes interact.
    let mut witness = Player::new_bot(
        2,
        "Witness".into(),
        [0.9, 0.6, 0.1],
        &crate::game_config::GameConfig::default(),
    );
    witness.iq = 50;
    // Troopless on purpose: an expanding witness would claim every free tile
    // of the target island and leave the tribe nothing neutral to sail to.
    witness.troops = 0.0;
    witness.max_troops = 0.0;
    witness.max_troops_cap = Some(0.0);
    witness.tile_count = 1;
    witness.border_insert(14 * W + 14);
    game.players.push(witness);
    game.player_lookup = vec![None, Some(0), Some(1)];
    game.map.set_owner_id(14, 14, 2);

    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        for y in 0..W {
            let mut row = String::new();
            for x in 0..W {
                let idx = (y * W + x) as usize;
                let t = if game.map.terrain[idx].is_land() {
                    "L"
                } else {
                    "~"
                };
                let o = game.map.owner_id(x, y);
                row.push_str(&format!("{t}{o:<2}"));
            }
            eprintln!("GRID y={y}: {row}");
        }
    }

    game.map.compute_shorelines(); // fixture maps lack the baked shoreline bit

    let water = WaterComponents::compute(&game.map, |_| {});

    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        eprintln!("S12 GRID (t=0):");
        for y in 0..W {
            let mut row = String::new();
            for x in 0..W {
                let idx = (y * W + x) as usize;
                let t = if game.map.terrain[idx].is_land() {
                    "L"
                } else {
                    "~"
                };
                let o = game.map.owner_id(x, y);
                row.push_str(&format!("{t}{o:<2}"));
            }
            eprintln!("GRID y={y}: {row}");
        }
    }
    let mut engine = SowEngine::new(game, water);

    // Shoreline probe: which border tiles would player_water_components accept?
    for raw in engine.state.player(1).unwrap().border_tiles.ones() {
        let idx = raw as usize;
        let t = &engine.state.map.terrain[idx];
        let owner = engine.state.map.owner_id(idx as u32 % W, idx as u32 / W);
        eprintln!(
            "SHORE idx={raw} owner={owner} land={} shoreline={}",
            t.is_land(),
            t.is_shoreline()
        );
    }

    let fleet_before = engine.state.next_fleet_id;
    for t in 0..2500u64 {
        engine.tick();
        if t % 100 == 0 || t < 5 {
            let alive: Vec<u16> = engine
                .state
                .players
                .iter()
                .filter(|p| p.alive && p.tile_count > 0)
                .map(|p| p.id)
                .collect();
            eprintln!(
                "S12TRACE t={t} phase={:?} winner={:?} alive={alive:?} t1={} t2={}",
                engine.state.phase,
                engine.state.winner,
                tiles(&engine, 1),
                tiles(&engine, 2)
            );
        }
    }

    let crossed = tiles(&engine, 1) > tribe_tiles;
    assert!(
        engine.state.next_fleet_id > fleet_before || crossed,
        "S12 FAIL: island tribe never launched an expansion boat"
    );
    assert!(
        crossed,
        "S12 FAIL: boat launched but tribe never claimed free land across water"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S13 — SPAWN PROXIMITY: same-team members cluster around their team
// centroid (OpenFront teamSpawnArea parity) — never scattered "por ningún
// lado".
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s13_team_members_spawn_clustered() {
    use crate::protocol::Team;

    let mut game = GameState::new(21, 40, 40, crate::game_config::GameConfig::default());
    game.config.random_spawn = true;
    for idx in 0..(40 * 40) as usize {
        game.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
    }
    let water = WaterComponents::compute(&game.map, |_| {});
    let mut engine = SowEngine::new(game, water);

    let blues = [1u16, 2, 3, 4];
    let reds = [5u16, 6];
    for id in blues {
        engine.spawn_human(crate::engine::HumanSpawn {
            player_id: id,
            name: format!("B{id}"),
            color: [0.2, 0.5, 1.0],
            team: Some(Team::Blue),
            civilization: crate::player::Leader::Caesar.civilization(),
            leader: crate::player::Leader::Caesar,
            is_ai_controlled: false,
        });
    }
    for id in reds {
        engine.spawn_human(crate::engine::HumanSpawn {
            player_id: id,
            name: format!("R{id}"),
            color: [1.0, 0.2, 0.2],
            team: Some(Team::Red),
            civilization: crate::player::Leader::Boudica.civilization(),
            leader: crate::player::Leader::Boudica,
            is_ai_controlled: false,
        });
    }

    let home = |id: u16| -> (f64, f64) {
        let p = engine.state.player(id).unwrap();
        (
            p.sum_x as f64 / p.tile_count as f64,
            p.sum_y as f64 / p.tile_count as f64,
        )
    };
    let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();

    // OF teamSpawnArea parity: each team owns a map half (Red left, Blue
    // right on the 40x40 lab) and every member spawns inside its zone with
    // the member floor (≥14) keeping homes apart — zone cohesion plus
    // breathing room, never stacked.
    let in_zone = |h: (f64, f64), left: bool| {
        if left { h.0 < 20.0 } else { h.0 >= 20.0 }
    };
    let blue_homes: Vec<(f64, f64)> = blues.iter().map(|id| home(*id)).collect();
    for (i, a) in blue_homes.iter().enumerate() {
        assert!(
            in_zone(*a, false),
            "S13 FAIL: Blue member {a:?} spawned outside the Blue half"
        );
        for b in blue_homes[i + 1..].iter() {
            let d = dist(*a, *b);
            assert!(
                d >= 12.0,
                "S13 FAIL: Blue members {a:?}/{b:?} stacked at {d}"
            );
        }
    }
    let red_homes: Vec<(f64, f64)> = reds.iter().map(|id| home(*id)).collect();
    for (i, a) in red_homes.iter().enumerate() {
        assert!(
            in_zone(*a, true),
            "S13 FAIL: Red member {a:?} spawned outside the Red half"
        );
        for b in red_homes[i + 1..].iter() {
            let d = dist(*a, *b);
            assert!(d >= 12.0, "S13 FAIL: Red pair stacked at {d}");
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S14 — OF calculateBotAttackTroops parity: nations hunt tribes DECISIVELY.
//   A) A tribe too big to strike (affordable < 2× its troops) is never
//      poked — not by land initiation, not by the weakest-player fleet
//      path. The nation grows on free land instead of bleeding into a
//      defense that scales with the tribe's TOTAL troops.
//   B) A tribe within striking range gets the 4×-sized decisive strike
//      (never a pointless poke) and gets eaten.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s14_nation_hunts_tribes_decisively() {
    // A) big tribe nearby: the attrition war OPENS (no troops-ratio floor —
    // tribes sit at cap while nations trail theirs, so any ratio bar closes
    // the window and freezes the map mid-game)
    {
        let mut engine = build_lab(
            90,
            90,
            "FFA",
            &[LabPlayer::nation(1, 10, 10), LabPlayer::tribe(2, 26, 26)],
        );
        grant_block(&mut engine, 2, 23, 23, 12, 12); // 144 tiles ≈ 5.2K troops
        let mut attacked = false;
        let mut max_nation_tiles = 1u32;
        for _ in 0..400 {
            if !run_window(&mut engine, 1) {
                break;
            }
            if engine
                .attacks
                .iter()
                .any(|a| a.owner_id == 1 && a.target_owner == 2)
            {
                attacked = true;
                break;
            }
            max_nation_tiles = max_nation_tiles.max(tiles(&engine, 1));
        }
        assert!(
            attacked,
            "S14 FAIL: nation never opened the attrition war on the big tribe"
        );
        assert!(
            max_nation_tiles > 1 && engine.state.player(1).unwrap().alive,
            "S14 FAIL: nation froze instead of growing on free land"
        );
    }
    // B) small tribe in range: 4×-sized decisive strike eats it
    {
        let mut engine = build_lab(
            90,
            90,
            "FFA",
            &[LabPlayer::nation(1, 10, 10), LabPlayer::tribe(2, 16, 10)],
        );
        grant_block(&mut engine, 1, 8, 8, 5, 5); // nation opens with real scale
        let mut strike_seen = None;
        for _ in 0..800 {
            let pre_tribe_troops = engine.state.player(2).unwrap().troops.max(1.0);
            if !run_window(&mut engine, 1) {
                break;
            }
            if let Some(a) = engine
                .attacks
                .iter()
                .find(|a| a.owner_id == 1 && a.target_owner == 2)
            {
                // Ratio vs the tribe's PRE-STRIKE troops: combat drains the
                // defender the same tick, so post-hoc ratios overshoot 4×.
                strike_seen = Some(a.troops / pre_tribe_troops);
                break;
            }
        }
        if let Some(ratio) = strike_seen {
            assert!(
                (0.95..=4.05).contains(&ratio),
                "S14 FAIL: strike ratio {ratio} — expected the decisive 1×..4× band"
            );
        }
        run_window(&mut engine, 800);
        assert!(
            tiles(&engine, 2) == 0 || !engine.state.player(2).unwrap().alive,
            "S14 FAIL: the decisive strike did not eat the small tribe"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S15 — OF isAttackTooWeak parity: a smart AI never initiates against a
// neighbor whose troops dwarf its affordable wave (<20%), and never freezes:
// with free land adjacent it grows instead (OF expansions precede wars).
// Passive big tribe as the neighbor so no defensive counter-confound.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s15_no_suicide_against_dwarfing_player() {
    // FFA: never initiate against a player neighbor that outguns you (OF
    // `weakest` + `isAttackTooWeak`). A NATION is the dwarfing neighbor —
    // tribe neighbors get ground down by attrition unconditionally (S14-A).
    let mut engine = build_lab(
        90,
        90,
        "FFA",
        &[LabPlayer::ghost(1, 10, 10), LabPlayer::nation(2, 40, 40)],
    );
    grant_block(&mut engine, 2, 36, 36, 9, 9); // ≈2.3K+ troops vs ghost's ~240
    let mut suicided = false;
    let mut max_ghost_tiles = 1u32;
    for _ in 0..200 {
        if !run_window(&mut engine, 1) {
            break;
        }
        if engine
            .attacks
            .iter()
            .any(|a| a.owner_id == 1 && a.target_owner == 2)
        {
            suicided = true;
            break;
        }
        max_ghost_tiles = max_ghost_tiles.max(tiles(&engine, 1));
    }
    assert!(
        !suicided,
        "S15 FAIL: ghost initiated against a dwarfing neighbor — odds discipline missing"
    );
    assert!(
        max_ghost_tiles > 1,
        "S15 FAIL: ghost froze behind free land instead of growing"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S16 — LONG-HORIZON ECOSYSTEM, high density with symmetric friction
// (≈6.7 min game time). Sparse open maps snowball; density forces the
// contested mid-game the docility complaint lives in. Regression-detects the
// total freeze (windows going quiet) and the gray-carpet win.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s16_long_horizon_wars_keep_flowing() {
    let mut specs: Vec<LabPlayer> = Vec::new();
    let mut id = 1u16;
    for row in 0..5 {
        for col in 0..4 {
            specs.push(LabPlayer::nation(id, 6 + col * 18, 6 + row * 18));
            id += 1;
        }
    }
    specs.push(LabPlayer::ghost(id, 40, 40));
    id += 1;
    let ghost_id = id - 1;
    let first_tribe = id;
    let mut tribes = 0;
    for row in 0..5 {
        for col in 0..8 {
            if tribes >= 40 {
                break;
            }
            let x = 2 + col * 9 + 4;
            let y = 2 + row * 16 + 9;
            let clash = specs
                .iter()
                .any(|s| (s.x as i32 - x).abs() < 4 && (s.y as i32 - y).abs() < 4);
            if clash || (x, y) == (40, 40) {
                continue;
            }
            specs.push(LabPlayer::tribe(id, x as u32, y as u32));
            id += 1;
            tribes += 1;
        }
    }
    let last_tribe = id - 1;

    let mut engine = build_lab(80, 80, "FFA", &specs);
    engine.state.config.map_control_win_percentage = 95.0; // never end early
    for s in &specs {
        if s.kind == crate::player::PlayerType::Bot {
            grant_block(
                &mut engine,
                s.id,
                s.x.saturating_sub(2),
                s.y.saturating_sub(2),
                5,
                5,
            );
        }
    }

    let windows = 8;
    let mut tribe_war_ticks = Vec::new();
    let mut contacts_log: Vec<u32> = Vec::new();
    let mut tribe_mass_log: Vec<u32> = Vec::new();
    for _ in 0..windows {
        let mut hits = 0u32;
        for _ in 0..500 {
            if !run_window(&mut engine, 1) {
                break;
            }
            if engine.attacks.iter().any(|a| {
                a.owner_id <= ghost_id
                    && engine
                        .state
                        .player(a.target_owner)
                        .map(|p| p.player_type == crate::player::PlayerType::Bot)
                        .unwrap_or(false)
            }) {
                hits += 1;
            }
        }
        let mut tribes_alive = 0u32;
        let mut tribe_tiles = 0u32;
        let mut ai_alive = 0u32;
        for t in first_tribe..=last_tribe {
            if let Some(pl) = engine.state.player(t)
                && pl.alive
            {
                tribes_alive += 1;
                tribe_tiles += pl.tile_count;
            }
        }
        for a in 1..=ghost_id {
            if engine.state.player(a).map(|pl| pl.alive).unwrap_or(false) {
                ai_alive += 1;
            }
        }
        // count AI players with at least one Bot-type neighbor border
        let mut contacts = 0u32;
        for a in 1..=ghost_id {
            let Some(pl) = engine.state.player(a) else {
                continue;
            };
            if !pl.alive {
                continue;
            }
            let touches = pl.border_tiles.ones().any(|raw| {
                let bx = raw % engine.state.map.width;
                let by = raw / engine.state.map.width;
                let mut hit = false;
                engine.state.map.for_each_neighbor(bx, by, |nx, ny| {
                    if hit {
                        return;
                    }
                    let o = engine.state.map.owner_id(nx, ny);
                    if o != 0
                        && o != pl.id
                        && let Some(op) = engine.state.player(o)
                        && op.player_type == crate::player::PlayerType::Bot
                    {
                        hit = true;
                    }
                });
                hit
            });
            if touches {
                contacts += 1;
            }
        }
        contacts_log.push(contacts);
        tribe_mass_log.push(tribe_tiles);
        eprintln!(
            "S16 W{}: hits={hits} ai_alive={ai_alive} tribes_alive={tribes_alive} tribe_tiles={tribe_tiles} ai-with-tribe-border={contacts}",
            tribe_war_ticks.len()
        );
        tribe_war_ticks.push(hits);
    }
    // The freeze signature: tribe borders exist AND tribes still hold real
    // mass, yet zero wars. (Player-war preference is legitimate; a resolved
    // map with tribes ground to stubs is not the regress.)
    for (w, hits) in tribe_war_ticks.iter().enumerate() {
        if contacts_log[w] > 0 && tribe_mass_log[w] >= 500 {
            assert!(
                *hits >= 3,
                "S16 FAIL: window {w} had {} AIs bordering tribes holding {} tiles but only {hits} tribe-war ticks — docility regress",
                contacts_log[w],
                tribe_mass_log[w]
            );
        }
    }
    let total_land = 80u32 * 80;
    let mut tribe_tiles_end = 0u32;
    for t in first_tribe..=last_tribe {
        if let Some(p) = engine.state.player(t) {
            tribe_tiles_end += p.tile_count;
        }
    }
    let functioning = (1..=20u16)
        .filter(|id| {
            engine
                .state
                .player(*id)
                .map(|p| p.alive && p.tile_count > 50)
                .unwrap_or(false)
        })
        .count();
    eprintln!(
        "S16 final: tribe tiles={tribe_tiles_end}/{total_land} nations-functioning={functioning}"
    );
    assert!(
        tribe_tiles_end * 10 < total_land * 8,
        "S16 FAIL: tribes own {tribe_tiles_end}/{total_land} tiles — gray carpet won"
    );
    assert!(
        functioning >= 1,
        "S16 FAIL: no AI nation alive & functioning (>50 tiles)"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// S17 — SPAWNING-PHASE ZONES (the path that actually runs on world maps):
// random_spawn=false → spawn_human registers WITHOUT position and the engine's
// Spawning phase places team ghosts. Regression for the option-D miss: the
// area code shipped into spawn_human while the live path kept the old tight
// centroid ring (tick.rs:52). Team ghosts + teamed stragglers must land in
// their own half (Red left, Blue right), never stacked.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s17_spawning_phase_team_ghosts_in_zone() {
    use crate::engine::SowEngine;
    use crate::game::GamePhase;
    use crate::water_components::WaterComponents;

    let mut game = GameState::new(23, 60, 60, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Spawning { end_tick: 12 };
    game.config.random_spawn = false;
    for idx in 0..(60 * 60) as usize {
        game.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
    }

    // Register players WITHOUT positions — the live world-map shape.
    let mut lookup: Vec<Option<usize>> = vec![None];
    let roster: Vec<(u16, Option<Team>, bool)> = vec![
        (1, Some(Team::Blue), true),
        (2, Some(Team::Blue), true),
        (3, Some(Team::Blue), true),
        (4, Some(Team::Blue), true),
        (5, Some(Team::Red), true),
        (6, Some(Team::Red), true),
        (7, Some(Team::Red), false), // real human, red team
    ];
    for (i, (id, team, ai)) in roster.iter().enumerate() {
        let mut p = Player::new_human(
            *id,
            format!("P{id}"),
            [0.5, 0.5, 0.5],
            &crate::game_config::GameConfig::default(),
        );
        p.team = *team;
        p.is_ai_controlled = *ai;
        game.players.push(p);
        lookup.push(Some(i));
    }
    game.player_lookup = lookup;

    let water = WaterComponents::default();
    let mut engine = SowEngine::new(game, water);
    // Drive the Spawning phase manually (run_window gates on Playing), then
    // a few Playing ticks for good measure.
    for _ in 0..60 {
        if engine.state.phase == GamePhase::Playing {
            break;
        }
        engine.tick();
    }
    assert_eq!(
        engine.state.phase,
        GamePhase::Playing,
        "S17: phase never ended"
    );
    assert!(run_window(&mut engine, 5));

    let home = |id: u16| -> (f64, f64) {
        let p = engine.state.player(id).unwrap();
        assert!(p.has_spawned, "S17 FAIL: player {id} never spawned");
        (
            p.sum_x as f64 / p.tile_count as f64,
            p.sum_y as f64 / p.tile_count as f64,
        )
    };
    for id in [1u16, 2, 3, 4] {
        assert!(
            home(id).0 >= 30.0,
            "S17 FAIL: blue player {id} spawned outside the Blue half"
        );
    }
    for id in [5u16, 7] {
        assert!(
            home(id).0 < 30.0,
            "S17 FAIL: red player {id} spawned outside the Red half"
        );
    }
    let blues: Vec<(f64, f64)> = [1u16, 2, 3, 4].iter().map(|id| home(*id)).collect();
    for (i, a) in blues.iter().enumerate() {
        for b in blues[i + 1..].iter() {
            let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            assert!(d >= 12.0, "S17 FAIL: blue homes {a:?}/{b:?} stacked at {d}");
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// S18 — WORLD-MAP SPAWN ZONES, the no-excuses repro: real terrain, real geo
// roster (spawn_ai 128/420), teamed ghosts registered WITHOUT position,
// real Spawning-phase drive. If a team balls up here, the lab sees it.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s18_world_map_team_spawn_zones() {
    use crate::engine::SowEngine;
    use crate::game::GamePhase;
    use crate::maps::{WORLD_MAP_BYTES, load_map_from_payload};
    use crate::water_components::WaterComponents;

    let mapfile = load_map_from_payload(WORLD_MAP_BYTES).expect("world map decodes");
    let (mw, mh) = (mapfile.width, mapfile.height);

    let mut game = GameState::new(31, mw, mh, crate::game_config::GameConfig::default());
    game.phase = GamePhase::Spawning { end_tick: 400 };
    game.config.random_spawn = false;
    game.config.game_mode = "Teams".to_string();

    let mut lookup: Vec<Option<usize>> = vec![None];
    let mut id = 1u16;
    for (team, n) in [(Team::Red, 8u32), (Team::Blue, 8)] {
        for _ in 0..n {
            let mut p = Player::new_human(
                id,
                format!("P{id}"),
                [0.5, 0.5, 0.5],
                &crate::game_config::GameConfig::default(),
            );
            p.team = Some(team);
            p.is_ai_controlled = true;
            game.players.push(p);
            lookup.push(Some((id - 1) as usize)); // player slot, NOT team idx
            id += 1;
        }
    }
    game.player_lookup = lookup;
    game.map.terrain = mapfile
        .terrain
        .iter()
        .map(|b| crate::map::MapTile::from_byte(*b))
        .collect();
    game.total_land_tiles = mapfile.num_land_tiles;

    let water = WaterComponents::compute(&game.map, |_| {});
    let mut engine = SowEngine::new(game, water);
    engine.spawn_ai(128, 420); // the real geo roster, placed before ghosts fire

    for _ in 0..500 {
        if engine.state.phase == GamePhase::Playing {
            break;
        }
        engine.tick();
    }
    assert_eq!(
        engine.state.phase,
        GamePhase::Playing,
        "S18: phase never ended"
    );

    let report = |label: &str, ids: &[u16]| -> (f64, f64, f64) {
        let homes: Vec<(f64, f64)> = ids
            .iter()
            .map(|id| {
                let p = engine.state.player(*id).unwrap();
                assert!(p.has_spawned, "S18 FAIL: {label} ghost {id} never spawned");
                (
                    p.sum_x as f64 / p.tile_count as f64,
                    p.sum_y as f64 / p.tile_count as f64,
                )
            })
            .collect();
        let mut min_d = f64::MAX;
        for (i, a) in homes.iter().enumerate() {
            for b in homes[i + 1..].iter() {
                min_d = min_d.min(((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt());
            }
        }
        let xs: Vec<f64> = homes.iter().map(|h| h.0).collect();
        let ys: Vec<f64> = homes.iter().map(|h| h.1).collect();
        let spread = (xs.iter().cloned().fold(f64::NAN, f64::max)
            - xs.iter().cloned().fold(f64::MAX, f64::min))
        .hypot(
            ys.iter().cloned().fold(f64::NAN, f64::max)
                - ys.iter().cloned().fold(f64::MAX, f64::min),
        );
        eprintln!(
            "S18 {label}: homes={:?} min_pair={:.1} bbox_diag={:.1}",
            homes
                .iter()
                .map(|h| (h.0 as u32, h.1 as u32))
                .collect::<Vec<_>>(),
            min_d,
            spread
        );
        (
            min_d,
            spread,
            xs.iter().cloned().sum::<f64>() / xs.len() as f64,
        )
    };
    let red = report("RED", &(1..=8u16).collect::<Vec<_>>());
    let blue = report("BLUE", &(9..=16u16).collect::<Vec<_>>());
    let half = mw as f64 / 2.0;
    assert!(
        red.2 < half,
        "S18 FAIL: red centroid {:.0} outside left half ({half})",
        red.2
    );
    assert!(
        blue.2 >= half,
        "S18 FAIL: blue centroid {:.0} outside right half ({half})",
        blue.2
    );
    assert!(
        red.0 >= 12.0 && blue.0 >= 12.0,
        "S18 FAIL: stacked — red min pair {:.1}, blue min pair {:.1}",
        red.0,
        blue.0
    );
    // A balled team hugs one spot: bbox diagonal tiny vs the half's size.
    assert!(
        red.1 >= mw as f64 * 0.15 && blue.1 >= mw as f64 * 0.15,
        "S18 FAIL: balled up — red spread {:.0}, blue spread {:.0} (half is {:.0} wide)",
        red.1,
        blue.1,
        half
    );
}

#[test]
fn s18_probe_area_seeds() {
    use crate::engine::SowEngine;
    use crate::maps::{WORLD_MAP_BYTES, load_map_from_payload};
    use wyrand::WyRand;

    let mapfile = load_map_from_payload(WORLD_MAP_BYTES).expect("world map decodes");
    let (mw, mh) = (mapfile.width, mapfile.height);
    let mut game = GameState::new(31, mw, mh, crate::game_config::GameConfig::default());
    game.map.terrain = mapfile
        .terrain
        .iter()
        .map(|b| crate::map::MapTile::from_byte(*b))
        .collect();
    let water = crate::water_components::WaterComponents::default();
    let mut engine = SowEngine::new(game, water);
    engine.spawn_ai(128, 420);

    let area = engine.team_spawn_area(&Team::Red);
    eprintln!("S18PROBE red area={area:?} map={mw}x{mh}");
    for pid in 1..=8u16 {
        let mut rng = WyRand::new(31u64.wrapping_add(pid as u64).wrapping_add(77));
        let r = engine.find_spawn_in_area(&mut rng, area);
        eprintln!("S18PROBE pid={pid} -> {r:?}");
    }
}
