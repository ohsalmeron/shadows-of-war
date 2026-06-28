pub mod actions;
pub mod context_menu;
pub mod hold_attack;

use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_map_interactions(&mut self, ctx: &egui::Context) {
        if self
            .sim
            .current_snapshot
            .as_ref()
            .is_some_and(|s| s.winner.is_some())
        {
            return;
        }

        if self.ui.app.main_menu_state.is_waiting {
            return;
        }

        // ── Hold-to-Attack pump: sends 10% of troops per second while held ──
        self.pump_hold_attack(ctx);

        // ── Hold-to-Build pump ──
        self.pump_hold_build(ctx);

        // Sync active context menu with request state (opens animation)
        if let Some(target) = self.input.map_context_menu {
            self.input.map_context_menu_active = Some(target);
        }

        // ── Context menu (right-click on desktop, tap on mobile) ──
        self.draw_context_menu(ctx);
    }
}
