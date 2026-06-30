use egui::Color32;
use sow_core::protocol::{AttackSnapshot, FleetSnapshot, PlayerSnapshot};
use web_time::Instant;

const EVENT_LOG_MAX_ENTRIES: usize = 50;
const HUD_BOTTOM_CONTROLS_GAP: f32 = 12.0;
const HUD_MAP_CONTROLS_DESKTOP_CLEARANCE: f32 = 100.0;
/// Fallback when the bottom panel has not been laid out yet this frame (mobile).
pub(in crate::ui::hud) const HUD_MAP_CONTROLS_MOBILE_FALLBACK_CLEARANCE: f32 = 220.0;

pub(in crate::ui::hud) fn hud_bottom_panel_clearance(ctx: &egui::Context, compact: bool) -> f32 {
    if !compact {
        return HUD_MAP_CONTROLS_DESKTOP_CLEARANCE;
    }
    let screen = ctx.content_rect();
    ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_bottom_panel_rect")))
        .map(|r| (screen.max.y - r.min.y).max(0.0) + HUD_BOTTOM_CONTROLS_GAP)
        .unwrap_or(HUD_MAP_CONTROLS_MOBILE_FALLBACK_CLEARANCE + HUD_BOTTOM_CONTROLS_GAP)
}

pub(in crate::ui::hud) fn hud_map_controls_anchor_offset(
    ctx: &egui::Context,
    compact: bool,
    safe_area_bottom: f32,
) -> egui::Vec2 {
    egui::vec2(
        -12.0,
        -hud_bottom_panel_clearance(ctx, compact) - safe_area_bottom,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BottomHudTab {
    #[default]
    Controls,
    BattleLog,
    EventLog,
}

#[derive(Clone, Debug)]
pub struct EventLogEntry {
    pub message: String,
    pub color: Color32,
    pub spawned_at: Instant,
}

#[derive(Clone, Debug)]
pub struct SelectedTileInfo {
    pub tile_idx: u32,
    pub owner_id: u16,
    pub is_own_territory: bool,
    pub is_friendly: bool,
    pub is_spawning: bool,
    pub is_land: bool,
}

#[derive(Clone, Debug)]
pub struct HudNotification {
    pub message: String,
    pub color: Color32,
    pub spawned_at: Instant,
}

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub troop_rate: f64,
    pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,

    pub my_player_id: u16,
    pub map_w: u32,
    pub attacks: Vec<AttackSnapshot>,
    pub fleets: Vec<FleetSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    pub safe_area_top: f32,
    pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>,
    pub show_emoji_panel: bool,
    pub emoji_panel_pos: Option<egui::Pos2>,
    pub emoji_panel_just_opened: bool,
    pub pin_emoji: bool,
    pub show_alliance_inbox: bool,
    pub(crate) prev_requests: Vec<u16>,
    pub(crate) last_request_time: Option<Instant>,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub betrayal_warning_cached: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub selected_building_kind: Option<sow_core::game::BuildingKind>,
    pub building_costs: [f64; 9],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>,
    pub event_log: Vec<EventLogEntry>,
    pub hud_notifications: Vec<HudNotification>,
    pub bottom_tab: BottomHudTab,
    pub battle_log_seen_count: usize,
    pub event_log_seen_count: usize,
    pub(crate) prev_incoming_dispatch_count: usize,
    pub show_ask_panel: Option<u16>,
    pub ask_gold: f64,
    pub ask_troops: f64,
    pub prev_resource_requests: Vec<u16>,
    pub transfer_confirm_pending: bool,
    pub chat_disabled: bool,
    pub show_exit_confirm: bool,
    pub is_tutorial: bool,

    /// A message that should *take over* the bottom panel (reusing its frame) instead of
    /// floating its own sheet. Set by the campaign/tutorial each frame it wants the dialog up;
    /// cleared to dismiss. The panel animates it in/out and reports the click via
    /// [`Self::bottom_dialog_click`]. See [`crate::ui::hud::draw`]'s bottom-panel takeover.
    pub bottom_dialog: Option<crate::widgets::BottomDialog>,
    /// `(dialog id, button index)` the panel recorded last frame (tap/timeout/button). Tagged with
    /// the id so a caller `take()`ing it can discard a click that belonged to a *different* dialog
    /// (e.g. one still fading out) instead of misattributing it.
    pub bottom_dialog_click: Option<(String, usize)>,
}

impl HudState {
    pub fn push_notification(&mut self, message: String, color: Color32) {
        let spawned_at = Instant::now();
        self.hud_notifications.push(HudNotification {
            message: message.clone(),
            color,
            spawned_at,
        });
        self.event_log.push(EventLogEntry {
            message,
            color,
            spawned_at,
        });
        if self.event_log.len() > EVENT_LOG_MAX_ENTRIES {
            self.event_log.remove(0);
        }
    }
}

pub(in crate::ui::hud) fn dispatch_count(state: &HudState) -> usize {
    let my_pid = state.my_player_id;
    if my_pid == 0 {
        return 0;
    }
    state
        .attacks
        .iter()
        .filter(|a| a.target_owner == my_pid || a.owner_id == my_pid)
        .count()
        + state.fleets.iter().filter(|f| f.owner_id == my_pid).count()
}

pub(in crate::ui::hud) fn incoming_dispatch_count(state: &HudState) -> usize {
    let my_pid = state.my_player_id;
    if my_pid == 0 {
        return 0;
    }
    state
        .attacks
        .iter()
        .filter(|a| a.target_owner == my_pid)
        .count()
}

pub(in crate::ui::hud) fn building_emoji(kind: sow_core::game::BuildingKind) -> &'static str {
    match kind {
        sow_core::game::BuildingKind::City => "🏛️",
        sow_core::game::BuildingKind::Factory => "🏭",
        sow_core::game::BuildingKind::Port => "⚓",
        sow_core::game::BuildingKind::Bunker => "🛡️",
    }
}

pub(in crate::ui::hud) fn get_player_display_name(
    players: &[PlayerSnapshot],
    id: u16,
    default: &str,
) -> String {
    players
        .iter()
        .find(|p| p.id == id)
        .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
        .unwrap_or_else(|| default.to_string())
}
