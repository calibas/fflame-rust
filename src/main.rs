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
    },

    /// Export animation as PNG sequence (high-quality frame-by-frame rendering)
    ExportAnimation {
        /// Input .fflame config file (base flame state)
        #[arg(short, long)]
        config: String,

        /// Input .anim animation file
        #[arg(short, long)]
        animation: String,

        /// Output directory for frames
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

        /// Export transparent PNGs
        #[arg(long)]
        transparent: bool,

        /// Iterations per thread (default: 256)
        #[arg(long, default_value = "256")]
        iterations_per_thread: u32,
    },
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::Export { input, output, width, height, category, iterations_per_thread }) => {
                // Run in headless export mode
                fractal_flame_wgpu::export_mode(&input, &output, width, height, category, iterations_per_thread);
            }
            Some(Commands::ExportAnimation { config, animation, output, width, height, fps, transparent, iterations_per_thread }) => {
                // Run animation export mode
                fractal_flame_wgpu::export_animation_mode(&config, &animation, &output, width, height, fps, transparent, iterations_per_thread);
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