/// Criterion benchmarks for CPU flame iteration performance
///
/// These benchmarks focus on CPU-side operations that run in tight loops:
/// - Affine transformations
/// - Point helper calculations (r, theta, phi)
/// - Transform selection (weighted random)
/// - Color blending
///
/// Note: Variation functions run on GPU (WGSL shaders), not CPU.
///
/// Usage:
///   cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fractal_flame_wgpu::scene::{
    presets::PresetLibrary,
    transforms::*,
};
use fractal_flame_wgpu::variations::VariationRegistry;

fn bench_affine_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("affine_transform");

    // Create test transform
    let mut transform = Transform::new();
    transform.a = 0.7;
    transform.b = 0.2;
    transform.c = -0.2;
    transform.d = 0.8;
    transform.e = 0.1;
    transform.f = -0.1;
    transform.g = 0.05;

    let point = Point::new(0.5, 0.5);

    // Benchmark affine transformation (this is the hottest path in CPU iteration)
    group.bench_function("affine_2x3_matrix", |b| {
        b.iter(|| {
            Point::new(
                black_box(transform.a * point.x + transform.b * point.y + transform.e),
                black_box(transform.c * point.x + transform.d * point.y + transform.f),
            )
        });
    });

    // Benchmark complete affine with all components
    group.bench_function("affine_full", |b| {
        b.iter(|| {
            let _z = black_box(transform.g);
            Point::new(
                black_box(transform.a * point.x + transform.b * point.y + transform.e),
                black_box(transform.c * point.x + transform.d * point.y + transform.f),
            )
        });
    });

    group.finish();
}

fn bench_point_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("point_helpers");

    let point = Point::new(0.7, 0.3);

    // These are used heavily in variation calculations
    group.bench_function("r_squared", |b| {
        b.iter(|| black_box(point).r_squared());
    });

    group.bench_function("r", |b| {
        b.iter(|| black_box(point).r());
    });

    group.bench_function("theta", |b| {
        b.iter(|| black_box(point).theta());
    });

    group.bench_function("phi", |b| {
        b.iter(|| black_box(point).phi());
    });

    group.finish();
}

fn bench_color_blending(c: &mut Criterion) {
    let mut group = c.benchmark_group("color_blending");

    // Test different color speeds
    for color_speed in [0.0, 0.5, 1.0] {
        group.bench_with_input(
            BenchmarkId::new("palette_position", color_speed),
            &color_speed,
            |b, &speed| {
                let current_pos = 0.3;
                let transform_color = 0.7;

                b.iter(|| {
                    black_box(current_pos * (1.0 - speed) + transform_color * speed)
                });
            },
        );
    }

    group.finish();
}

fn bench_preset_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("preset_iteration");

    let presets = PresetLibrary::new();

    // Benchmark first 3 presets
    for config in presets.presets().iter().take(3) {
        let flame = &config.flame;

        group.bench_with_input(
            BenchmarkId::new("single_transform_iter", &flame.name),
            flame,
            |b, flame| {
                let transform = &flame.transforms[0];
                let mut point = Point::new(0.5, 0.5);
                let mut color_pos = 0.5;

                b.iter(|| {
                    // Affine transformation
                    let affine_point = Point::new(
                        transform.a * point.x + transform.b * point.y + transform.e,
                        transform.c * point.x + transform.d * point.y + transform.f,
                    );

                    // Simplified variation application (just linear for benchmark)
                    if let Some(&weight) = transform.variations.get("linear") {
                        point = Point::new(
                            affine_point.x * weight,
                            affine_point.y * weight,
                        );
                    } else {
                        point = affine_point;
                    }

                    // Color blending
                    let s = transform.color_speed;
                    color_pos = color_pos * (1.0 - s) + transform.color * s;

                    black_box((point, color_pos))
                });
            },
        );
    }

    group.finish();
}

fn bench_variation_weight_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("variation_lookup");

    // Create transform with several variations
    let mut transform = Transform::new();
    transform.variations.insert("linear".to_string(), 0.5);
    transform.variations.insert("sinusoidal".to_string(), 0.3);
    transform.variations.insert("spherical".to_string(), 0.2);

    // Benchmark HashMap lookup (happens for each variation in each iteration)
    group.bench_function("hashmap_get", |b| {
        b.iter(|| {
            black_box(transform.variations.get("linear"))
        });
    });

    // Benchmark iteration over all variations
    group.bench_function("hashmap_iter", |b| {
        b.iter(|| {
            for (name, weight) in black_box(&transform.variations) {
                black_box((name, weight));
            }
        });
    });

    group.finish();
}

fn bench_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_operations");

    let registry = VariationRegistry::new();

    // Benchmark registry lookups (happens during shader compilation)
    group.bench_function("registry_get", |b| {
        b.iter(|| {
            black_box(registry.get("linear"))
        });
    });

    group.bench_function("registry_get_is_some", |b| {
        b.iter(|| {
            black_box(registry.get("linear").is_some())
        });
    });

    group.finish();
}

fn bench_transform_struct_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("transform_creation");

    // Benchmark creating new transforms (happens when adding transforms in UI)
    group.bench_function("new_transform", |b| {
        b.iter(|| {
            black_box(Transform::new())
        });
    });

    // Benchmark cloning transforms (for undo/redo)
    let mut transform = Transform::new();
    transform.variations.insert("linear".to_string(), 1.0);
    transform.variations.insert("sinusoidal".to_string(), 0.5);

    group.bench_function("clone_transform", |b| {
        b.iter(|| {
            black_box(transform.clone())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_affine_transform,
    bench_point_calculations,
    bench_color_blending,
    bench_preset_iteration,
    bench_variation_weight_lookup,
    bench_registry_operations,
    bench_transform_struct_creation,
);
criterion_main!(benches);
