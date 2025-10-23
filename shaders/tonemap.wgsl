// Tonemap shader for displaying the accumulation buffer
// Applies logarithmic tone mapping and gamma correction

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct TonemapParams {
    exposure: f32,
    gamma: f32,
    density_scale: f32,
    _pad0: f32,
    background_color: vec3<f32>,
    _pad1: f32,
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var accumulation_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap_params: TonemapParams;

// Vertex shader for fullscreen quad
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate fullscreen triangle
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);

    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);

    return output;
}

// Fragment shader with tone mapping
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample accumulation buffer
    let accum = textureSample(accumulation_texture, accumulation_sampler, input.uv);

    // Extract RGB and alpha (density)
    var color = accum.rgb;
    let density = accum.a;

    // Apply exposure to color directly (not multiplied by density)
    // The density is already encoded in the accumulated RGB values
    color *= tonemap_params.exposure;

    // Logarithmic tone mapping for high dynamic range
    // This compresses the bright areas while preserving detail
    color = log(color + 1.0) / log(10.0);

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Map density to alpha using density_scale
    // The density represents how many samples hit this pixel
    // density_scale controls transparency: higher = more opaque
    let alpha = clamp(density * tonemap_params.density_scale, 0.0, 1.0);

    // Check if background is black (transparent export mode)
    let bg_sum = tonemap_params.background_color.r + tonemap_params.background_color.g + tonemap_params.background_color.b;
    let is_transparent_mode = bg_sum < 0.001;

    // In transparent mode, output fractal color with alpha
    // In normal mode, blend with background and output opaque
    let final_color = select(
        mix(tonemap_params.background_color, color, alpha),  // Normal mode: blend with background
        color,                                                 // Transparent mode: just the color
        is_transparent_mode
    );
    let output_alpha = select(1.0, alpha, is_transparent_mode);

    return vec4<f32>(final_color, output_alpha);
}
