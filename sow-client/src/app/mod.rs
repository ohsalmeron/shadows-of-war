mod account;
pub(crate) mod audio;
mod bootstrap;
mod gpu;
mod progress;
mod state;

pub use state::*;

impl SowApp {
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
    pub fn update(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.check_surface();

        let now = web_time::Instant::now();
        self.update_net(now);
        self.update_assets();
        self.update_loader();
        self.update_sim(now);
    }
}
