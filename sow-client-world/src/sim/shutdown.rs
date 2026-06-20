use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_sim_shutdown(&mut self) {
        self.sim.engine = None;
        self.sim.current_snapshot = None;
        self.ui.mover_scene = crate::render::world::movers::MoverScene::new();
    }
}
