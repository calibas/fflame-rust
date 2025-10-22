/// Example: Display version information
///
/// Usage:
///   cargo run --example show_version

use fractal_flame_wgpu::version::get_version_info;

fn main() {
    let info = get_version_info();

    println!("=== Fractal Flame Renderer - Version Info ===");
    println!();
    println!("{}", info.detailed_info());
    println!();
    println!("Platform: {} ({})", info.platform(), info.architecture());
    println!("Build ID: {}", info.build_id());
    println!("Compact: {}", info.compact_info());
    println!();

    // JSON export
    println!("=== JSON Export ===");
    match serde_json::to_string_pretty(&info) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Failed to serialize: {}", e),
    }
}
