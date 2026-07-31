// Hue Cycle Effect
//
// Rotates the hue of all colors by a fixed amount.
// Animation system can keyframe the offset parameter for animated rotation.
// Parameters:
//   params[0] = offset (0-360): Hue rotation in degrees
//   params[1] = intensity (0-1): Blend with original
//   params[2] = blend_mode (0-12): See blend_modes.wgsl for options

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

// Convert RGB to HSV
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let c_max = max(rgb.r, max(rgb.g, rgb.b));
    let c_min = min(rgb.r, min(rgb.g, rgb.b));
    let delta = c_max - c_min;

    var h: f32 = 0.0;
    var s: f32 = 0.0;
    let v: f32 = c_max;

    if (delta > 0.0001) {
        s = delta / c_max;

        if (c_max == rgb.r) {
            h = (rgb.g - rgb.b) / delta;
            if (rgb.g < rgb.b) {
                h += 6.0;
            }
        } else if (c_max == rgb.g) {
            h = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            h = 4.0 + (rgb.r - rgb.g) / delta;
        }
        h /= 6.0;
    }

    return vec3<f32>(h, s, v);
}

// Convert HSV to RGB
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let i_mod = i32(i) % 6;

    if (i_mod == 0) {
        return vec3<f32>(v, t, p);
    } else if (i_mod == 1) {
        return vec3<f32>(q, v, p);
    } else if (i_mod == 2) {
        return vec3<f32>(p, v, t);
    } else if (i_mod == 3) {
        return vec3<f32>(p, q, v);
    } else if (i_mod == 4) {
        return vec3<f32>(t, p, v);
    } else {
        return vec3<f32>(v, p, q);
    }
}

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
    let offset = get_param(0u);
    let intensity = get_param(1u);
    let blend_mode = i32(get_param(2u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Convert degrees to 0-1 range for hue rotation
    let rotation = offset / 360.0;

    // Convert to HSV, rotate hue, convert back
    var hsv = rgb_to_hsv(original.rgb);
    hsv.x = fract(hsv.x + rotation);
    let rotated = hsv_to_rgb(hsv);

    // Apply blend mode
    let result = apply_blend(original.rgb, rotated, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
