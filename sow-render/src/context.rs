use blade_graphics as gpu;

pub struct RenderContext {
    pub context: gpu::Context,
    pub command_encoder: gpu::CommandEncoder,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderContext {
    pub fn try_new() -> Result<Self, gpu::NotSupportedError> {
        let context = unsafe {
            gpu::Context::init(gpu::ContextDesc {
                presentation: true,
                validation: cfg!(debug_assertions),
                overlay: false,
                ..Default::default()
            })?
        };
        let command_encoder = context.create_command_encoder(gpu::CommandEncoderDesc {
            name: "main",
            buffer_count: 2,
            manual_barriers: false,
        });

        Ok(Self {
            context,
            command_encoder,
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("Failed to initialize Blade Context")
    }

    pub fn create_surface<
        I: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    >(
        &self,
        window: &I,
        width: u32,
        height: u32,
    ) -> Result<gpu::Surface, blade_graphics::NotSupportedError> {
        // PREVENT CRASH: `blade-graphics` unwraps `window.window_handle()`, which panics with
        // `Unavailable` on Android before the NativeActivity surface is fully initialized.
        if window.window_handle().is_err() || window.display_handle().is_err() {
            return Err(blade_graphics::NotSupportedError::PlatformNotSupported);
        }

        let display_sync = if cfg!(any(target_os = "android", target_os = "ios")) {
            gpu::DisplaySync::Block
        } else {
            gpu::DisplaySync::Tear
        };

        let config = gpu::SurfaceConfig {
            size: gpu::Extent {
                width,
                height,
                depth: 1,
            },
            usage: gpu::TextureUsage::TARGET,
            display_sync,
            color_space: gpu::ColorSpace::Linear,
            ..Default::default()
        };
        self.context.create_surface_configured(window, config)
    }

    /// Drop and recreate the command encoder so Blade releases internal scratch buffers.
    pub fn reset_command_encoder(&mut self) {
        self.context
            .destroy_command_encoder(&mut self.command_encoder);
        self.command_encoder = self
            .context
            .create_command_encoder(gpu::CommandEncoderDesc {
                name: "main",
                buffer_count: 2,
                manual_barriers: false,
            });
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        // Blade does not auto-destroy the command encoder when the context is
        // dropped; without this its internal `_scratch`/`_marker` GPU blocks
        // leak (reported by the Blade leak checker on every teardown).
        self.context
            .destroy_command_encoder(&mut self.command_encoder);
    }
}
