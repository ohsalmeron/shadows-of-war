use crate::app::SowApp;

pub(super) struct ContextMenuTileOpts {
    pub tile_idx: u32,
    pub col: u32,
    pub row: u32,
    pub center: egui::Pos2,
    pub scale: f32,
    pub compact: bool,
    pub screen: egui::Rect,
    pub outer_r: f32,
    pub is_own_territory: bool,
    pub radial_build_active: bool,
    pub radial_missile_active: bool,
    pub build_active_id: egui::Id,
    pub missile_active_id: egui::Id,
}

impl SowApp {
    pub(super) fn draw_context_menu_popovers(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        opts: &ContextMenuTileOpts,
    ) {
        self.draw_build_popover(ui, ctx, opts);
        self.draw_missile_popover(ui, ctx, opts);
    }
}
