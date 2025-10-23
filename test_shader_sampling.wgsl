// Test to understand texture sampling behavior

@group(0) @binding(0) var lut_texture: texture_1d<f32>;
@group(0) @binding(1) var lut_sampler: sampler;

@fragment
fn main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // Sample at a few test coordinates
    let samples = array<f32, 5>(0.0, 0.25, 0.5, 0.75, 1.0);

    var result = vec4<f32>(0.0);
    for (var i = 0u; i < 5u; i++) {
        let coord = samples[i];
        let sampled = textureSample(lut_texture, lut_sampler, coord).r;
        // Accumulate to see if they're all identity
        result.r += abs(sampled - coord);
    }

    // If result.r is 0, all samples were identity
    // Color it green if good, red if bad
    if (result.r < 0.01) {
        return vec4<f32>(0.0, 1.0, 0.0, 1.0); // Green = working
    } else {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red = broken
    }
}
