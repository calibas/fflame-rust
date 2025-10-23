use fractal_flame_wgpu::scene::tonemap::ToneCurve;

fn main() {
    let curve = ToneCurve::linear();
    let lut = curve.generate_lut();

    println!("Linear curve LUT (first 10 and last 10 values):");
    for i in 0..10 {
        let idx = i * 4;
        println!("  Index {}: R={}, G={}, B={}, A={}", i, lut[idx], lut[idx+1], lut[idx+2], lut[idx+3]);
    }
    println!("  ...");
    for i in 246..256 {
        let idx = i * 4;
        println!("  Index {}: R={}, G={}, B={}, A={}", i, lut[idx], lut[idx+1], lut[idx+2], lut[idx+3]);
    }

    // Test some key values
    println!("\nKey evaluations:");
    for i in &[0, 64, 128, 192, 255] {
        let x = *i as f32 / 255.0;
        let y = curve.evaluate(x);
        let expected = x;
        let diff = (y - expected).abs();
        println!("  evaluate({:.4}) = {:.4} (expected {:.4}, diff={:.6})", x, y, expected, diff);
    }

    // Check if LUT matches evaluation
    println!("\nLUT vs evaluate comparison:");
    let mut max_diff = 0.0f32;
    for i in 0..256 {
        let x = i as f32 / 255.0;
        let eval_y = curve.evaluate(x);
        let lut_y = lut[i * 4] as f32 / 255.0;
        let diff = (eval_y - lut_y).abs();
        if diff > max_diff {
            max_diff = diff;
        }
        if i < 5 || i > 250 || (i >= 127 && i <= 129) {
            println!("  i={}: x={:.4}, evaluate={:.4}, LUT={:.4}, diff={:.6}",
                     i, x, eval_y, lut_y, diff);
        }
    }
    println!("Maximum difference: {:.6}", max_diff);
}
