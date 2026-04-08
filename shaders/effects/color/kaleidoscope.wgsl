// Kaleidoscope Effect
//
// Creates N-fold rotational symmetry by folding UV coordinates in polar space.
// Parameters:
//   params[0] = segments (2-16): Number of symmetric segments
//   params[1] = rotation (0-360): Rotation offset in degrees (controls which slice is mirrored)
//   params[2] = zoom (0.1-3.0): Zoom factor
//   params[3] = intensity (0-1): Blend with original
//   params[4] = blend_mode (0-12): See blend_modes.wgsl for options
//   params[5] = square_mode (0-1): 0 = Screen ratio (stretched), 1 = Square (1:1 perfect symmetry)
//   params[6] = edge_offset (0-2000): Shift sampled region toward center by N pixels
//                                     to avoid sampling offscreen edges. Direction is
//                                     determined by the source angle (rotation).

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
const TWO_PI: f32 = 6.28318530718;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

fn kaleidoscope_uv(uv: vec2<f32>, segments: f32, source_angle: f32, zoom: f32, aspect: f32, square_mode: bool) -> vec2<f32> {
    var centered = (uv - 0.5) / zoom;

    // In square mode, correct for aspect ratio so circles appear circular
    // This makes the kaleidoscope operate in a virtual square space
    if (square_mode) {
        centered.x = centered.x * aspect;
    }

    let radius = length(centered);
    var angle = atan2(centered.y, centered.x);

    // Fold angle into one segment (creates the kaleidoscope pattern)
    let segment_angle = TWO_PI / segments;
    angle = angle - segment_angle * floor(angle / segment_angle);

    // Mirror within segment
    if (angle > segment_angle * 0.5) {
        angle = segment_angle - angle;
    }

    // Apply source angle AFTER folding to rotate which part of the image is sampled
    // This controls the source slice, not the output pattern orientation
    angle = angle + source_angle;

    var result = vec2<f32>(cos(angle), sin(angle)) * radius;

    // Convert back from square space to screen space
    if (square_mode) {
        result.x = result.x / aspect;
    }

    return result + 0.5;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let segments = max(2.0, get_param(0u));
    let rotation = get_param(1u) * PI / 180.0;
    let zoom = max(0.1, get_param(2u));
    let intensity = get_param(3u);
    let blend_mode = i32(get_param(4u));
    let square_mode = get_param(5u) > 0.5;
    let edge_offset = get_param(6u);

    // Calculate aspect ratio for square mode correction
    let aspect = f32(effect_params.width) / f32(effect_params.height);

    let original = textureSample(input_texture, input_sampler, input.uv);

    let kaleido_uv = kaleidoscope_uv(input.uv, segments, rotation, zoom, aspect, square_mode);

    // Edge offset: shift sampled UV toward image center to avoid offscreen sampling.
    // Direction points from the source slice (at angle `rotation`) toward the center,
    // which is the negative of the rotation's unit vector.
    let dir = -vec2<f32>(cos(rotation), sin(rotation));
    let pixel_to_uv = vec2<f32>(1.0 / f32(effect_params.width), 1.0 / f32(effect_params.height));
    let offset_shift = dir * edge_offset * pixel_to_uv;
    let shifted_uv = kaleido_uv + offset_shift;

    let clamped_uv = clamp(shifted_uv, vec2<f32>(0.001), vec2<f32>(0.999));
    let kaleido = textureSample(input_texture, input_sampler, clamped_uv);

    // Apply blend mode between original and kaleidoscope
    let result = apply_blend(original.rgb, kaleido.rgb, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
