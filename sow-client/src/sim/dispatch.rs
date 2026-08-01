use crate::app::SowApp;

impl SowApp {
    pub fn dispatch_sim_command(&mut self, cmd: sow_core::protocol::SimCommand) {
        match cmd {
            sow_core::protocol::SimCommand::Init {
                config,
                seed,
                map_bytes,
                players,
                map_spawns,
                geo_bounds,
                num_land_tiles,
            } => self.handle_sim_init(super::init::SimInitOpts {
                config,
                seed,
                map_bytes,
                players,
                map_spawns,
                geo_bounds,
                num_land_tiles,
            }),
            sow_core::protocol::SimCommand::Turn(turn) => self.handle_sim_turn(turn),
            sow_core::protocol::SimCommand::Shutdown => self.handle_sim_shutdown(),
        }
    }
}
