use winit::window::Window;
use wgpu::*;

pub struct GpuContext {
    #[allow(dead_code)]
    pub instance: Instance,
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl GpuContext {
    pub async fn new(window: &Window) -> anyhow::Result<Self> {
        let size = window.inner_size();

        // WASM fallback: ensure we have valid dimensions
        #[cfg(target_arch = "wasm32")]
        let size = {
            let final_size = if size.width == 0 || size.height == 0 {
                log::warn!("Window inner_size is zero, using fallback dimensions");
                winit::dpi::PhysicalSize::new(1280, 720)
            } else {
                size
            };
            log::info!("GPU Context size: {}x{}", final_size.width, final_size.height);
            final_size
        };

        let instance = Instance::default();

        // SAFETY: We're extending the lifetime of the surface to 'static.
        // This is safe because the window will outlive the GpuContext in our usage.
        // The window is moved into the event loop closure and won't be dropped
        // until the application exits.
        let surface: Surface<'static> = unsafe {
            std::mem::transmute(instance.create_surface(window)?)
        };

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: PowerPreference::HighPerformance,
            ..Default::default()
        }).await.expect("No suitable GPU adapters found");

        let (device, queue) = adapter.request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::CLEAR_TEXTURE,
            required_limits: Limits::default(),
            memory_hints: Default::default(),
        }, None).await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps.formats[0];

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Mailbox,  // Fast but smooth - software frame limiter controls speed multiplier
            // Use Opaque alpha mode to ensure frames don't accumulate
            // Auto mode in WASM can cause compositing issues where frames blend together
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self { instance, surface, device, queue, config, size })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn begin_frame(&self) {
        // Placeholder: clear frame or begin compute passes
    }

}