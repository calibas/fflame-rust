/// Simple CPU benchmark for fractal flame algorithms
///
/// Usage:
///   cargo run --release --bin simple_benchmark

use fractal_flame_wgpu::scene::{
    presets::PresetLibrary,
    transforms::*,
};
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
        let affine_point = transform.apply_affine(point);
        let var_point = transform.apply_variations(affine_point);

        // Blend color
        let s = transform.color_speed;
        color = [
            color[0] * (1.0 - s) + transform.color[0] * s,
            color[1] * (1.0 - s) + transform.color[1] * s,
            color[2] * (1.0 - s) + transform.color[2] * s,
        ];

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
    let point = Point::new(0.7, 0.3);
    let iterations = 1_000_000;

    for i in 0..MAX_VARIATIONS {
        let var_type = VariationType::from_index(i);
        let start = Instant::now();

        let mut result = point;
        for _ in 0..iterations {
            result = var_type.apply(result);
        }

        let elapsed = start.elapsed();
        let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!("{:?}: {:.2} M ops/sec (result: {:.4}, {:.4})",
            var_type,
            ops_per_sec / 1_000_000.0,
            result.x,
            result.y
        );
    }
}

fn benchmark_affine() {
    let transform = Transform {
        a: 0.7,
        b: 0.2,
        c: -0.2,
        d: 0.8,
        e: 0.1,
        f: -0.1,
        g: 0.0,
        weight: 1.0,
        variations: {
            let mut vars = [0.0; MAX_VARIATIONS];
            vars[0] = 1.0; // Linear only
            vars
        },
        color: [1.0, 0.5, 0.3],
        color_speed: 0.5,
    };

    let point = Point::new(0.5, 0.5);
    let iterations = 10_000_000;

    // Affine only
    let start = Instant::now();
    let mut result = point;
    for _ in 0..iterations {
        result = transform.apply_affine(result);
    }
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Affine only: {:.2} M ops/sec", ops_per_sec / 1_000_000.0);
    println!("  Result: ({:.4}, {:.4})", result.x, result.y);

    // Affine + variations
    let start = Instant::now();
    let mut result = point;
    for _ in 0..iterations {
        let affine_point = transform.apply_affine(result);
        result = transform.apply_variations(affine_point);
    }
    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Affine + variations: {:.2} M ops/sec", ops_per_sec / 1_000_000.0);
    println!("  Result: ({:.4}, {:.4})", result.x, result.y);
}
