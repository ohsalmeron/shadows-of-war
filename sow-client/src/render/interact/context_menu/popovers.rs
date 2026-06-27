use crate::app::SowApp;

impl SowApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_context_menu_popovers(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        tile_idx: u32,
        col: u32,
        row: u32,
        center: egui::Pos2,
        scale: f32,
        compact: bool,
        screen: egui::Rect,
        outer_r: f32,
        is_own_territory: bool,
        radial_build_active: bool,
        radial_missile_active: bool,
        build_active_id: egui::Id,
        missile_active_id: egui::Id,
    ) {
        self.draw_build_popover(
            ui,
            ctx,
            tile_idx,
            center,
            scale,
            compact,
            screen,
            outer_r,
            col,
            row,
            is_own_territory,
            radial_build_active,
            build_active_id,
        );
        self.draw_missile_popover(
            ui,
            ctx,
            tile_idx,
            center,
            scale,
            compact,
            screen,
            outer_r,
            is_own_territory,
            radial_missile_active,
            missile_active_id,
        );
    }
}
