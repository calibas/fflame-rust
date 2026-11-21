// Hardcoded color scale (was `params.histogram_color_scale`, formerly
// a user-tunable slider — removed because the value cancels in the
// color-recovery math `(scale × Σ color) / (scale × N) = Σ color / N`,
// and u32 overflow is unreachable under any plausible iteration count).
// Must match the const in `core/main_template.wgsl`.
const HISTOGRAM_COLOR_SCALE: f32 = 100.0;

// Accumulation shader — adaptive blend (Phase 8c follow-up). The
// accumulator stores:
//   - rgb: density-weighted running mean color, in [0,1]
//   - a:   raw iteration count for this pixel (sum of hits, cumulative)
//
// Density is always cumulative (matches `sample_density = total_iters
// / pixel_count` in the tonemap). Color update uses an adaptive blend
// rate:
//
//     effective_blend = max(new_density / total_density, blend_factor)
//
// `new_density / total_density` is the cumulative-mean contribution
// rate (1.0 at first batch, → 0 as samples grow). `blend_factor` is
// a user-controlled floor.
//
// `blend_factor = 0`  → effective = cumulative rate. Pure
//                       density-weighted running mean — matches the
//                       reference Fractal Flame algorithm.
// `blend_factor > 0`  → effective floors at this rate once cumulative
//                       drops below it (a recency bias). The colour
//                       only: the density is cumulative in every mode.
//
// Precision over long renders (docs/projects/exact-accumulator.md):
//   - The DENSITY was an f32 running sum, and past 2^24 hits each
//     dispatch's addition rounds -- the same way every time, so the
//     error builds (simulated: 2.6% after 4M dispatches into a bright
//     pixel, 28% after 30M; a pixel hit once a dispatch freezes at
//     2^25). Once a render has run `exact_count` passes' worth, the
//     host binds `accum_count`: the pixel's density word summed EXACTLY
//     as a 64-bit integer (lo, hi), and `a` is derived from it each
//     pass: within two f32 steps of the count, never accumulated. Integer only, so Metal's
//     fast-math cannot reassociate it away.
//   - The COLOUR is updated as `prev + (new - prev) * alpha`, not
//     `prev * (1 - alpha) + new * alpha`: with alpha tiny, `1 - alpha`
//     rounds to a multiple of 2^-24 and the mean drifted (simulated:
//     one 8-bit level after 3M dispatches); the delta form stays
//     100-1000x closer, for the same arithmetic.
// Overwrite mode (clear prev before add) is its own mode value
// (`use_fixed_ema == 2`), the host's parameter-change reset signal.
// It used to be signalled by `blend_factor ≥ 0.99`, which a Fixed
// Blend Rate slider at 0.99-1.0 also sent: the shader then cleared
// every batch while the host kept counting the samples the buffer
// no longer held, and the picture faded as `sample_density` grew.
//
// Pre-Phase-8b this was a true EMA `prev * (1-α) + new * α` with
// nonlinearly-scaled density `prev.a + density × 0.01 × blend_factor`.
// That made the stored alpha not match "iterations per pixel," which
// leaked iters_per_thread and frame count into brightness.
//
// Uses ping-pong textures to work around read-write limitations.
// See docs/archive/projects/accumulator-unification.md.

struct AccumulateParams {
    width: u32,
    height: u32,
    // EMA blend strength when use_fixed_ema == 1; read in no other mode.
    // Any value is a blend rate, 1.0 included -- overwrite is mode 2,
    // never a blend_factor.
    blend_factor: f32,
    // Mode (was _pad0):
    //   0 = cumulative-mean — α per pixel = new_density / total_density.
    //       Matches the reference Fractal Flame algorithm. Brightness
    //       correct from the first frame.
    //   1 = fixed EMA — α = blend_factor for the colour, applied
    //       uniformly (the density stays cumulative). First frames are
    //       dim (stored value bootstraps from 0 at rate α); steady
    //       state is the running EMA mean.
    //   2 = overwrite — discard prev, store this batch alone (the
    //       host's parameter-change reset; it counts only this
    //       batch's samples in `samples_in_buffer` to match).
    use_fixed_ema: u32,
    background_r: f32, // Unused - kept for struct layout compatibility
    background_g: f32,
    background_b: f32,
    _pad1: f32,
    // Solid rendering: depth-tightening reset (see below). Only read
    // when has_depth != 0 (the histogram carries the depth region and
    // accum_depth is a real buffer).
    surface_thickness: f32,
    has_depth: u32,
    // 1 when `accum_count` is a real buffer: the density is counted
    // exactly (see the header). 0: a dummy is bound, never touched.
    exact_count: u32,
    _pad2: u32,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;
// Solid rendering: the depth (encoded, nearest-wins) that this pixel's
// ACCUMULATED data belongs to. 0 = none. 4-byte dummy when has_depth
// == 0 — never indexed then.
@group(0) @binding(4) var<storage, read_write> accum_depth: array<u32>;
// The exact density, in the histogram's own integer units (density x
// HISTOGRAM_COLOR_SCALE), as a 64-bit count: x = low word, y = high.
// A dummy when exact_count == 0 -- never indexed then.
@group(0) @binding(5) var<storage, read_write> accum_count: array<vec2<u32>>;

// Decode the splat path's inverted ordered-float depth encoding
// (0 handled by callers).
fn decode_depth(enc: u32) -> f32 {
    let ord = ~enc;
    let bits = select(~ord, ord ^ 0x80000000u, (ord & 0x80000000u) != 0u);
    return bitcast<f32>(bits);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    // Overwrite mode (slider-drag etc): discard prev so this dispatch
    // starts fresh. Lets the host signal "reset accumulation" without
    // a separate clear pass. Its own mode value, never inferred from
    // blend_factor (see the header).
    let is_overwrite_mode = params.use_fixed_ema == 2u;
    let prev_raw = textureLoad(previous_accumulation, pixel, 0);
    var prev = select(prev_raw, vec4<f32>(0.0), is_overwrite_mode);

    // Solid rendering: DEPTH-TIGHTENING RESET. Under progressive
    // sampling of a sparse front surface, a pixel can accumulate the
    // structure BEHIND it for a long time before its first
    // front-surface sample arrives and snaps the nearest depth closer.
    // The history then belongs to a surface that is now known to be
    // occluded — blending it in poisons the color for hundreds of
    // batches (and makes the shade pass's occlusion repair pop when it
    // hands the pixel back to "own data"). Discard it instead: the
    // pixel restarts cleanly from its first front-surface samples.
    if (params.has_depth != 0u && !is_overwrite_mode) {
        let pi = global_id.y * params.width + global_id.x;
        let denc = histogram[params.width * params.height * 4u + pi];
        if (denc != 0u) {
            let penc = accum_depth[pi];
            if (penc != 0u && denc > penc) {
                let d_new = decode_depth(denc);
                let d_old = decode_depth(penc);
                if (d_old - d_new > max(params.surface_thickness, 0.001) * 1.5) {
                    prev = vec4<f32>(0.0);
                }
            }
            // Track the nearest depth the accumulated data represents
            // (encoding: larger = nearer).
            accum_depth[pi] = max(denc, penc);
        }
    }

    // Read this batch's contributions to this pixel from the
    // histogram. Compute shader stores 4× u32 per pixel: scaled R, G,
    // B sums and density (where density = histogram_color_scale × N
    // and the color sums are histogram_color_scale × Σ color).
    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 4u;
    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let scaled_density = f32(histogram[base_idx + 3u]);

    // Convert scaled density to raw iteration count. The compute shader
    // multiplies by HISTOGRAM_COLOR_SCALE to keep per-iteration u32
    // values in a useful precision range; we undo it here so the stored
    // alpha is "iteration count for this pixel," matching the units
    // the tonemap's sample_density expects (iters / pixel_count).
    let color_scale = HISTOGRAM_COLOR_SCALE;
    let new_density = scaled_density / color_scale;
    var total_density = prev.a + new_density;

    // The exact count, when bound. An empty history (`prev.a == 0`:
    // overwrite, the depth reset, a clear, never hit) empties the count
    // too, so a count can never outlive its texture. A zero count
    // beside a non-zero `a` is a count made this render: seeded from
    // `a`, which was exact enough when the host made it. Written back
    // every pass, hit or not, so a reset reaches it.
    if (params.exact_count != 0u) {
        var count = accum_count[pixel_idx];
        if (prev.a == 0.0) {
            count = vec2<u32>(0u);
        } else if (count.x == 0u && count.y == 0u) {
            let seed = prev.a * color_scale;
            let hi = floor(seed / 4294967296.0);
            count = vec2<u32>(u32(seed - hi * 4294967296.0), u32(hi));
        }
        let lo = count.x + histogram[base_idx + 3u];
        count = vec2<u32>(lo, count.y + select(0u, 1u, lo < count.x));
        accum_count[pixel_idx] = count;
        total_density = (f32(count.y) * 4294967296.0 + f32(count.x)) / color_scale;
        prev.a = total_density;
    }

    // No new contribution this pixel this dispatch — keep prev. (In
    // overwrite mode, prev was already cleared above, so this stores
    // zero, which is the right "fresh start" behavior.)
    if (new_density <= 0.0) {
        textureStore(output_texture, pixel, prev);
        return;
    }

    // This batch's average color at this pixel: Σ scaled_color / scaled_density.
    // The histogram_color_scale factor in numerator and denominator
    // cancels, so this is the unscaled batch-average color in [0,1].
    let new_color = clamp(
        vec3<f32>(r_sum, g_sum, b_sum) / max(scaled_density, 1.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Pick α based on mode (see AccumulateParams::use_fixed_ema doc):
    //   0 → cumulative-mean rate (α = new_density / total_density)
    //   1 → fixed EMA at blend_factor
    //   2 → overwrite: prev was cleared, so the cumulative rate is 1
    // The delta form `prev + (new - prev) * α`, in every mode:
    // algebraically the density-weighted mean in cumulative mode, and
    // it does not round `1 - α` (see the header).
    let cumulative_alpha = new_density / total_density;
    let alpha = select(cumulative_alpha, params.blend_factor, params.use_fixed_ema == 1u);
    let blended_rgb = prev.rgb + (new_color - prev.rgb) * alpha;

    textureStore(output_texture, pixel, vec4<f32>(blended_rgb, total_density));
}
