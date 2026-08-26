// Distance Field (jump flooding) Effect
//
// The flame->distance-field bridge (escape-time plan §7.3): treat the
// rendered attractor as a seed mask, jump-flood it into an exterior
// distance field in O(log n) fullscreen passes, then shade the field.
// Every DE-style look (glow, contour bands, nearest-color fill)
// applies to ARBITRARY flames — no invertibility required.
//
// Three entry points over one shared vertex shader; the chain runner
// special-cases this effect into the multi-pass pipeline:
//   fs_seed      image        -> coord texture (rg = own uv | -1 sentinel)
//   fs_flood     coord ping   -> coord pong, step halves each pass (JFA)
//   fs_composite image+coords -> final color
//
// The coordinate textures are rg32float (unfilterable): all reads go
// through textureLoad, which is also the WASM-safe idiom.
//
// Parameters:
//   params[0] = threshold (0-1): luminance above this seeds the field
//   params[1] = spread (0.01-1): field range as a fraction of max dim
//   params[2] = mode (0-2): 0 = glow, 1 = contour bands, 2 = nearest fill
//   params[3] = intensity (0-2): effect strength over the base image
//   params[4] = band_count (1-64): contour bands across the range (mode 1)
//   (flood passes reuse the slot layout: params[0] = step size in px)

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
// The coordinate field (absent in the seed pass's bind group is fine:
// the layout always carries it; the seed pass binds the pong texture
// it is about to overwrite).
@group(0) @binding(3) var coord_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

fn luminance_df(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ---- pass 1: seed --------------------------------------------------
// A pixel above the luminance threshold seeds the field with its own
// pixel coordinates; everything else carries the (-1, -1) sentinel.

@fragment
fn fs_seed(input: VertexOutput) -> @location(0) vec4<f32> {
    let px = vec2<i32>(input.position.xy);
    let c = textureLoad(input_texture, px, 0);
    let threshold = get_param(0u);
    if (luminance_df(c.rgb) > threshold) {
        return vec4<f32>(input.position.xy, 0.0, 1.0);
    }
    return vec4<f32>(-1.0, -1.0, 0.0, 1.0);
}

// ---- pass 2..n: jump flood ----------------------------------------
// Standard JFA gather: examine the 3x3 neighborhood at the current
// step distance; keep the recorded seed nearest to this pixel.

@fragment
fn fs_flood(input: VertexOutput) -> @location(0) vec4<f32> {
    let step_px = i32(get_param(0u));
    let px = vec2<i32>(input.position.xy);
    let dims = vec2<i32>(i32(effect_params.width), i32(effect_params.height));
    var best = vec2<f32>(-1.0, -1.0);
    var best_d = 1.0e30;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let q = px + vec2<i32>(dx, dy) * step_px;
            if (q.x < 0 || q.y < 0 || q.x >= dims.x || q.y >= dims.y) {
                continue;
            }
            let seed = textureLoad(coord_texture, q, 0).xy;
            if (seed.x < 0.0) {
                continue;
            }
            let d = distance(seed, input.position.xy);
            if (d < best_d) {
                best_d = d;
                best = seed;
            }
        }
    }
    return vec4<f32>(best, 0.0, 1.0);
}

// ---- pass n+1: composite ------------------------------------------

@fragment
fn fs_composite(input: VertexOutput) -> @location(0) vec4<f32> {
    let px = vec2<i32>(input.position.xy);
    let base = textureLoad(input_texture, px, 0);
    let seed = textureLoad(coord_texture, px, 0).xy;
    if (seed.x < 0.0) {
        // No seed reached this pixel (empty image): leave it alone.
        return base;
    }
    let spread = max(get_param(1u), 0.01);
    let mode = i32(get_param(2u));
    let intensity = get_param(3u);
    let bands = max(get_param(4u), 1.0);
    let max_dim = f32(max(effect_params.width, effect_params.height));
    let range_px = spread * max_dim;
    let d = distance(seed, input.position.xy);
    let t = clamp(d / range_px, 0.0, 1.0);
    // The color the field propagates: the image at the nearest seed.
    let seed_color = textureLoad(input_texture, vec2<i32>(seed), 0).rgb;

    var out = base.rgb;
    if (mode == 0) {
        // Glow: the seed's color, exponential falloff with distance.
        let glow = exp(-4.0 * t) * f32(d > 0.5);
        out = base.rgb + seed_color * glow * intensity;
    } else if (mode == 1) {
        // Contour bands: iso-distance rings carrying the seed color.
        let phase = fract(t * bands);
        let ring = smoothstep(0.0, 0.15, phase) * (1.0 - smoothstep(0.35, 0.5, phase));
        let fade = 1.0 - t;
        out = base.rgb + seed_color * ring * fade * intensity * f32(d > 0.5);
    } else {
        // Nearest fill: solidify — every pixel takes its nearest
        // seed's color, faded by distance (a Voronoi-style flat).
        let fill = mix(base.rgb, seed_color, clamp(intensity, 0.0, 1.0) * (1.0 - t));
        out = select(fill, base.rgb, d <= 0.5);
    }
    return vec4<f32>(out, base.a);
}
