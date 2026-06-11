use crate::{
    ui::{asset_loader, hud, loading_screen, main_menu},
    UiAction,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientPhase {
    Splash,
    MainMenu,
    Playing,
}

pub struct ClientApp {
    pub phase: ClientPhase,
    pub main_menu_state: main_menu::MainMenuState,
    pub hud_state: hud::HudState,
    pub splash_state: loading_screen::SplashState,
    pub asset_loader: asset_loader::AssetLoader,
    pub is_settings_open: bool,
    pub is_credits_open: bool,
    pub is_privacy_open: bool,
    pub is_terms_open: bool,
    pub settings_state: crate::ui::settings::SettingsState,
}

impl Default for ClientApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientApp {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::Splash,
            main_menu_state: main_menu::MainMenuState::default(),
            hud_state: hud::HudState {
                gold: 0.0,
                troops: 0.0,
                max_troops: 0.0,

                attack_ratio: 0.25,
                spawn_timer_secs: None,
                sync_state: None,

                my_player_id: 0,
                map_w: 0,
                attacks: Vec::new(),
                fleets: Vec::new(),
                players: Vec::new(),
                safe_area_top: 0.0,
                safe_area_bottom: 0.0,
                selected_tile: None,
                show_emoji_panel: false,
                emoji_panel_pos: None,
                emoji_panel_just_opened: false,
                pin_emoji: false,
                show_alliance_inbox: false,
                prev_requests: Vec::new(),
                last_request_time: None,
                show_betrayal_warning: None,
                betrayal_warning_cached: None,
                show_error: None,
                last_error_message: None,
                error_display_timer: None,
                show_info: None,
                last_info_message: None,
                info_display_timer: None,
                selected_building_kind: None,
                building_costs: [0.0; 9],
                selected_nuke_kind: None,
                event_log: Vec::new(),
                bottom_tab: hud::BottomHudTab::Controls,
                battle_log_seen_count: 0,
                event_log_seen_count: 0,
                prev_incoming_dispatch_count: 0,
                show_ask_panel: None,
                ask_gold: 0.0,
                ask_troops: 0.0,
                prev_resource_requests: Vec::new(),
                transfer_confirm_pending: false,
                chat_disabled: false,
            },
            splash_state: loading_screen::SplashState::default(),
            asset_loader: asset_loader::AssetLoader::new(),
            is_settings_open: false,
            is_credits_open: false,
            is_privacy_open: false,
            is_terms_open: false,
            settings_state: crate::ui::settings::SettingsState::default(),
        }
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    ) -> Option<UiAction> {
        crate::ui::theme::publish_reduced_motion(ui.ctx(), self.settings_state.reduced_motion);

        let mut action = if self.is_settings_open {
            crate::ui::settings::draw(ui, &mut self.settings_state)
        } else {
            match self.phase {
                ClientPhase::MainMenu => {
                    self.asset_loader.ensure_avatars_loaded(ui.ctx());
                    #[cfg(target_arch = "wasm32")]
                    self.asset_loader
                        .request_avatar_priority(self.main_menu_state.selected_leader);
                    self.asset_loader.ensure_ui_assets_loaded(ui.ctx());
                    main_menu::draw(
                        ui,
                        &mut self.main_menu_state,
                        &mut self.asset_loader,
                        self.settings_state.language,
                    )
                }
                ClientPhase::Splash => {
                    self.asset_loader.ensure_ui_assets_loaded(ui.ctx());
                    if let Some(new_phase) = loading_screen::draw(
                        ui,
                        &mut self.splash_state,
                        &self.asset_loader,
                        self.settings_state.language,
                    ) {
                        self.phase = new_phase;
                    }
                    None
                }
                ClientPhase::Playing => {
                    self.asset_loader.ensure_hud_icons_loaded(ui.ctx());
                    hud::draw(
                        ui,
                        &mut self.hud_state,
                        cancel_intents,
                        self.settings_state.language,
                        &mut self.asset_loader,
                    )
                }
            }
        };

        if let Some(ref toggle) = &action {
            if matches!(
                toggle,
                UiAction::ToggleSettings
                    | UiAction::ToggleCredits
                    | UiAction::TogglePrivacy
                    | UiAction::ToggleTerms
            ) {
                self.apply_modal_toggle(toggle);
                action = None;
            }
        }

        let credits_action = crate::ui::credits::draw(
            ui,
            self.is_credits_open,
            self.settings_state.language,
            self.settings_state.reduced_motion,
        );
        if let Some(ref toggle) = credits_action {
            self.apply_modal_toggle(toggle);
        } else if let Some(ref toggle) = action {
            if matches!(toggle, UiAction::ToggleCredits) {
                self.apply_modal_toggle(toggle);
                action = None;
            }
        }

        let privacy_action = crate::ui::legal_doc::draw(
            ui,
            sow_i18n::privacy(self.settings_state.language),
            self.is_privacy_open,
            UiAction::TogglePrivacy,
            "privacy",
            self.settings_state.reduced_motion,
        );
        if let Some(ref toggle) = privacy_action {
            self.apply_modal_toggle(toggle);
        } else if let Some(ref toggle) = action {
            if matches!(toggle, UiAction::TogglePrivacy) {
                self.apply_modal_toggle(toggle);
                action = None;
            }
        }

        let terms_action = crate::ui::legal_doc::draw(
            ui,
            sow_i18n::terms(self.settings_state.language),
            self.is_terms_open,
            UiAction::ToggleTerms,
            "terms",
            self.settings_state.reduced_motion,
        );
        if let Some(ref toggle) = terms_action {
            self.apply_modal_toggle(toggle);
        } else if let Some(ref toggle) = action {
            if matches!(toggle, UiAction::ToggleTerms) {
                self.apply_modal_toggle(toggle);
                action = None;
            }
        }

        action
    }

    fn apply_modal_toggle(&mut self, action: &UiAction) {
        match action {
            UiAction::ToggleSettings => {
                self.is_settings_open = !self.is_settings_open;
                if self.is_settings_open {
                    self.is_credits_open = false;
                    self.is_privacy_open = false;
                    self.is_terms_open = false;
                }
            }
            UiAction::ToggleCredits => {
                self.is_credits_open = !self.is_credits_open;
                if self.is_credits_open {
                    self.is_settings_open = false;
                    self.is_privacy_open = false;
                    self.is_terms_open = false;
                }
            }
            UiAction::TogglePrivacy => {
                self.is_privacy_open = !self.is_privacy_open;
                if self.is_privacy_open {
                    self.is_settings_open = false;
                    self.is_credits_open = false;
                    self.is_terms_open = false;
                }
            }
            UiAction::ToggleTerms => {
                self.is_terms_open = !self.is_terms_open;
                if self.is_terms_open {
                    self.is_settings_open = false;
                    self.is_credits_open = false;
                    self.is_privacy_open = false;
                }
            }
            _ => {}
        }
    }
}
