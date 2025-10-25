//! Test parsing an Apophysis .flame import

use fractal_flame_wgpu::apophysis_xml::parse_flame_xml;

fn main() -> anyhow::Result<()> {
    println!("Testing Apophysis Import → .fflame Export");
    println!("==========================================\n");

    // Read and parse the Apophysis file
    let xml = std::fs::read_to_string("tests/visual/apophysis/spherical-apo.flame")?;
    let configs = parse_flame_xml(&xml)?;

    println!("✓ Parsed {} config(s)", configs.len());

    if configs.is_empty() {
        anyhow::bail!("No configs found");
    }

    let config = &configs[0];
    println!("✓ Using config: {}\n", config.flame.name);

    // Save as .fflame
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write("spherical_from_apophysis.fflame", json)?;
    println!("✓ Saved as spherical_from_apophysis.fflame\n");
    println!("Now run:");
    println!("  cargo run --release -- export -i spherical_from_apophysis.fflame -o spherical_from_apophysis.png\n");

    Ok(())
}
