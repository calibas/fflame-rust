use fractal_flame_wgpu::desktop_main;

#[cfg(not(target_arch = "wasm32"))]
use clap::{Parser, Subcommand};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Parser)]
#[command(name = "fractal_flame_wgpu")]
#[command(about = "Fractal Flame Renderer", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Subcommand)]
enum Commands {
    /// List available FFmpeg encoders and hardware acceleration options
    ListEncoders,

    /// Export flame configs to PNG (headless batch mode)
    Export {
        /// Input .fflame config file or directory
        #[arg(short, long)]
        input: String,

        /// Output PNG file or directory
        #[arg(short, long)]
        output: String,

        /// Image width (default: from config or 1920)
        #[arg(short, long)]
        width: Option<u32>,

        /// Image height (default: from config or 1080)
        #[arg(short = 'H', long)]
        height: Option<u32>,

        /// Test category for metadata
        #[arg(short, long)]
        category: Option<String>,

        /// Iterations per thread (overrides config file value if provided, range: 64-4096)
        #[arg(long)]
        iterations_per_thread: Option<u32>,

        /// Dump generated shader source to file for debugging
        #[arg(long)]
        dump_shader: bool,

        /// Export a transparent PNG (alpha channel) instead of compositing on the background
        #[arg(long)]
        transparent: bool,

        /// For transparent export, use premultiplied alpha (vs the default straight-alpha flatten-over-black reconstruction)
        #[arg(long)]
        premultiplied: bool,

        /// Force a render engine (auto = size-based routing; flamerenderer / highres force one path — for parity testing)
        #[arg(long, value_enum, default_value_t = fractal_flame_wgpu::ExportEngine::Auto)]
        engine: fractal_flame_wgpu::ExportEngine,
    },

    /// Export animation to video (pipes directly to ffmpeg, requires ffmpeg in PATH)
    ExportAnimation {
        /// Input .fflame config file (base flame state)
        #[arg(short, long)]
        config: String,

        /// Input .anim animation file
        #[arg(short, long)]
        animation: String,

        /// Output video file path (e.g., output.mp4)
        #[arg(short, long)]
        output: String,

        /// Frame width (default: 1920)
        #[arg(short, long, default_value = "1920")]
        width: u32,

        /// Frame height (default: 1080)
        #[arg(short = 'H', long, default_value = "1080")]
        height: u32,

        /// Frames per second (default: 30)
        #[arg(long, default_value = "30")]
        fps: u32,

        /// Iterations per thread (default: 256)
        #[arg(long, default_value = "256")]
        iterations_per_thread: u32,

        /// Video codec: h264, h265, or vp9 (default: h265)
        #[arg(long, default_value = "h265")]
        video_codec: String,

        /// Hardware acceleration: none, nvenc, qsv, amf, videotoolbox (default: none)
        #[arg(long, default_value = "none")]
        hw_accel: String,

        /// Video quality (CRF): 0-51, lower = better (default: 18)
        #[arg(long, default_value = "18")]
        video_quality: u8,

        /// Audio file to include in export (MP3, WAV, FLAC, OGG)
        #[arg(long)]
        audio: Option<String>,

        /// Audio time offset in seconds (negative = skip into audio, positive = delay start)
        #[arg(long, default_value = "0.0")]
        audio_offset: f64,

        /// Audio fade in duration in seconds
        #[arg(long, default_value = "0.0")]
        audio_fade_in: f64,

        /// Audio fade out duration in seconds
        #[arg(long, default_value = "0.0")]
        audio_fade_out: f64,

        /// Audio bitrate in kbps (default: 192)
        #[arg(long, default_value = "192")]
        audio_bitrate: u32,
    },
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::ListEncoders) => {
                // List available FFmpeg encoders
                fractal_flame_wgpu::animation::export::print_available_encoders();
            }
            Some(Commands::Export { input, output, width, height, category, iterations_per_thread, dump_shader, transparent, premultiplied, engine }) => {
                // Run in headless export mode
                fractal_flame_wgpu::export_mode(&input, &output, width, height, category, iterations_per_thread, dump_shader, transparent, premultiplied, engine);
            }
            Some(Commands::ExportAnimation { config, animation, output, width, height, fps, iterations_per_thread, video_codec, hw_accel, video_quality, audio, audio_offset, audio_fade_in, audio_fade_out, audio_bitrate }) => {
                // Parse video codec
                let codec = match video_codec.to_lowercase().as_str() {
                    "h264" => fractal_flame_wgpu::animation::export::VideoCodec::H264,
                    "h265" | "hevc" => fractal_flame_wgpu::animation::export::VideoCodec::H265,
                    "vp9" => fractal_flame_wgpu::animation::export::VideoCodec::VP9,
                    _ => {
                        eprintln!("Unknown video codec '{}'. Using h264.", video_codec);
                        fractal_flame_wgpu::animation::export::VideoCodec::H264
                    }
                };

                // Parse hardware acceleration
                let hardware_accel = match hw_accel.to_lowercase().as_str() {
                    "none" | "software" | "cpu" => fractal_flame_wgpu::animation::export::HardwareAccel::None,
                    "nvenc" | "nvidia" => fractal_flame_wgpu::animation::export::HardwareAccel::Nvenc,
                    "qsv" | "quicksync" | "intel" => fractal_flame_wgpu::animation::export::HardwareAccel::Qsv,
                    "amf" | "amd" => fractal_flame_wgpu::animation::export::HardwareAccel::Amf,
                    "videotoolbox" | "vt" | "apple" => fractal_flame_wgpu::animation::export::HardwareAccel::VideoToolbox,
                    _ => {
                        eprintln!("Unknown hardware acceleration '{}'. Using software encoding.", hw_accel);
                        fractal_flame_wgpu::animation::export::HardwareAccel::None
                    }
                };

                // Validate hardware accel supports codec
                if !hardware_accel.supports_codec(codec) {
                    eprintln!("Warning: {} does not support {}. Falling back to software encoding.",
                        hardware_accel.display_name(), codec.display_name());
                }

                // Build video settings (always required - pipes directly to ffmpeg)
                let video_settings = fractal_flame_wgpu::animation::export::VideoEncodingSettings {
                    codec,
                    hardware_accel: if hardware_accel.supports_codec(codec) {
                        hardware_accel
                    } else {
                        fractal_flame_wgpu::animation::export::HardwareAccel::None
                    },
                    quality: video_quality,
                    preset: fractal_flame_wgpu::animation::export::EncodingPreset::default(),  // TODO: Add CLI args
                    tune: fractal_flame_wgpu::animation::export::EncodingTune::default(),
                };

                // Build audio config (if audio file specified)
                let audio_config = audio.map(|file| {
                    fractal_flame_wgpu::animation::export::AudioExportConfig {
                        file: std::path::PathBuf::from(file),
                        offset: audio_offset,
                        fade_in: audio_fade_in,
                        fade_out: audio_fade_out,
                        bitrate_kbps: audio_bitrate,
                    }
                });

                // Run animation export mode
                fractal_flame_wgpu::export_animation_mode(&config, &animation, &output, width, height, fps, iterations_per_thread, video_settings, audio_config);
            }
            None => {
                // Run normal GUI mode
                desktop_main();
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        desktop_main();
    }
}