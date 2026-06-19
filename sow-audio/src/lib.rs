//! Procedural retro audio effects with spatial panning.
//! Leaf crate: no workspace dependencies beyond rodio (native only).

/// Combat / expansion sound category for procedural synthesis.
#[derive(Clone, Copy, Debug)]
pub enum CombatSoundKind {
    WildernessExpansion,
    AttackHuman,
    AttackEmpire,
    AttackTribe,
    CounterAttack,
}

/// Player archetype for death-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum PlayerSoundType {
    Human,
    Nation,
    Bot,
}

/// Structure type for placement-sound synthesis.
#[derive(Clone, Copy, Debug)]
pub enum BuildingSoundKind {
    City,
    Bunker,
    Factory,
    Port,
}

/// World position and camera state for spatial audio panning/attenuation.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSoundParams {
    pub wx: f32,
    pub wy: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
}

pub fn play_death_sound(player_type: PlayerSoundType, seed: u32, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_death_sound(player_type, seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (player_type, seed, spatial);
}

pub fn play_deploy_sound(spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_deploy_sound(spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = spatial;
}

pub fn play_combat_sound(
    kind: CombatSoundKind,
    troops: f32,
    seed: u32,
    spatial: SpatialSoundParams,
) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_combat_sound(kind, troops, seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, troops, seed, spatial);
}

pub fn play_building_placement_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_building_placement_sound(kind, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, spatial);
}

pub fn play_building_completed_sound(kind: BuildingSoundKind, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_building_completed_sound(kind, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (kind, spatial);
}

pub fn play_nuke_launch_sound(spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_nuke_launch_sound(spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = spatial;
}

pub fn play_nuke_impact_sound(level: u8, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_nuke_impact_sound(level, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (level, spatial);
}

pub fn play_bunker_defense_sound(seed: u32, spatial: SpatialSoundParams) {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_bunker_defense_sound(seed, spatial);
    #[cfg(target_arch = "wasm32")]
    let _ = (seed, spatial);
}

pub fn set_music_context(seed: u32, anchor_wx: f32, anchor_wy: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    native::set_music_context(seed, anchor_wx, anchor_wy);
    #[cfg(target_arch = "wasm32")]
    let _ = (seed, anchor_wx, anchor_wy);
}

pub fn play_victory_sound() {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_victory_sound();
}

pub fn play_defeat_sound() {
    #[cfg(not(target_arch = "wasm32"))]
    native::play_defeat_sound();
}

#[cfg(not(target_arch = "wasm32"))]

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::play_spatial;
