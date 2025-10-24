/// Debug tool to inspect curve LUT generation
/// This tool shows what data is being generated for the GPU curve LUT
use fractal_flame_wgpu::scene::tonemap::ToneCurve;
use half::f16;

fn main() {
    println!("=== Curve LUT Debug Tool ===\n");

    let curve = ToneCurve::linear();
    let lut_bytes = curve.generate_lut();

    // Parse f16 data (256 pixels * 4 components * 2 bytes)
    assert_eq!(lut_bytes.len(), 256 * 4 * 2);

    println!("Linear curve LUT analysis:");
    println!("Format: Rgba16Float (4 channels, 2 bytes each)\n");

    // Sample key positions
    let sample_indices = [0, 1, 63, 64, 127, 128, 191, 192, 254, 255];

    println!("Key samples:");
    println!("{:>5} {:>10} {:>10} {:>10} {:>10} {:>10}", "Index", "X (i/255)", "R (LUT)", "Expected", "Error", "Error %");
    println!("{}", "-".repeat(70));

    let mut max_error = 0.0f32;
    let mut max_error_idx = 0;
    let mut total_error = 0.0f64;

    for &i in &sample_indices {
        let x = i as f32 / 255.0;
        let expected = curve.evaluate(x);

        // Read R channel (first component)
        let offset = i * 4 * 2; // 4 components * 2 bytes per f16
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let r_f16 = f16::from_le_bytes(r_bytes);
        let r_value = r_f16.to_f32();

        let error = (r_value - expected).abs();
        let error_pct = (error / expected.max(0.001)) * 100.0;

        total_error += error as f64;

        if error > max_error {
            max_error = error;
            max_error_idx = i;
        }

        println!("{:5} {:10.6} {:10.6} {:10.6} {:10.6} {:9.4}%",
                 i, x, r_value, expected, error, error_pct);
    }

    println!("\n{}", "-".repeat(70));
    println!("Statistics:");
    println!("  Max error: {:.8} at index {}", max_error, max_error_idx);
    println!("  Avg error (sampled): {:.8}", total_error / sample_indices.len() as f64);

    // Check G, B, A channels (should be 0, 0, 1)
    println!("\nVerifying other channels (should be G=0, B=0, A=1):");
    let mut g_ok = true;
    let mut b_ok = true;
    let mut a_ok = true;

    for i in [0, 127, 255] {
        let offset = i * 4 * 2;

        let g_bytes = [lut_bytes[offset + 2], lut_bytes[offset + 3]];
        let b_bytes = [lut_bytes[offset + 4], lut_bytes[offset + 5]];
        let a_bytes = [lut_bytes[offset + 6], lut_bytes[offset + 7]];

        let g = f16::from_le_bytes(g_bytes).to_f32();
        let b = f16::from_le_bytes(b_bytes).to_f32();
        let a = f16::from_le_bytes(a_bytes).to_f32();

        println!("  Index {}: G={:.6}, B={:.6}, A={:.6}", i, g, b, a);

        if (g - 0.0).abs() > 0.0001 { g_ok = false; }
        if (b - 0.0).abs() > 0.0001 { b_ok = false; }
        if (a - 1.0).abs() > 0.0001 { a_ok = false; }
    }

    println!("\nChannel verification: G {} B {} A {}",
             if g_ok { "✓" } else { "✗" },
             if b_ok { "✓" } else { "✗" },
             if a_ok { "✓" } else { "✗" });

    // Full scan for any anomalies
    println!("\nFull LUT scan (all 256 entries):");
    let mut anomalies = Vec::new();

    for i in 0..256 {
        let x = i as f32 / 255.0;
        let expected = curve.evaluate(x);

        let offset = i * 4 * 2;
        let r_bytes = [lut_bytes[offset], lut_bytes[offset + 1]];
        let r_value = f16::from_le_bytes(r_bytes).to_f32();

        let error = (r_value - expected).abs();
        if error > 0.0001 {  // Threshold for anomaly
            anomalies.push((i, x, r_value, expected, error));
        }
    }

    if anomalies.is_empty() {
        println!("  ✓ All 256 entries match expected values (within f16 precision)");
    } else {
        println!("  ✗ Found {} anomalies:", anomalies.len());
        for (i, x, r, exp, err) in anomalies.iter().take(10) {
            println!("    Index {}: x={:.6}, LUT={:.6}, Expected={:.6}, Error={:.6}",
                     i, x, r, exp, err);
        }
        if anomalies.len() > 10 {
            println!("    ... and {} more", anomalies.len() - 10);
        }
    }

    println!("\n=== Analysis Complete ===");
    println!("This shows what data is generated on the CPU before GPU upload.");
    println!("If this looks correct, the issue is in GPU sampling, not LUT generation.");
}
