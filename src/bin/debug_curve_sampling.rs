/// Debug tool to test what the GPU samples from the curve LUT
/// This creates test patterns to verify GPU sampling behavior
use fractal_flame_wgpu::scene::tonemap::ToneCurve;

fn main() {
    println!("=== Curve LUT Sampling Debug ===\n");

    let curve = ToneCurve::linear();

    println!("Testing what values the GPU would sample at different coordinates:\n");
    println!("{:>15} {:>15} {:>15} {:>15}", "Input Coord", "Expected Output", "Actual (nearest)", "Actual (linear)");
    println!("{}", "-".repeat(65));

    // Test coordinates that would be used when sampling colors
    let test_coords = [
        0.0, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999, 1.0,
        // Problem zones - between texels
        0.498, 0.499, 0.500, 0.501, 0.502,
    ];

    for &coord in &test_coords {
        let expected = curve.evaluate(coord);

        // Simulate nearest-neighbor sampling
        let nearest_index = (coord * 255.0).round() as usize;
        let nearest_index = nearest_index.min(255);
        let nearest_value = simulate_lut_read(nearest_index);

        // Simulate linear interpolation sampling
        let linear_value = simulate_linear_sample(&curve, coord);

        let nearest_error = (nearest_value - expected).abs();
        let linear_error = (linear_value - expected).abs();

        println!("{:15.6} {:15.6} {:15.6} {:15.6}",
                 coord, expected, nearest_value, linear_value);

        if nearest_error > 0.001 || linear_error > 0.001 {
            println!("                       ^^^ Error: nearest={:.6} linear={:.6}",
                     nearest_error, linear_error);
        }
    }

    println!("\n{}", "-".repeat(65));
    println!("\nKey insights:");
    println!("1. Nearest-neighbor should be exact for integer/255 coordinates");
    println!("2. Linear interpolation should be close for all coordinates");
    println!("3. If GPU results differ, it's a coordinate mapping issue");

    // Test coordinate mapping
    println!("\n=== Coordinate Mapping Analysis ===\n");
    println!("When GPU samples at coordinate C:");
    println!("  Texel index formula: floor(C * 255)");
    println!("  Linear blend factor: fract(C * 255)\n");

    println!("{:>10} {:>15} {:>15} {:>15}", "Coord", "Texel 0 (idx)", "Texel 1 (idx)", "Blend Factor");
    println!("{}", "-".repeat(60));

    for &coord in &[0.0, 0.25, 0.498, 0.5, 0.502, 0.75, 1.0] {
        let scaled = coord * 255.0f32;
        let idx0 = scaled.floor() as usize;
        let idx1 = (idx0 + 1).min(255);
        let blend = scaled.fract();

        println!("{:10.6} {:15} {:15} {:15.6}", coord, idx0, idx1, blend);
    }

    println!("\n=== Test Complete ===");
}

fn simulate_lut_read(index: usize) -> f32 {
    // Simulate reading from LUT (should be identity for linear curve)
    index as f32 / 255.0
}

fn simulate_linear_sample(curve: &ToneCurve, coord: f32) -> f32 {
    // Simulate GPU linear interpolation
    let scaled = coord * 255.0;
    let idx0 = scaled.floor() as usize;
    let idx1 = (idx0 + 1).min(255);
    let blend = scaled.fract();

    let val0 = curve.evaluate(idx0 as f32 / 255.0);
    let val1 = curve.evaluate(idx1 as f32 / 255.0);

    val0 * (1.0 - blend) + val1 * blend
}
