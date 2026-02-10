// Plasma Effect
//
// Classic demoscene plasma effect using summed sinusoids.
// Blends procedural plasma colors with the input image.
// Parameters:
//   params[0] = intensity (0-1): Blend amount with original image
//   params[1] = scale (0.5-10): Frequency of plasma pattern
//   params[2] = speed (0-10): Animation speed multiplier
//   params[3] = time: Current time for animation
//   params[4] = blend_mode (0-12): See blend_modes.wgsl for options
//   params[5] = direction (0-360): Direction of apparent motion in degrees

struct EffectParams {
    params: array<vec4<f32>, 4>,
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

const PI: f32 = 3.14159265359;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

fn plasma(uv: vec2<f32>, scale: f32, time: f32, dir_phase: f32) -> f32 {
    let p = uv * scale;
    let t = time + dir_phase;

    var v = sin(p.x + t);
    v += sin((p.y + t) * 0.5);
    v += sin((p.x + p.y + t) * 0.5);

    // Circular pattern (center orbits via sin/cos)
    let cx = p.x + 0.5 * sin(t * 0.25);
    let cy = p.y + 0.5 * cos(t * 0.5);
    v += sin(sqrt(cx * cx + cy * cy + 1.0) + t);

    return v * 0.25;  // Normalize to roughly -1 to 1
}

fn plasma_color(v: f32) -> vec3<f32> {
    // Generate psychedelic colors from plasma value
    return vec3<f32>(
        sin(v * PI) * 0.5 + 0.5,
        sin(v * PI + 2.094) * 0.5 + 0.5,  // 2*PI/3
        sin(v * PI + 4.189) * 0.5 + 0.5   // 4*PI/3
    );
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(input_texture, input_sampler, input.uv);

    let intensity = get_param(0u);
    let scale = max(0.5, get_param(1u));
    let speed = get_param(2u);
    let time = get_param(3u) * speed;
    let blend_mode = i32(get_param(4u));
    let direction = get_param(5u) * PI / 180.0;

    // Per-pixel phase offset along direction axis creates apparent motion
    // Each pixel sees the same animation but offset in time, so the pattern
    // appears to travel in the chosen direction while keeping a fixed loop period of 8*PI
    let dir_vec = vec2<f32>(cos(direction), sin(direction));
    let dir_phase = dot(input.uv * scale, dir_vec);

    // Generate plasma with directional movement
    let v = plasma(input.uv, scale, time, dir_phase);
    let plasma_rgb = plasma_color(v);

    // Apply blend mode using shared library
    let result = apply_blend(original.rgb, plasma_rgb, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
