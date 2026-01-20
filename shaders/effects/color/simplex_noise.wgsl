// Simplex Noise Overlay Effect
//
// Creates psychedelic noise overlays and distortions using simplex-like noise.
// Parameters:
//   params[0] = intensity (0-1): Blend amount with original image
//   params[1] = scale (0.5-20): Noise frequency
//   params[2] = octaves (1-6): Number of noise layers (FBM)
//   params[3] = time: Animation time
//   params[4] = mode: 0=Color, 1=Distort, 2=Mask
//   params[5] = blend_mode (0-12): See blend_modes.wgsl for options
//   params[6] = direction (0-360): Direction of apparent motion in degrees

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

// Simplex-like 2D noise using gradient interpolation
fn hash2(p: vec2<f32>) -> vec2<f32> {
    var n = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(n) * 43758.5453) * 2.0 - 1.0;
}

fn simplex_noise(p: vec2<f32>) -> f32 {
    let F2 = 0.5 * (sqrt(3.0) - 1.0);
    let G2 = (3.0 - sqrt(3.0)) / 6.0;

    let s = (p.x + p.y) * F2;
    let i = floor(p + s);
    let t = (i.x + i.y) * G2;
    let x0 = p - (i - t);

    var i1: vec2<f32>;
    if (x0.x > x0.y) {
        i1 = vec2<f32>(1.0, 0.0);
    } else {
        i1 = vec2<f32>(0.0, 1.0);
    }

    let x1 = x0 - i1 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;

    var n0 = 0.0;
    var n1 = 0.0;
    var n2 = 0.0;

    var t0 = 0.5 - dot(x0, x0);
    if (t0 >= 0.0) {
        t0 *= t0;
        n0 = t0 * t0 * dot(hash2(i), x0);
    }

    var t1 = 0.5 - dot(x1, x1);
    if (t1 >= 0.0) {
        t1 *= t1;
        n1 = t1 * t1 * dot(hash2(i + i1), x1);
    }

    var t2 = 0.5 - dot(x2, x2);
    if (t2 >= 0.0) {
        t2 *= t2;
        n2 = t2 * t2 * dot(hash2(i + 1.0), x2);
    }

    return 70.0 * (n0 + n1 + n2);
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;

    let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);

    for (var i = 0; i < octaves; i++) {
        value += amplitude * simplex_noise(pos);
        pos = rot * pos * 2.02;
        amplitude *= 0.5;
    }

    return value;
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x;
    let s = hsv.y;
    let v = hsv.z;

    let c = v * s;
    let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
    let m = v - c;

    var rgb: vec3<f32>;
    let h6 = h * 6.0;
    if (h6 < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (h6 < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (h6 < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (h6 < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (h6 < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    else { rgb = vec3<f32>(c, 0.0, x); }

    return rgb + m;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let intensity = get_param(0u);
    let scale = max(0.5, get_param(1u));
    let octaves = clamp(i32(get_param(2u)), 1, 6);
    let time = get_param(3u);
    let mode = i32(get_param(4u));
    let blend_mode = i32(get_param(5u));
    let direction = get_param(6u) * PI / 180.0;

    let uv = input.uv;
    let original = textureSample(input_texture, input_sampler, uv);

    // Generate animated noise with directional movement
    let time_offset = vec2<f32>(cos(direction), sin(direction)) * time * 0.1;
    let noise_pos = uv * scale + time_offset;
    let noise = fbm(noise_pos, octaves);
    let noise2 = fbm(noise_pos + vec2<f32>(5.2, 1.3), octaves);

    var effect_color: vec3<f32>;

    if (mode == 1) {
        // Distort mode: Use noise to warp UVs, return warped image
        let warp = vec2<f32>(noise, noise2) * intensity * 0.1;
        let warped_uv = clamp(uv + warp, vec2<f32>(0.0), vec2<f32>(1.0));
        // For distort, we return directly (blend doesn't apply well)
        return textureSample(input_texture, input_sampler, warped_uv);
    } else if (mode == 2) {
        // Mask mode: Use noise as brightness mask
        let mask = smoothstep(-0.5, 0.5, noise);
        effect_color = original.rgb * mask;
    } else {
        // Color mode (default): Generate psychedelic noise colors
        effect_color = hsv_to_rgb(vec3<f32>(
            fract(noise * 0.5 + 0.5 + time * 0.1),
            0.8,
            noise2 * 0.5 + 0.5
        ));
    }

    // Apply blend mode
    let result = apply_blend(original.rgb, effect_color, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
