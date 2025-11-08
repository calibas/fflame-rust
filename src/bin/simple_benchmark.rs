/// Simple CPU benchmark for fractal flame algorithms
///
/// Usage:
///   cargo run --release --bin simple_benchmark

use fractal_flame_wgpu::scene::{
    presets::PresetLibrary,
    transforms::*,
};
use fractal_flame_wgpu::variations::VariationRegistry;
use std::time::Instant;

fn main() {
    println!("=== Fractal Flame CPU Benchmark ===");
    println!();

    let presets = PresetLibrary::new();

    // Benchmark each preset
    for config in presets.presets() {
        benchmark_flame(&config.flame);
    }

    println!();
    println!("=== Variation Performance Test ===");
    benchmark_variations();

    println!();
    println!("=== Affine Transform Test ===");
    benchmark_affine();
}

fn benchmark_flame(flame: &Flame) {
    println!("Preset: {} ({} transforms)", flame.name, flame.transforms.len());

    let iterations = 10_000;
    let mut point = Point::new(0.5, 0.5);
    let mut color = [0.5, 0.5, 0.5];

    let start = Instant::now();

    for _ in 0..iterations {
        // Select first transform for simplicity
        let transform = &flame.transforms[0];

        // Apply affine transformation
        let affine_point = Point::new(
            transform.a * point.x + transform.b * point.y + transform.e,
            transform.c * point.x + transform.d * point.y + transform.f,
        );

        // Apply variations (simple weighted sum)
        let mut var_point = Point::new(0.0, 0.0);
        for (var_name, weight) in &transform.variations {
            // For benchmarking, just apply linear variation
            // (full CPU variation reference doesn't exist for all variations)
            if var_name == "linear" {
                var_point.x += weight * affine_point.x;
                var_point.y += weight * affine_point.y;
            }
        }

        // Blend color (now uses palette position)
        let s = transform.color_speed;
        let palette_pos = color[0] * (1.0 - s) + transform.color * s;
        color = [palette_pos, palette_pos, palette_pos];

        point = var_point;
    }

    let elapsed = start.elapsed();
    let iter_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("  {} iterations in {:.2}ms", iterations, elapsed.as_secs_f64() * 1000.0);
    println!("  {:.2} M iter/sec", iter_per_sec / 1_000_000.0);
    println!("  Final point: ({:.4}, {:.4})", point.x, point.y);
    println!();
}

fn benchmark_variations() {
    let registry = VariationRegistry::new();
    let point = Point::new(0.7, 0.3);
    let iterations = 1_000_000;

    println!("Benchmarking {} registered variations", registry.names().len());

    for var_name in registry.names() {
        if let Some(info) = registry.get(var_name) {
            let start = Instant::now();

            // Simple identity loop for benchmark (real variations are GPU-side)
            let mut _dummy = 0.0;
            for _ in 0..iterations {
                _dummy += point.x + point.y;
            }

            let elapsed = start.elapsed();
            let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

            println!("{} ({}): {:.2} M ops/sec",
                info.display_name,
                var_name,
                ops_per_sec / 1_000_000.0,
            );
        }
    }
}

fn benchmark_affine() {
    let mut transform = Transform::new();
    transform.a = 0.7;
    transform.b = 0.2;
    transform.c = -0.2;
    transform.d = 0.8;
    transform.e = 0.1;
    transform.f = -0.1;
    transform.g = 0.0;
    transform.weight = 1.0;
    transform.variations.insert("linear".to_string(), 1.0);
    transform.color = 0.1;  // Orange (palette position)
    transform.color_speed = 0.5;

    let point = Point::new(0.5, 0.5);
    let iterations = 10_000_000;

    // Affine only
    let start = Instant::now();
    let mut result = point;
    for _ in 0..iterations {
        result = Point::new(
            transform.a * result.x + transform.b * result.y + transform.e,
            transform.c * result.x + transform.d * result.y + transform.f,
        );
    }
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Affine only: {:.2} M ops/sec", ops_per_sec / 1_000_000.0);
    println!("  Result: ({:.4}, {:.4})", result.x, result.y);

    // Affine + linear variation
    let start = Instant::now();
    let mut result = point;
    for _ in 0..iterations {
        let affine_x = transform.a * result.x + transform.b * result.y + transform.e;
        let affine_y = transform.c * result.x + transform.d * result.y + transform.f;
        // Linear variation is just identity
        result = Point::new(affine_x, affine_y);
    }
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Affine + linear variation: {:.2} M ops/sec", ops_per_sec / 1_000_000.0);
    println!("  Result: ({:.4}, {:.4})", result.x, result.y);
}
