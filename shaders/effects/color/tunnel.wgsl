// Tunnel Effect
//
// Creates an infinite tunnel effect by mapping texture to polar coordinates.
// Parameters:
//   params[0] = speed (0-5): Forward movement speed
//   params[1] = rotation_speed (0-5): Tunnel rotation speed
//   params[2] = distortion (0-1): Amount of radial distortion
//   params[3] = time: Current time for animation
//   params[4] = intensity (0-1): Blend with original
//   params[5] = blend_mode (0-12): See blend_modes.wgsl for options

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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let speed = get_param(0u);
    let rotation_speed = get_param(1u);
    let distortion = get_param(2u);
    let time = get_param(3u);
    let intensity = get_param(4u);
    let blend_mode = i32(get_param(5u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Center coordinates and correct for aspect ratio
    let aspect = f32(effect_params.width) / f32(effect_params.height);
    var p = input.uv - 0.5;
    p.x *= aspect;

    // Convert to polar coordinates
    let angle = atan2(p.y, p.x) / PI;
    let radius = length(p);

    // Avoid division by zero at center
    if (radius < 0.001) {
        return original;
    }

    // Apply distortion
    let distorted_radius = radius + distortion * sin(angle * PI * 4.0 + time) * 0.1;

    // Create tunnel UV coordinates
    var tunnel_uv = vec2<f32>(
        angle * 0.5 + 0.5 + time * rotation_speed * 0.1,
        1.0 / distorted_radius + time * speed
    );
    tunnel_uv = fract(tunnel_uv);

    // Sample with tunnel coordinates
    let tunnel_color = textureSample(input_texture, input_sampler, tunnel_uv);

    // Darken toward center for depth effect
    let depth_fade = smoothstep(0.0, 0.5, radius);
    let tunnel_rgb = tunnel_color.rgb * depth_fade;

    // Apply blend mode
    let result = apply_blend(original.rgb, tunnel_rgb, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
