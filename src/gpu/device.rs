use std::sync::Arc;
use std::mem::ManuallyDrop;
use winit::window::Window;
use egui_wgpu::wgpu::*;

pub struct GpuContext {
    #[allow(dead_code)]
    pub instance: Instance,
    /// Wrapped in ManuallyDrop so reinit() can drop the old surface
    /// before creating a new one (OS only allows one swap chain per window).
    /// Deref makes this transparent — all existing code works unchanged.
    pub surface: ManuallyDrop<Surface<'static>>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub config: SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    /// True when running on a software renderer (SwiftShader, Lavapipe, etc.)
    pub is_software_renderer: bool,
    /// GPU adapter name (e.g. "NVIDIA GeForce RTX 3080" or "llvmpipe")
    pub adapter_name: String,
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        // SAFETY: surface is always valid except briefly during reinit()
        unsafe { ManuallyDrop::drop(&mut self.surface); }
    }
}

impl GpuContext {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
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

        // Create instance with appropriate backend for platform
        #[cfg(target_arch = "wasm32")]
        log::info!("Creating GPU instance with BROWSER_WEBGPU backend (WebGL not supported - requires compute shaders)");
        // Support WGPU_BACKEND env var to force a specific backend
        // (e.g. WGPU_BACKEND=vulkan to test SwiftShader on Windows where DX12 is preferred)
        #[cfg(not(target_arch = "wasm32"))]
        let desktop_backends = std::env::var("WGPU_BACKEND").ok().and_then(|val| {
            match val.to_lowercase().as_str() {
                "vulkan" | "vk" => Some(Backends::VULKAN),
                "dx12" | "d3d12" => Some(Backends::DX12),
                "metal" | "mtl" => Some(Backends::METAL),
                "gl" | "opengl" => Some(Backends::GL),
                _ => None,
            }
        }).unwrap_or(Backends::all());
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("Creating GPU instance with backends: {:?}", desktop_backends);

        let instance = Instance::new(&InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: Backends::BROWSER_WEBGPU,
            #[cfg(not(target_arch = "wasm32"))]
            backends: desktop_backends,
            ..Default::default()
        });

        // Using Arc<Window> gives the surface a 'static lifetime safely
        log::info!("Creating surface from window...");

        #[cfg(target_arch = "wasm32")]
        let surface: Surface<'static> = {
            use winit::platform::web::WindowExtWebSys;

            // Get the canvas element directly (bypasses winit's canvas handling)
            let canvas = window.canvas()
                .ok_or_else(|| anyhow::anyhow!("Failed to get canvas from window"))?;

            log::info!("Got canvas element, creating WebGPU surface target...");

            // Create surface from canvas using raw web_sys element
            let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|e| anyhow::anyhow!("Failed to create surface from canvas: {:?}", e))?;

            log::info!("✓ Surface created successfully from canvas");
            surface
        };

        #[cfg(not(target_arch = "wasm32"))]
        let surface: Surface<'static> = {
            // Arc<Window> implements Into<SurfaceTarget<'static>>, giving us Surface<'static>
            let surface = instance.create_surface(window.clone())
                .map_err(|e| {
                    log::error!("Failed to create surface: {:?}", e);
                    anyhow::anyhow!("Surface creation failed: {:?}", e)
                })?;
            log::info!("✓ Surface created successfully");
            surface
        };

        // Try to get adapter with high performance preference first
        log::info!("Requesting GPU adapter (high-performance)...");
        let adapter_options = RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        };

        let adapter = instance.request_adapter(&adapter_options).await;

        // If that fails, try with fallback adapter
        let adapter = match adapter {
            Ok(a) => {
                log::info!("✓ High-performance adapter found");
                a
            },
            Err(e) => {
                log::warn!("High-performance adapter not found: {:?}", e);
                log::warn!("Trying fallback adapter...");
                let fallback_options = RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    power_preference: PowerPreference::default(),
                    force_fallback_adapter: true,
                };
                let fallback = instance.request_adapter(&fallback_options)
                    .await
                    .expect("No suitable GPU adapters found (tried high-performance and fallback)");
                log::info!("✓ Fallback adapter found");
                fallback
            }
        };

        // Log adapter info and detect software rendering
        let adapter_info = adapter.get_info();
        let is_software_renderer = adapter_info.device_type == DeviceType::Cpu;
        let adapter_name = adapter_info.name.clone();
        log::info!("GPU Adapter: {}", adapter_info.name);
        log::info!("  Backend: {:?}", adapter_info.backend);
        log::info!("  Device Type: {:?}", adapter_info.device_type);
        log::info!("  Driver: {}", adapter_info.driver);
        log::info!("  Driver Info: {}", adapter_info.driver_info);
        if is_software_renderer {
            log::warn!("Software renderer detected — performance will be reduced");
        }

        // Get adapter limits to request higher storage buffer size for large resolutions
        let adapter_limits = adapter.limits();
        log::info!(
            "Adapter max_storage_buffer_binding_size: {} bytes ({} MB)",
            adapter_limits.max_storage_buffer_binding_size,
            adapter_limits.max_storage_buffer_binding_size / (1024 * 1024)
        );

        // Use WebGL2-compatible limits for WASM, full limits for desktop
        #[cfg(target_arch = "wasm32")]
        let mut limits = Limits::downlevel_webgl2_defaults();
        #[cfg(not(target_arch = "wasm32"))]
        let mut limits = Limits::default();

        // Request adapter's max storage buffer size (for 4K+ resolutions)
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;

        log::info!("Requesting device with limits: max_storage_buffer_binding_size = {} MB",
            limits.max_storage_buffer_binding_size / (1024 * 1024));

        // Check adapter features for timestamp query support
        let adapter_features = adapter.features();
        log::info!("Adapter features: {:?}", adapter_features);

        let required_features = Features::CLEAR_TEXTURE;

        let (device, queue) = adapter.request_device(
            &DeviceDescriptor {
                label: Some("Main GPU Device"),
                required_features,
                required_limits: limits,
                memory_hints: Default::default(),
                experimental_features: Default::default(),
                trace: Default::default(),
            }
        ).await?;

        log::info!(
            "Device max_storage_buffer_binding_size: {} bytes ({} MB)",
            device.limits().max_storage_buffer_binding_size,
            device.limits().max_storage_buffer_binding_size / (1024 * 1024)
        );

        log::info!("✓ GPU device created successfully");

        let surface_caps = surface.get_capabilities(&adapter);
        log::info!("Surface capabilities:");
        log::info!("  Formats: {:?}", surface_caps.formats);
        log::info!("  Present modes: {:?}", surface_caps.present_modes);
        log::info!("  Alpha modes: {:?}", surface_caps.alpha_modes);

        // Prefer non-sRGB formats for egui compatibility (avoids sRGB warning)
        let format = surface_caps.formats.iter()
            .find(|f| matches!(f, TextureFormat::Bgra8Unorm | TextureFormat::Rgba8Unorm))
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        log::info!("Selected format: {:?}", format);

        // Allow testing different present modes via environment variable
        // let present_mode = if cfg!(target_arch = "wasm32") {
        //     PresentMode::Fifo  // WASM only supports Fifo
        // } else if let Ok(mode_str) = std::env::var("PRESENT_MODE") {
        //     match mode_str.to_lowercase().as_str() {
        //         "immediate" => {
        //             log::warn!("Using PresentMode::Immediate (no VSync) - EXPERIMENTAL");
        //             PresentMode::Immediate
        //         }
        //         "fifo" => {
        //             log::info!("Using PresentMode::Fifo (VSync blocking)");
        //             PresentMode::Fifo
        //         }
        //         "mailbox" => {
        //             log::info!("Using PresentMode::Mailbox (VSync non-blocking)");
        //             PresentMode::Mailbox
        //         }
        //         _ => {
        //             log::warn!("Unknown PRESENT_MODE '{}', using default", mode_str);
        //             if cfg!(target_os = "macos") {
        //                 PresentMode::Fifo
        //             } else {
        //                 PresentMode::Mailbox
        //             }
        //         }
        //     }
        // } else {
        //     // Default: Use Fifo (true VSync) to cap at monitor refresh rate
        //     PresentMode::Fifo
        // };
        let present_mode = PresentMode::Fifo;

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode,
            // Use Opaque alpha mode to ensure frames don't accumulate
            // Auto mode in WASM can cause compositing issues where frames blend together
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        log::info!("Configuring surface with: {:?}x{:?}, format: {:?}, present_mode: {:?}",
            config.width, config.height, config.format, config.present_mode);

        surface.configure(&device, &config);
        log::info!("✓ Surface configured successfully");

        Ok(Self {
            instance,
            surface: ManuallyDrop::new(surface),
            device: Arc::new(device),
            queue: Arc::new(queue),
            config,
            size,
            is_software_renderer,
            adapter_name,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Update present mode (VSync setting)
    /// Note: WASM only supports Fifo (VSync always enabled)
    pub fn set_present_mode(&mut self, vsync_enabled: bool) {
        #[cfg(target_arch = "wasm32")]
        let present_mode = PresentMode::Fifo;  // WASM only supports Fifo

        #[cfg(not(target_arch = "wasm32"))]
        let present_mode = if vsync_enabled {
            PresentMode::Fifo  // VSync enabled - cap at monitor refresh rate
        } else {
            PresentMode::Immediate  // VSync disabled - render as fast as possible
        };

        if self.config.present_mode != present_mode {
            self.config.present_mode = present_mode;
            self.surface.configure(&self.device, &self.config);
            log::info!("Updated present mode: {:?}", present_mode);
        }
    }

    /// Full GPU reinitialization: new instance, adapter, device, queue, and surface.
    /// Called after device loss (e.g. Windows sleep/wake). Uses pollster::block_on
    /// for the async adapter/device calls, which complete synchronously on desktop.
    /// Preserves current size and present mode from the old config.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn reinit(&mut self, window: Arc<Window>) -> anyhow::Result<()> {
        log::info!("Full GPU reinitialization starting...");
        let old_size = self.size;
        let old_present_mode = self.config.present_mode;

        // Flush any in-flight GPU work before releasing resources.
        // The device may be lost, so ignore poll errors.
        let _ = self.device.poll(PollType::Wait { submission_index: None, timeout: None });

        // SAFETY: Drop old surface to release the window's swap chain.
        // The OS only allows one swap chain per window, so we must release
        // the old one before GpuContext::new() creates a new one.
        // If new() fails after this, we panic — there's no way to restore a valid surface.
        unsafe { ManuallyDrop::drop(&mut self.surface); }

        let mut new_ctx = ManuallyDrop::new(
            pollster::block_on(Self::new(window))
                .expect("GPU reinitialization failed — cannot recover from device loss")
        );

        // SAFETY: We're moving all fields out of new_ctx and preventing its Drop
        // from running (via ManuallyDrop). The old surface was already dropped above.
        unsafe {
            self.instance = std::ptr::read(&new_ctx.instance);
            self.surface = std::ptr::read(&new_ctx.surface);
            self.device = std::ptr::read(&new_ctx.device);
            self.queue = std::ptr::read(&new_ctx.queue);
            self.config = std::ptr::read(&new_ctx.config);
        }
        self.size = old_size;

        // Restore size and present mode
        self.config.width = old_size.width;
        self.config.height = old_size.height;
        self.config.present_mode = old_present_mode;
        self.surface.configure(&self.device, &self.config);

        log::info!("✓ Full GPU reinitialization complete");
        Ok(())
    }

    pub fn begin_frame(&self) {
        // Placeholder: clear frame or begin compute passes
    }

}