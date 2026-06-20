use super::state::SowApp;
use sow_render::RenderContext;

impl SowApp {
    /// Initialize the shared Blade context once; returns false after a fatal error.
    pub fn ensure_render_ctx(&mut self) -> bool {
        if self.gfx.render_ctx.is_some() {
            return true;
        }
        if self.gpu_init_failed {
            return false;
        }
        match RenderContext::try_new() {
            Ok(ctx) => {
                self.gfx.render_ctx = Some(ctx);
                true
            }
            Err(err) => {
                self.gpu_init_failed = true;
                eprintln!(
                    "Failed to initialize GPU (Vulkan).\n\
                     On Linux, ensure Vulkan drivers are installed and loaded.\n\
                     If you use NVIDIA, run `nvidia-smi` — a driver/library version mismatch \
                     requires a reboot after updating nvidia-utils.\n\
                     Close other GPU apps if video memory is exhausted.\n\
                     Details: {err}"
                );
                log::error!("GPU init failed: {err}");
                false
            }
        }
    }
    /// Window used for input/redraw.
    pub fn active_window(&self) -> Option<&dyn winit::window::Window> {
        self.gfx.window.as_deref()
    }

    pub fn handle_suspended(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        let Some(render_ctx) = self.gfx.render_ctx.as_mut() else {
            return;
        };
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut s) = self.gfx.surface.take() {
            if let Some(mut gp) = self.gfx.gui_painter.take() {
                gp.destroy(&render_ctx.context);
            }
            if let Some(mut mr) = self.gfx.map_renderer.take() {
                mr.destroy(render_ctx);
            }
            if let Some(mut mover) = self.gfx.mover_renderer.take() {
                mover.destroy(render_ctx);
            }
            render_ctx.context.destroy_surface(&mut s);
        }
    }

    pub fn handle_resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        // App or tab foregrounded — retry WS soon if the socket died in the background.
        self.net.ws_reconnect_after_resume = true;
        if !self.ensure_render_ctx() {
            event_loop.exit();
            return;
        }
        if self.gfx.window.is_none() {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            #[allow(unused_mut)]
            let mut attributes =
                winit::window::WindowAttributes::default().with_title("Shadows of War");

            #[cfg(target_os = "ios")]
            {
                let ios_attrs = winit::platform::ios::WindowAttributesIos::default()
                    .with_valid_orientations(
                        winit::platform::ios::ValidOrientations::LandscapeAndPortrait,
                    )
                    .with_prefers_status_bar_hidden(true)
                    .with_prefers_home_indicator_hidden(true);
                attributes = attributes.with_platform_attributes(Box::new(ios_attrs));
            }
            #[cfg(target_arch = "wasm32")]
            let mut attributes = {
                let (w, h) = crate::web_canvas::canvas_logical_size();
                winit::window::WindowAttributes::default()
                    .with_title("Shadows of War")
                    .with_surface_size(winit::dpi::LogicalSize::new(w, h))
            };

            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsCast;
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();
                let canvas = document
                    .get_element_by_id("blade")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .unwrap();
                let web_attrs = winit::platform::web::WindowAttributesWeb::default()
                    .with_canvas(Some(canvas))
                    .with_prevent_default(true);
                attributes = attributes.with_platform_attributes(Box::new(web_attrs));
                crate::ime::ensure_canvas_tabindex();
            }

            #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
            let attributes = winit::window::WindowAttributes::default()
                .with_title("Shadows of War")
                .with_surface_size(winit::dpi::LogicalSize::new(800.0, 800.0));

            match event_loop.create_window(attributes) {
                Ok(win) => self.gfx.window = Some(win),
                Err(e) => {
                    log::warn!("Window creation failed: {:?}", e);
                    return;
                }
            }
        }
        self.check_surface();
    }
}

impl Drop for SowApp {
    fn drop(&mut self) {
        let Some(render_ctx) = self.gfx.render_ctx.as_mut() else {
            return;
        };
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.gfx.map_renderer.take() {
            mr.destroy(render_ctx);
        }
        if let Some(mut mover) = self.gfx.mover_renderer.take() {
            mover.destroy(render_ctx);
        }
        if let Some(mut gui) = self.gfx.gui_painter.take() {
            gui.destroy(&render_ctx.context);
        }
        if let Some(mut s) = self.gfx.surface.take() {
            render_ctx.context.destroy_surface(&mut s);
        }
        // The command encoder is destroyed by `RenderContext`'s own `Drop`
        // when the `Option<RenderContext>` field is dropped after this.
    }
}
