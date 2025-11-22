use winit::window::Window;
use egui_wgpu::wgpu::*;

pub struct GpuContext {
    #[allow(dead_code)]
    pub instance: Instance,
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    #[cfg(not(target_arch = "wasm32"))]
    pub profiler: wgpu_profiler::GpuProfiler,
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

        // Create instance with appropriate backend for platform
        #[cfg(target_arch = "wasm32")]
        log::info!("Creating GPU instance with BROWSER_WEBGPU backend (WebGL not supported - requires compute shaders)");
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("Creating GPU instance with all backends");

        let instance = Instance::new(&InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: Backends::BROWSER_WEBGPU,  // WebGL doesn't support compute shaders
            #[cfg(not(target_arch = "wasm32"))]
            backends: Backends::all(),
            ..Default::default()
        });

        // SAFETY: We're extending the lifetime of the surface to 'static.
        // This is safe because the window will outlive the GpuContext in our usage.
        // The window is moved into the event loop closure and won't be dropped
        // until the application exits.
        log::info!("Creating surface from window...");

        #[cfg(target_arch = "wasm32")]
        let surface: Surface<'static> = {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowExtWebSys;

            // Get the canvas element directly (bypasses winit's canvas handling)
            let canvas = window.canvas()
                .ok_or_else(|| anyhow::anyhow!("Failed to get canvas from window"))?;

            log::info!("Got canvas element, creating WebGPU surface target...");

            // Create surface from canvas using raw web_sys element
            let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|e| anyhow::anyhow!("Failed to create surface from canvas: {:?}", e))?;

            log::info!("✓ Surface created successfully from canvas");
            unsafe { std::mem::transmute(surface) }
        };

        #[cfg(not(target_arch = "wasm32"))]
        let surface: Surface<'static> = {
            let surface_result = instance.create_surface(window);
            match surface_result {
                Ok(s) => {
                    log::info!("✓ Surface created successfully");
                    unsafe { std::mem::transmute(s) }
                }
                Err(e) => {
                    log::error!("Failed to create surface: {:?}", e);
                    return Err(anyhow::anyhow!("Surface creation failed: {:?}", e));
                }
            }
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

        // Log adapter info
        let adapter_info = adapter.get_info();
        log::info!("GPU Adapter: {}", adapter_info.name);
        log::info!("  Backend: {:?}", adapter_info.backend);
        log::info!("  Device Type: {:?}", adapter_info.device_type);
        log::info!("  Driver: {}", adapter_info.driver);
        log::info!("  Driver Info: {}", adapter_info.driver_info);

        // Use WebGL2-compatible limits for WASM, full limits for desktop
        #[cfg(target_arch = "wasm32")]
        let limits = Limits::downlevel_webgl2_defaults();
        #[cfg(not(target_arch = "wasm32"))]
        let limits = Limits::default();

        log::info!("Requesting device with limits: {:?}", limits);

        // Check adapter features for timestamp query support
        let adapter_features = adapter.features();
        log::info!("Adapter features: {:?}", adapter_features);
        log::info!("TIMESTAMP_QUERY supported: {}", adapter_features.contains(Features::TIMESTAMP_QUERY));
        log::info!("TIMESTAMP_QUERY_INSIDE_ENCODERS supported: {}", adapter_features.contains(Features::TIMESTAMP_QUERY_INSIDE_ENCODERS));
        log::info!("TIMESTAMP_QUERY_INSIDE_PASSES supported: {}", adapter_features.contains(Features::TIMESTAMP_QUERY_INSIDE_PASSES));

        // Enable timestamp queries for profiling (desktop only)
        #[cfg(not(target_arch = "wasm32"))]
        let required_features = Features::CLEAR_TEXTURE
            | Features::TIMESTAMP_QUERY
            | Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
            | Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        #[cfg(target_arch = "wasm32")]
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

        #[cfg(not(target_arch = "wasm32"))]
        let profiler = wgpu_profiler::GpuProfiler::new(
            &device,
            wgpu_profiler::GpuProfilerSettings {
                enable_timer_queries: true,
                enable_debug_groups: true,
                max_num_pending_frames: 3,
            }
        ).unwrap();

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            size,
            #[cfg(not(target_arch = "wasm32"))]
            profiler,
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
    pub fn set_present_mode(&mut self, vsync_enabled: bool) {
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

    pub fn begin_frame(&self) {
        // Placeholder: clear frame or begin compute passes
    }

}