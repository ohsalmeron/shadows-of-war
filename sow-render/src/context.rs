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
    pub fn new() -> Self {
        let context = unsafe {
            gpu::Context::init(gpu::ContextDesc {
                presentation: true,
                validation: cfg!(debug_assertions),
                overlay: false,
                ..Default::default()
            })
            .expect("Failed to initialize Blade Context")
        };
        let command_encoder = context.create_command_encoder(gpu::CommandEncoderDesc {
            name: "main",
            buffer_count: 2,
        });

        Self {
            context,
            command_encoder,
        }
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

        let config = gpu::SurfaceConfig {
            size: gpu::Extent {
                width,
                height,
                depth: 1,
            },
            usage: gpu::TextureUsage::TARGET,
            display_sync: gpu::DisplaySync::Tear,
            color_space: gpu::ColorSpace::Srgb,
            ..Default::default()
        };
        self.context.create_surface_configured(window, config)
    }
}
