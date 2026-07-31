// Domain Warp Effect
//
// Creates organic flowing distortion using fractal Brownian motion (FBM) noise.
// Parameters:
//   params[0] = intensity (0-0.5): Amount of UV distortion
//   params[1] = scale (0.5-10): Noise frequency
//   params[2] = octaves (1-6): Number of noise layers
//   params[3] = time: Animation time
//   params[4] = blend_mode (0-12): See blend_modes.wgsl for options
//   params[5] = direction (0-360): Direction of apparent motion in degrees

const PI: f32 = 3.14159265359;

struct EffectParams {
    params: array<vec4<f32>, 12>,
    width: u32,
    height: u32,
    _padding: vec2<f32>,
}

fn get_param(index: u32) -> f32 {
    return effect_params.params[index / 4u][index % 4u];
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect_params: EffectParams;

// INCLUDE_BLEND_MODES

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

// Simplex-like noise (2D gradient noise)
fn hash2(p: vec2<f32>) -> vec2<f32> {
    var n = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(n) * 43758.5453) * 2.0 - 1.0;
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    let u = f * f * (3.0 - 2.0 * f);

    let a = dot(hash2(i + vec2<f32>(0.0, 0.0)), f - vec2<f32>(0.0, 0.0));
    let b = dot(hash2(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let c = dot(hash2(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let d = dot(hash2(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;

    let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(pos);
        pos = rot * pos * 2.02;
        amplitude *= 0.5;
    }

    return value;
}

fn domain_warp_calc(p: vec2<f32>, time_offset: vec2<f32>, octaves: i32) -> vec2<f32> {
    let q = vec2<f32>(
        fbm(p + vec2<f32>(0.0, 0.0) + time_offset, octaves),
        fbm(p + vec2<f32>(5.2, 1.3) + time_offset * 1.2, octaves)
    );

    let r = vec2<f32>(
        fbm(p + 4.0 * q + vec2<f32>(1.7, 9.2), octaves),
        fbm(p + 4.0 * q + vec2<f32>(8.3, 2.8), octaves)
    );

    return r;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let intensity = get_param(0u);
    let scale = max(0.5, get_param(1u));
    let octaves = clamp(i32(get_param(2u)), 1, 6);
    let time = get_param(3u);
    let blend_mode = i32(get_param(4u));
    let direction = get_param(5u) * PI / 180.0;

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Compute directional time offset
    let time_offset = vec2<f32>(cos(direction), sin(direction)) * time * 0.1;

    // Apply domain warping to UV coordinates
    let warp = domain_warp_calc(input.uv * scale, time_offset, octaves);
    let warped_uv = clamp(input.uv + warp * intensity, vec2<f32>(0.0), vec2<f32>(1.0));
    let warped = textureSample(input_texture, input_sampler, warped_uv);

    // Apply blend mode between original and warped
    let result = apply_blend(original.rgb, warped.rgb, blend_mode, intensity * 2.0);

    return vec4<f32>(result, original.a);
}
