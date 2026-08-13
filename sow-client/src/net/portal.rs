use crate::app::SowApp;

impl SowApp {
    pub(crate) fn poll_portal_intents(&mut self) {
        if let Some(id) = crate::store_portals::poll_pending_invite_lobby() {
            self.ui.app.main_menu_state.pending_join_lobby_id = Some(id);
            self.ui.app.main_menu_state.is_waiting = true;
            if self.net.client.is_some() {
                self.send_join_if_connected(Some(id), false);
            }
        }
        if crate::store_portals::poll_auth_changed() {
            let fallback = self.ui.app.main_menu_state.player_name.clone();
            let identity = crate::store_portals::load_identity(&fallback);
            self.ui.app.main_menu_state.player_name = identity.display_name;
            self.ui.app.main_menu_state.name_locked = identity.name_locked;
            if crate::store_portals::should_fetch_cloud_profile() {
                self.fetch_cloud_progress();
            }
        }
        if let Some(mute) = crate::store_portals::poll_mute_audio_setting() {
            crate::store_portals::apply_mute_audio_setting(mute);
        }
        self.ui.app.hud_state.chat_disabled = crate::store_portals::is_chat_disabled();
    }
}
