// Julia/Mandelbrot Overlay Effect
//
// Renders Mandelbrot or Julia set fractals as an overlay on the image.
// Parameters:
//   params[0] = mode: 0 = Mandelbrot, 1 = Julia
//   params[1] = julia_cx (-2 to 2): Real part of Julia c parameter
//   params[2] = julia_cy (-2 to 2): Imaginary part of Julia c parameter
//   params[3] = max_iter (50-500): Maximum iterations
//   params[4] = zoom (0.1-10): Zoom level (independent of flame)
//   params[5] = center_x (-3 to 3): Pan X
//   params[6] = center_y (-3 to 3): Pan Y
//   params[7] = color_mode: 0 = Escape time, 1 = Smooth, 2 = Original mask
//   params[8] = intensity (0-1): Blend amount
//   params[9] = blend_mode (0-12): See blend_modes.wgsl for options

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

// HSV to RGB for coloring
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

// Iterate z = z^2 + c
// Returns: (iterations, final |z|^2) for smooth coloring
fn iterate_mandelbrot(c: vec2<f32>, max_iter: i32) -> vec2<f32> {
    var z = vec2<f32>(0.0, 0.0);
    var i = 0;

    for (; i < max_iter; i++) {
        let z2 = z * z;
        if (z2.x + z2.y > 4.0) {
            break;
        }
        z = vec2<f32>(z2.x - z2.y + c.x, 2.0 * z.x * z.y + c.y);
    }

    return vec2<f32>(f32(i), z.x * z.x + z.y * z.y);
}

// Julia set: z starts at pixel, c is fixed
fn iterate_julia(z_start: vec2<f32>, c: vec2<f32>, max_iter: i32) -> vec2<f32> {
    var z = z_start;
    var i = 0;

    for (; i < max_iter; i++) {
        let z2 = z * z;
        if (z2.x + z2.y > 4.0) {
            break;
        }
        z = vec2<f32>(z2.x - z2.y + c.x, 2.0 * z.x * z.y + c.y);
    }

    return vec2<f32>(f32(i), z.x * z.x + z.y * z.y);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let mode = i32(get_param(0u));
    let julia_cx = get_param(1u);
    let julia_cy = get_param(2u);
    let max_iter = max(10, i32(get_param(3u)));
    let zoom = max(0.1, get_param(4u));
    let center_x = get_param(5u);
    let center_y = get_param(6u);
    let color_mode = i32(get_param(7u));
    let intensity = get_param(8u);
    let blend_mode = i32(get_param(9u));

    let original = textureSample(input_texture, input_sampler, input.uv);

    // Convert UV to fractal coordinates
    // Correct for aspect ratio
    let aspect = f32(effect_params.width) / f32(effect_params.height);
    var p = (input.uv - 0.5) * 2.0;  // -1 to 1
    p.x *= aspect;
    p = p / zoom + vec2<f32>(center_x, center_y);

    // Iterate based on mode
    var result: vec2<f32>;
    if (mode == 1) {
        // Julia mode
        result = iterate_julia(p, vec2<f32>(julia_cx, julia_cy), max_iter);
    } else {
        // Mandelbrot mode
        result = iterate_mandelbrot(p, max_iter);
    }

    let iter = result.x;
    let z_mag_sq = result.y;
    let max_iter_f = f32(max_iter);

    // Generate color based on color_mode
    var effect_color: vec3<f32>;

    if (iter >= max_iter_f) {
        // Inside the set - black
        effect_color = vec3<f32>(0.0);
    } else if (color_mode == 1) {
        // Smooth coloring using continuous potential
        let log_zn = log(z_mag_sq) / 2.0;
        let nu = log(log_zn / log(2.0)) / log(2.0);
        let smooth_iter = iter + 1.0 - nu;
        let t = smooth_iter / max_iter_f;
        effect_color = hsv_to_rgb(vec3<f32>(fract(t * 5.0), 0.8, 1.0));
    } else if (color_mode == 2) {
        // Use original image colors, masked by set membership
        let t = iter / max_iter_f;
        effect_color = original.rgb * t;
    } else {
        // Basic escape time coloring
        let t = iter / max_iter_f;
        effect_color = hsv_to_rgb(vec3<f32>(fract(t * 3.0), 0.7, t));
    }

    // Apply blend mode
    let blended = apply_blend(original.rgb, effect_color, blend_mode, intensity);

    return vec4<f32>(blended, original.a);
}
