//! Test Apophysis .flame XML import

use fractal_flame_wgpu::apophysis_xml::parse_flame_xml;

fn main() -> anyhow::Result<()> {
    println!("Testing Apophysis .flame XML Import");
    println!("====================================\n");

    // Read the test file
    let xml = std::fs::read_to_string("tests/visual/apophysis/spherical-apo.flame")?;

    // Parse it
    let configs = parse_flame_xml(&xml)?;

    println!("✓ Successfully parsed {} flame(s)\n", configs.len());

    for (i, config) in configs.iter().enumerate() {
        println!("Flame {}:", i + 1);
        println!("  Name: {}", config.flame.name);
        println!("  Transforms: {}", config.flame.transforms.len());
        println!("  Zoom: {:.2}", config.zoom);
        println!("  Pan: ({:.4}, {:.4})", config.pan_x, config.pan_y);
        println!("  Background: {:?}", config.background_color);
        println!("  Gamma: {}", config.gamma);

        for (j, xform) in config.flame.transforms.iter().enumerate() {
            println!("\n  Transform {}:", j + 1);
            println!("    Weight: {}", xform.weight);
            println!("    Color position: {}", xform.color);
            println!("    Color Speed: {}", xform.color_speed);
            println!("    Affine: a={} b={} c={} d={} e={} f={}",
                xform.a, xform.b, xform.c, xform.d, xform.e, xform.f);
            println!("    Variations: {}", xform.variations.len());
            for (name, weight) in &xform.variations {
                println!("      {} = {}", name, weight);
            }
        }

        if let Some(ref palette) = config.palette {
            println!("\n  Palette: {} colors", palette.stops.len());
            println!("    First color: {:?}", palette.stops[0].color);
            println!("    Last color: {:?}", palette.stops.last().unwrap().color);
        }

        println!();
    }

    Ok(())
}
