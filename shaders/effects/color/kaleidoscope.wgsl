// Kaleidoscope Effect
//
// Creates N-fold rotational symmetry by folding UV coordinates in polar space.
// Parameters:
//   params[0] = segments (2-16): Number of symmetric segments
//   params[1] = rotation (0-360): Rotation offset in degrees
//   params[2] = zoom (0.1-3.0): Zoom factor

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

fn kaleidoscope_uv(uv: vec2<f32>, segments: f32, rotation: f32, zoom: f32) -> vec2<f32> {
    // Center and apply zoom
    var centered = (uv - 0.5) / zoom;

    // Convert to polar coordinates
    let radius = length(centered);
    var angle = atan2(centered.y, centered.x) + rotation;

    // Fold into one segment
    let segment_angle = TWO_PI / segments;
    angle = angle - segment_angle * floor(angle / segment_angle);

    // Mirror within segment for seamless reflection
    if (angle > segment_angle * 0.5) {
        angle = segment_angle - angle;
    }

    // Convert back to cartesian and recenter
    return vec2<f32>(cos(angle), sin(angle)) * radius + 0.5;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let segments = max(2.0, get_param(0u));
    let rotation = get_param(1u) * PI / 180.0;  // Convert degrees to radians
    let zoom = max(0.1, get_param(2u));

    let kaleido_uv = kaleidoscope_uv(input.uv, segments, rotation, zoom);

    // Clamp UV to valid range for sampling
    let clamped_uv = clamp(kaleido_uv, vec2<f32>(0.001), vec2<f32>(0.999));

    return textureSample(input_texture, input_sampler, clamped_uv);
}
