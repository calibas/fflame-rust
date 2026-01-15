// Hue Cycle Effect
//
// Rotates the hue of all colors, optionally animated over time.
// Parameters:
//   params[0] = offset (0-360): Static hue rotation in degrees
//   params[1] = speed (-360 to 360): Rotation speed in degrees per second

struct EffectParams {
    // Parameters packed into vec4s for uniform buffer alignment
    // Access as: params[i/4][i%4] or use helper below
    params: array<vec4<f32>, 4>,
    width: u32,
    height: u32,
    time: f32,
    _padding: f32,
}

// Helper to get parameter by index
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

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate fullscreen triangle (no vertex buffer needed)
    // Triangle covers [-1, 3] x [-1, 3] in clip space
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);

    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);

    return output;
}

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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample input texture
    let color = textureSample(input_texture, input_sampler, input.uv);

    // Get parameters
    let offset = get_param(0u);     // degrees
    let speed = get_param(1u);      // degrees per second

    // Calculate total hue rotation
    let rotation = (offset + speed * effect_params.time) / 360.0;

    // Convert to HSV, rotate hue, convert back
    var hsv = rgb_to_hsv(color.rgb);
    hsv.x = fract(hsv.x + rotation);
    let rgb = hsv_to_rgb(hsv);

    return vec4<f32>(rgb, color.a);
}
