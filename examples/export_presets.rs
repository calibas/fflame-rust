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
        let config = FractalConfig {
            flame,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            density_scale: 1.0,
            speed_factor: 0.5,
            color_mode: ColorMode::Transform,
            palette_index: 1,
            background_color: [0.0, 0.0, 0.0],
        };

        let filename = format!("assets/presets/{}.flame", config.flame.name.to_lowercase().replace(" ", "_"));
        let json = serde_json::to_string_pretty(&config)?;
        fs::write(&filename, json)?;
        println!("Exported: {}", filename);
    }

    println!("All presets exported successfully!");
    Ok(())
}
