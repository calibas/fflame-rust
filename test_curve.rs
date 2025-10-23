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
        println!("  evaluate({:.4}) = {:.4} (expected {:.4})", x, y, x);
    }
}
