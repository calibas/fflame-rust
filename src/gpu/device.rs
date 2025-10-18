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
    pub async fn new<'a>(window: &'a Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
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
            required_features: Features::empty(),
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
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
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