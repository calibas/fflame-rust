use fractal_flame_wgpu::scene::tonemap::ToneCurve;
use half::f16;

fn main() {
    let curve = ToneCurve::linear();
    let lut_bytes = curve.generate_lut();

    // Convert bytes back to f16 values
    println!("Linear curve LUT (f16 format):");
    println!("First 10 entries:");
    for i in 0..10 {
        let offset = i * 4 * 2; // 4 components * 2 bytes per f16
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let r = f16::from_le_bytes(r_bytes);
        let expected = i as f32 / 255.0;
        println!("  Entry {}: {} (expected {}, diff {})", i, r.to_f32(), expected, (r.to_f32() - expected).abs());
    }

    println!("\nMiddle entries:");
    for i in 125..130 {
        let offset = i * 4 * 2;
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let r = f16::from_le_bytes(r_bytes);
        let expected = i as f32 / 255.0;
        println!("  Entry {}: {} (expected {}, diff {})", i, r.to_f32(), expected, (r.to_f32() - expected).abs());
    }

    println!("\nLast 10 entries:");
    for i in 246..256 {
        let offset = i * 4 * 2;
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let r = f16::from_le_bytes(r_bytes);
        let expected = i as f32 / 255.0;
        println!("  Entry {}: {} (expected {}, diff {})", i, r.to_f32(), expected, (r.to_f32() - expected).abs());
    }

    // Test what linear interpolation would give us
    println!("\nSimulated linear interpolation between entries:");
    for test_val in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let texel_coord: f32 = test_val * 255.0;
        let idx_low = texel_coord.floor() as usize;
        let idx_high = (texel_coord.ceil() as usize).min(255);
        let fract = texel_coord.fract();

        let offset_low = idx_low * 4 * 2;
        let offset_high = idx_high * 4 * 2;

        let r_low = f16::from_le_bytes([lut_bytes[offset_low], lut_bytes[offset_low + 1]]);
        let r_high = f16::from_le_bytes([lut_bytes[offset_high], lut_bytes[offset_high + 1]]);

        let interpolated = r_low.to_f32() * (1.0 - fract) + r_high.to_f32() * fract;
        let diff = (interpolated - test_val).abs();

        println!("  Input {}: interpolate between texel {} ({}) and {} ({}) = {} (diff {})",
                 test_val, idx_low, r_low.to_f32(), idx_high, r_high.to_f32(), interpolated, diff);
    }
}
