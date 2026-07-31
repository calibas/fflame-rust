// Worley Noise Effect
//
// Creates cellular/voronoi patterns with animation.
// Parameters:
//   params[0] = intensity (0-1): Effect strength
//   params[1] = scale (1-20): Cell density
//   params[2] = time: Animation time
//   params[3] = mode: 0=Cells, 1=Edges, 2=Organic, 3=Crystal
//   params[4] = color_mode: 0=Grayscale, 1=Rainbow, 2=Original
//   params[5] = blend_mode (0-12): See blend_modes.wgsl for options
//   params[6] = direction (0-360): Direction of apparent motion in degrees

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

// Hash function for random cell points
fn hash2(p: vec2<f32>) -> vec2<f32> {
    var n = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(n) * 43758.5453);
}

// Worley noise - returns (F1, F2) distances
fn worley(p: vec2<f32>, time: f32) -> vec2<f32> {
    let cell = floor(p);
    let frac = fract(p);

    var d1 = 8.0;  // First closest distance
    var d2 = 8.0;  // Second closest distance

    // Check 3x3 neighborhood
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            let cell_pos = cell + neighbor;

            // Random point within each cell (animated)
            var point = hash2(cell_pos);
            point = 0.5 + 0.5 * sin(time * 0.5 + 6.2831 * point);

            let diff = neighbor + point - frac;
            let dist = length(diff);

            // Track two closest distances
            if (dist < d1) {
                d2 = d1;
                d1 = dist;
            } else if (dist < d2) {
                d2 = dist;
            }
        }
    }

    return vec2<f32>(d1, d2);
}

fn hsv_to_rgb_local(hsv: vec3<f32>) -> vec3<f32> {
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
    let scale = max(1.0, get_param(1u));
    let time = get_param(2u);
    let mode = i32(get_param(3u));
    let color_mode = i32(get_param(4u));
    let blend_mode = i32(get_param(5u));
    let direction = get_param(6u) * PI / 180.0;

    let uv = input.uv;
    let original = textureSample(input_texture, input_sampler, uv);

    // Compute directional time offset
    let time_offset = vec2<f32>(cos(direction), sin(direction)) * time * 0.1;
    let animated_uv = uv + time_offset;

    // Calculate Worley noise with directional movement
    let w = worley(animated_uv * scale, time);
    let f1 = w.x;  // Distance to closest point
    let f2 = w.y;  // Distance to second closest

    // Different pattern modes
    var pattern: f32;
    if (mode == 1) {
        // Edges: Highlight cell boundaries
        pattern = f2 - f1;
        pattern = smoothstep(0.0, 0.15, pattern);
    } else if (mode == 2) {
        // Organic: Smooth blobs
        pattern = 1.0 - smoothstep(0.0, 0.5, f1);
    } else if (mode == 3) {
        // Crystal: Sharp geometric cells
        pattern = f1 * f2 * 2.0;
        pattern = clamp(pattern, 0.0, 1.0);
    } else {
        // Cells (default): Basic distance field
        pattern = f1;
    }

    // Generate effect color based on color_mode
    var effect_color: vec3<f32>;
    if (color_mode == 1) {
        // Rainbow: Use distance for hue
        effect_color = hsv_to_rgb_local(vec3<f32>(
            fract(f1 + time * 0.1),
            0.8,
            pattern
        ));
    } else if (color_mode == 2) {
        // Original: Modulate original color by pattern
        effect_color = original.rgb * pattern;
    } else {
        // Grayscale (default)
        effect_color = vec3<f32>(pattern);
    }

    // Apply blend mode
    let result = apply_blend(original.rgb, effect_color, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
