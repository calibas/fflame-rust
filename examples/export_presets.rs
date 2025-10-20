use fractal_flame_wgpu::scene::presets::*;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let presets = vec![
        create_simple_flame(),
        create_spherical_flame(),
        create_spiral_flame(),
        create_julia_flame(),
        create_complex_flame(),
    ];

    for preset in presets {
        let filename = format!("assets/presets/{}.flame", preset.name.to_lowercase().replace(" ", "_"));
        let json = serde_json::to_string_pretty(&preset)?;
        fs::write(&filename, json)?;
        println!("Exported: {}", filename);
    }

    println!("All presets exported successfully!");
    Ok(())
}
