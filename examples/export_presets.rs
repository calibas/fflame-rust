use fractal_flame_wgpu::scene::presets::*;
use fractal_flame_wgpu::scene::palette::ColorMode;
use fractal_flame_wgpu::config::FractalConfig;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let flames = vec![
        create_simple_flame(),
        create_spherical_flame(),
        create_spiral_flame(),
        create_julia_flame(),
        create_complex_flame(),
        create_flower_of_life(),
    ];

    for flame in flames {
        // Use Default and override only the flame
        let mut config = FractalConfig::default();
        config.flame = flame;
        config.palette_index = 1;

        let filename = format!("assets/presets/{}.fflame", config.flame.name.to_lowercase().replace(" ", "_"));
        let json = serde_json::to_string_pretty(&config)?;
        fs::write(&filename, json)?;
        println!("Exported: {}", filename);
    }

    println!("All presets exported successfully!");
    Ok(())
}
