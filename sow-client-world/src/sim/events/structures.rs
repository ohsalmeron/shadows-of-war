use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_structure_spawned(
        &mut self,
        my_id: u16,
        tile_idx: u32,
        kind: sow_core::game::BuildingKind,
        owner_id: u16,
    ) {
        if owner_id == my_id {
            return;
        }
        let x = (tile_idx % self.sim.map_w) as f32 + 0.5;
        let y = (tile_idx / self.sim.map_w) as f32 + 0.5;
        sow_audio::play_building_placement_sound(
            crate::building_sound_kind(kind),
            self.spatial_sound_params(x, y),
        );
    }
}
