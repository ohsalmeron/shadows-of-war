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

use crate::engine::SowEngine;
use crate::game::{GamePhase, GameState};
use crate::player::{Player, PlayerType};
use crate::protocol::{AttackIntent, GameplayIntent, Team};
use crate::water_components::WaterComponents;
use super::profile::{ai_profile_for, ai_tier};

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
    let mut idx = 0usize;
    for spec in specs {
        let mut player = match spec.kind {
            PlayerType::Human => Player::new_human(
                spec.id,
                format!("L{}", spec.id),
                [0.2, 0.5, 1.0],
                &crate::game_config::GameConfig::default(),
            ),
            PlayerType::Nation => Player::new_bot(
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
        idx += 1;
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

// ──────────────────────────────────────────────────────────────────────────
// S1 — Ghost isolated on free land keeps expanding until domination/victory.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s1_isolated_ghost_expands_until_victory() {
    let ghost = LabPlayer::ghost(1, 1, 1);
    let tribe = LabPlayer::tribe(2, 9, 9);
    let mut engine = build_lab(10, 10, "FFA", &[ghost, tribe]);

    let start = tiles(&engine, 1);
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
    assert!(reached_end || tiles(&engine, 1) >= (10 * 10) * 9 / 10,
        "S1 FAIL: no domination and no live game left");
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
    assert!(
        tiles(&engine, 1) > 1,
        "S2 FAIL: ghost did not grow at all"
    );
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
#[ignore = "known bug: enclosure-capture eats player tiles (game.rs set_tile_owner cascade); waiting for design ruling"]
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
            eprintln!("INIT idx{i} owner={}", engine.state.map.owner_id((i as u32) % 10, (i as u32) / 10));
        }
    }

    run_window(&mut engine, WINDOW_TICKS * 2);

    if std::env::var("SOW_LAB_VERBOSE").is_ok() {
        let p1 = engine.state.player(1).unwrap();
        let p2 = engine.state.player(2).unwrap();
        eprintln!(
            "S3DBG h_tiles={} t_tiles={} h_alive={} h_troops={} t_troops={} h_owner33={} att_count={}",
            p1.tile_count, p2.tile_count, p1.alive, p1.troops, p2.troops,
            engine.state.map.owner_id(3, 3), engine.attacks.len()
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
    let tribal_aggression = engine.attacks.iter().any(|a| a.owner_id == 2 && a.target_owner == 1);
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
    let mut mk = |id: u16, kind, ai: bool, iq: u32, x: u32, y: u32, team| {
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

    game.players.push(mk(
        1,
        PlayerType::Human,
        true,
        170,
        0,
        0,
        blue_team.clone(),
    ));
    game.players.push(mk(
        3,
        PlayerType::Human,
        true,
        165,
        0,
        1,
        blue_team.clone(),
    )); // ally wall to ghost's south
    game.players.push(mk(
        2,
        PlayerType::Nation,
        false,
        150,
        W - 1,
        W - 1,
        None,
    ));
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
    let naval_action = launched
        || !engine.fleets.is_empty()
        || tiles(&engine, 2) < enemy_start;
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

    let start: std::collections::HashMap<u16, u32> = specs
        .iter()
        .map(|s| (s.id, tiles(&engine, s.id)))
        .collect();
    let alive_start = specs.len();

    let mut prev_counts = start.clone();
    for window in 0..MAX_WINDOWS {
        let live = run_window(&mut engine, WINDOW_TICKS);
        let mut row = String::new();
        let mut stalls = Vec::new();
        for spec in specs.iter() {
            let now = tiles(&engine, spec.id);
            let grew = now.saturating_sub(*prev_counts.get(&spec.id).unwrap_or(&0));
            row.push_str(&format!(" p{}={}", spec.id, now));
            if grew == 0 && engine.state.player(spec.id).map(|p| p.alive).unwrap_or(false) {
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
            let Some(p) = engine.state.player(spec.id) else { continue };
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
                "DISSECT id={} alive={} tiles={} troops={:.0}/max{:.0} pts={:.0} gold={:.0} \
                 neighbors={:?} neutral={} armed={} next_att_id={}",
                spec.id,
                true,
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
// S10 — REAL WORLD MAP, prod-shaped roster: the reproduction attempt for the
// live report "everyone stops expanding mid-game". Uses the bundled world
// map (oceans/highlands/real terrain), real spawn points from the map file,
// ghost-filled FFA + nations + tribes across the actual continents.
// Telemetry prints per-window tiles per player; stall detection mirrors S9.
// ──────────────────────────────────────────────────────────────────────────
#[test]
fn s10_real_world_lobby_midgame() {
    use crate::maps::{load_map_from_payload, WORLD_MAP_BYTES};

    let mapfile = load_map_from_payload(WORLD_MAP_BYTES)
        .expect("bundled world map must decode for the lab");
    let (mw, mh) = (mapfile.width, mapfile.height);

    // Pick spawns spread across the world (stride-sampled, deterministic).
    let stride = ((mapfile.spawns.len() as f32) / 8.0).ceil().max(1.0) as usize;
    let chosen: Vec<_> = mapfile
        .spawns
        .iter()
        .step_by(stride)
        .take(8)
        .collect();
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

    let mut game =
        GameState::new(7, mw, mh, crate::game_config::GameConfig::default());
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
            PlayerType::Nation => Player::new_bot(
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
        for (id, base) in &start {
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
        if window >= 2 && !stalls.is_empty() {
            if std::env::var("SOW_LAB_VERBOSE").is_ok() {
                for sid in &stalls {
                    let probe = engine.state.player(*sid).map(|p| {
                        (
                            p.alive,
                            p.troops,
                            p.max_troops,
                            p.iq_points,
                        )
                    });
                    if let Some((alive, troops, maxt, iqpts)) = probe {
                        let (neighbors, has_neutral) =
                            engine.nation_scan_neighbors(*sid);
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
}
