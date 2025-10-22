/// Criterion benchmarks for CPU flame iteration performance
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fractal_flame_wgpu::scene::{
    presets::PresetLibrary,
    transforms::*,
};

fn bench_cpu_iteration(c: &mut Criterion) {
    let presets = PresetLibrary::new();

    let mut group = c.benchmark_group("cpu_iteration");

    for config in presets.configs().iter().take(3) {
        let flame = &config.flame;

        group.bench_with_input(
            BenchmarkId::new("single_iter", &flame.name),
            flame,
            |b, flame| {
                let mut point = Point::new(0.5, 0.5, 0.0);
                let mut color = [0.5, 0.5, 0.5];

                b.iter(|| {
                    (point, color) = flame.iterate(black_box(point), black_box(color));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("100_iters", &flame.name),
            flame,
            |b, flame| {
                b.iter(|| {
                    let mut point = Point::new(0.5, 0.5, 0.0);
                    let mut color = [0.5, 0.5, 0.5];

                    for _ in 0..100 {
                        (point, color) = flame.iterate(black_box(point), black_box(color));
                    }

                    (point, color)
                });
            },
        );
    }

    group.finish();
}

fn bench_variations(c: &mut Criterion) {
    let mut group = c.benchmark_group("variations");
    let point = Point::new(0.7, 0.3, 0.5);

    // Benchmark each variation function
    for i in 0..24 {
        let var_type = VariationType::from_index(i);
        let name = format!("{:?}", var_type);

        group.bench_with_input(BenchmarkId::new("apply", &name), &var_type, |b, var_type| {
            b.iter(|| var_type.apply(black_box(point)));
        });
    }

    group.finish();
}

fn bench_affine_transform(c: &mut Criterion) {
    let transform = Transform {
        a: 0.7,
        b: 0.2,
        c: -0.2,
        d: 0.8,
        e: 0.1,
        f: -0.1,
        g: 0.05,
        weight: 1.0,
        variations: [0.5; 24],
        color: [1.0, 0.5, 0.3],
        color_speed: 0.5,
    };

    c.bench_function("affine_transform", |b| {
        let point = Point::new(0.5, 0.5, 0.0);
        b.iter(|| transform.apply_affine(black_box(point)));
    });

    c.bench_function("apply_variations", |b| {
        let point = Point::new(0.5, 0.5, 0.0);
        b.iter(|| transform.apply_variations(black_box(point)));
    });
}

fn bench_point_calculations(c: &mut Criterion) {
    let point = Point::new(0.7, 0.3, 0.2);

    c.bench_function("point_r", |b| {
        b.iter(|| black_box(point).r());
    });

    c.bench_function("point_r_squared", |b| {
        b.iter(|| black_box(point).r_squared());
    });

    c.bench_function("point_theta", |b| {
        b.iter(|| black_box(point).theta());
    });

    c.bench_function("point_phi", |b| {
        b.iter(|| black_box(point).phi());
    });
}

criterion_group!(
    benches,
    bench_cpu_iteration,
    bench_variations,
    bench_affine_transform,
    bench_point_calculations
);
criterion_main!(benches);
