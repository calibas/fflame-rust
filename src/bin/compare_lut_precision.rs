use fractal_flame_wgpu::scene::tonemap::ToneCurve;
use half::f16;

fn main() {
    let curve = ToneCurve::linear();
    let lut_bytes = curve.generate_lut();

    println!("Testing precision loss through f16 conversion:");
    println!("Index | Input (f32) | Stored (f16) | Stored as f32 | Error");
    println!("------|-------------|--------------|---------------|-------");

    let mut max_error = 0.0f32;
    let mut sum_error = 0.0f32;

    for i in 0..256 {
        let input = i as f32 / 255.0;

        // Read what's in the LUT
        let offset = i * 4 * 2;
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let stored_f16 = f16::from_le_bytes(r_bytes);
        let stored_f32 = stored_f16.to_f32();

        let error = (stored_f32 - input).abs();
        max_error = max_error.max(error);
        sum_error += error;

        if i < 10 || i > 245 || (i >= 125 && i <= 130) {
            println!("{:5} | {:11.8} | {:12} | {:13.8} | {:.10}",
                     i, input, stored_f16, stored_f32, error);
        }
    }

    let avg_error = sum_error / 256.0;
    println!("\nMax error: {:.10}", max_error);
    println!("Avg error: {:.10}", avg_error);
    println!("\nFor reference:");
    println!("  f16 precision: ~0.0009765625 (1/1024)");
    println!("  f32 precision: ~0.00000001");
}
