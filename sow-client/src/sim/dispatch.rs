use crate::app::SowApp;

impl SowApp {
    pub fn dispatch_sim_command(&mut self, cmd: sow_core::protocol::SimCommand) {
        match cmd {
            sow_core::protocol::SimCommand::Init {
                config,
                seed,
                map_bytes,
                players,
            } => self.handle_sim_init(config, seed, map_bytes, players),
            sow_core::protocol::SimCommand::Turn(turn) => self.handle_sim_turn(turn),
            sow_core::protocol::SimCommand::Shutdown => self.handle_sim_shutdown(),
        }
    }
}
