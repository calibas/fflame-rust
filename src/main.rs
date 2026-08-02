// Release builds on Windows are GUI-subsystem: launching the app from
// Explorer or a shortcut must not open an empty black console beside it.
//
// The same binary is also the CLI (`export`, `generate`,
// `export-animation`), and a GUI-subsystem process has no console at
// all — so `--help` would print into the void. `attach_parent_console`
// below puts that back when there is a terminal to attach to.
//
// Debug builds keep the console subsystem, so `cargo run` still shows
// panics and log output without ceremony.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

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

/// Parse a seed onto the ring: any integer, reduced modulo 2^64.
///
/// Seeds are a circle — `u64::MAX + 1` is `0`, and `-1` is the step
/// before `0` (= `u64::MAX`). `u64` alone rejected both `-1` and
/// anything past the top, which made the CLI the odd one out: the
/// browser already accepted them (wasm-bindgen reduces BigInt mod
/// 2^64), so the same seed worked on the web and failed here.
///
/// `i128 as u64` truncates to the low 64 bits — two's complement, the
/// same reduction as JavaScript's `BigInt.asUintN(64, x)` and Python's
/// `x & (2**64 - 1)`.
#[cfg(not(target_arch = "wasm32"))]
fn seed_on_the_ring(s: &str) -> Result<u64, String> {
    s.parse::<i128>()
        .map(|v| v as u64)
        .map_err(|_| format!("`{s}` is not a whole number"))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Subcommand)]
enum Commands {
    /// List available FFmpeg encoders and hardware acceleration options
    ListEncoders,

    /// Run a flame script to produce a .fflame (headless)
    Generate {
        /// Script file (.rhai)
        #[arg(short, long)]
        script: String,

        /// Output .fflame (defaults to the script name)
        #[arg(short, long)]
        output: Option<String>,

        /// Seed — the same script and seed always produce the same
        /// flame. Wraps modulo 2^64, so -1 is the last seed
        // allow_hyphen_values: without it clap reads the leading `-` of
        // `--seed -1` as the start of another flag and never reaches the
        // parser.
        #[arg(long, default_value_t = 0, value_parser = seed_on_the_ring, allow_hyphen_values = true)]
        seed: u64,

        /// Starting config for modifier scripts
        #[arg(short, long)]
        base: Option<String>,

        /// Set a declared script parameter, e.g. --set copies=5
        #[arg(long = "set", value_name = "KEY=VALUE")]
        sets: Vec<String>,

        /// List the script's declared parameters and exit
        #[arg(long)]
        list_params: bool,
    },

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

        /// Omit metadata from the exported PNG. Exports normally embed the
        /// complete config, which makes them reproducible — and shares the
        /// flame with anyone who has the image.
        #[arg(long)]
        strip_metadata: bool,

        /// 2× supersampled antialiasing: render at double resolution and box-filter down (with firefly clamp). ~4× render cost.
        #[arg(long, default_value_t = false)]
        supersample: bool,

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


/// Re-attach to the terminal that launched us, if there was one.
///
/// A GUI-subsystem process starts with no console and invalid standard
/// handles, so every `println!` goes nowhere. `AttachConsole` with
/// `ATTACH_PARENT_PROCESS` borrows the parent's console when the parent
/// has one — a `cmd`, PowerShell or terminal window — and does nothing
/// when it does not, which is exactly the Explorer / shortcut case.
///
/// Declared inline rather than pulling in `windows-sys` for two
/// symbols. `AttachConsole` is kernel32 and has been since Windows XP.
///
/// # Why the handles are re-bound
///
/// Attaching gives the process a console but does **not** repoint the
/// standard handles it started with — those are still the invalid ones
/// from launch. Rust's `Stdout` resolves `STD_OUTPUT_HANDLE` lazily and
/// then caches it, so this has to happen before the first print or the
/// output is lost with no error anywhere.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn GetStdHandle(nStdHandle: u32) -> isize;
        fn SetStdHandle(nStdHandle: u32, hHandle: isize) -> i32;
        fn CreateFileA(
            lpFileName: *const u8,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut core::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;
    }

    unsafe {
        // Snapshot the handles BEFORE attaching.
        //
        // This ordering is the whole trick, and getting it wrong is
        // silent. `AttachConsole` REPLACES the standard handles with the
        // console's, so a shell that redirected `> file` loses the file
        // handle — and inspecting the handles afterwards cannot tell
        // that apart from a console it legitimately created, because by
        // then both look like valid console handles.
        //
        // Measured, not assumed: with the check placed after the attach,
        // `--help > file` produced a zero-byte file from both cmd and
        // PowerShell while a pipe still worked.
        let slots = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE, STD_INPUT_HANDLE];
        let before = slots.map(|slot| GetStdHandle(slot));
        let usable = |h: isize| h != 0 && h != INVALID_HANDLE_VALUE;

        // Every stream already goes somewhere real: piped, redirected,
        // or inherited. There is nothing to attach to and nothing to
        // repair, so do not touch anything.
        if before.iter().all(|h| usable(*h)) {
            return;
        }

        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            // No parent console — launched from Explorer or a shortcut.
            // A GUI app with nowhere to print is the correct outcome.
            return;
        }

        for (i, slot) in slots.iter().enumerate() {
            if usable(before[i]) {
                // Put back what the shell gave us, undoing the attach's
                // replacement.
                SetStdHandle(*slot, before[i]);
                continue;
            }
            // Genuinely had nothing: point it at the console we just
            // attached to. `CONOUT$` / `CONIN$` name that console.
            let name: &[u8] = if *slot == STD_INPUT_HANDLE { b"CONIN$\0" } else { b"CONOUT$\0" };
            let h = CreateFileA(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                core::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                0,
            );
            if h != INVALID_HANDLE_VALUE {
                SetStdHandle(*slot, h);
            }
        }
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Before anything can print, including clap's `--help` and its
        // parse errors.
        if std::env::args_os().len() > 1 {
            attach_parent_console();
        }
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::ListEncoders) => {
                // List available FFmpeg encoders
                fractal_flame_wgpu::animation::export::print_available_encoders();
            }
            Some(Commands::Generate { script, output, seed, base, sets, list_params }) => {
                fractal_flame_wgpu::generate_mode(
                    &script,
                    output.as_deref(),
                    seed,
                    base.as_deref(),
                    &sets,
                    list_params,
                );
            }
            Some(Commands::Export { input, output, width, height, category, iterations_per_thread, dump_shader, transparent, premultiplied, supersample, strip_metadata, engine }) => {
                // Before any render: the encoder reads this from a global.
                fractal_flame_wgpu::png_metadata::set_strip_metadata(strip_metadata);
                // Run in headless export mode
                fractal_flame_wgpu::export_mode(&input, &output, width, height, category, iterations_per_thread, dump_shader, transparent, premultiplied, engine, supersample);
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