// Accumulate pass — consumes the sample stream produced by the unified
// iteration shader (with OUTPUT_HISTOGRAM_DIRECT=false) and atomic-adds
// each sample into the right slot of a histogram buffer.
//
// The histogram is laid out flat row-major (same as the direct-histogram
// output the interactive renderer writes to, see main_template.wgsl):
// 4× u32 per pixel — R, G, B, density. Per-pixel base index =
// (pixel_y - bound_y_start) × bound_width + (pixel_x - bound_x_start),
// with `bound_*` describing the histogram region this dispatch is bound
// to.
//
// The bound region lets one dispatch act on a sub-tile of the full
// histogram, so a multi-tile render can pipeline iterate→accumulate
// per tile (Phase 5) or per tile-batch. Samples landing outside the
// bound region get dropped — bin them with an outer dispatch loop, not
// inside the shader.
//
// One thread per sample (workgroup_size 64 along x, dispatch sized to
// the sample-count read back from the counter). See
// docs/projects/unified-render-pipeline.md.

struct Sample {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    // Density weight (depth-density compensation; 1.0 = neutral).
    // Scales all four histogram adds below.
    weight: f32,
    // Solid rendering: camera-space depth (positive = in front of the
    // camera). Only meaningful when the iteration shader was built with
    // SOLID; 0 otherwise.
    depth: f32,
    _pad3: f32,
}

struct AccumulateParams {
    // X/Y origin of the histogram region the dispatch is bound to,
    // in full-image pixel coords. Samples landing outside
    // [bound_x, bound_x + bound_width) × [bound_y, bound_y + bound_height)
    // are dropped.
    bound_x: u32,
    bound_y: u32,
    bound_width: u32,
    bound_height: u32,

    // Sample count actually written by the iteration shader (read from
    // the atomic sample counter on host before dispatch). Threads with
    // global_id.x >= sample_count exit early.
    sample_count: u32,

    // Color scale applied to clamp(r,g,b) before atomic-adding into the
    // histogram, mirroring the direct path's `histogram_color_scale`
    // multiplier so the accumulation precision matches across modes.
    color_scale: f32,

    // Solid rendering (0 = off). When active, the bound histogram region
    // carries one extra u32 per pixel at offset bound_width*bound_height*4
    // — the nearest-depth region (inverted ordered-float encoding, 0 =
    // "no sample"; identical to the interactive direct path, see
    // main_template.wgsl SOLID). Samples deeper than nearest +
    // surface_thickness get weight *= (1 - solid_strength).
    solid_strength: f32,
    surface_thickness: f32,
    // 1 = depth-priming dispatch: record depth only, plot nothing (the
    // host sets this for the export's first batch, mirroring the
    // interactive renderer's post-reset priming).
    depth_prime: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    // ── Light-space shadow maps (Stage 2, export path) ──
    // shadow_count = 0 disables the splat. Sample coords are FULL-image
    // pixels; the world position is reconstructed from (pixel, depth)
    // with the same inversion as shade.wgsl, so the full view transform
    // rides along. Layout mirrors src/export/accumulate.rs exactly.
    full_width: u32,
    full_height: u32,
    shadow_count: u32,
    _pad3: u32,
    zoom: f32,
    rotation: f32,
    pan_x: f32,
    pan_y: f32,
    persp: f32,
    _pad4: f32,
    _pad5: f32,
    _pad6: f32,
    // Effective world→camera rotation rows (world = Σ cam_i · row_i +
    // cam_pos; see shade_pass.rs::effective_camera_rows).
    cam_row0: vec4<f32>,
    cam_row1: vec4<f32>,
    cam_row2: vec4<f32>,
    cam_pos: vec4<f32>,
    // xyz = map center, w = bounding radius (must match the lookup).
    shadow_fit: vec4<f32>,
    // xyz = world direction TO each light, w = enabled.
    shadow_dirs: array<vec4<f32>, 4>,
}

@group(0) @binding(0) var<storage, read> samples: array<Sample>;
@group(0) @binding(1) var<uniform> ap: AccumulateParams;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;
// Light-space shadow maps: 4 × 1024² words (ordered-float atomicMax of
// dot(p − center, L) per texel). 16-byte dummy when shadow_count == 0.
@group(0) @binding(3) var<storage, read_write> shadow_maps: array<atomic<u32>>;

// Camera-space position for a FULL-image pixel at camera depth d —
// identical inversion to shade.wgsl::reconstruct.
fn sm_reconstruct(px: f32, py: f32, d: f32) -> vec3<f32> {
    let scale = f32(min(ap.full_width, ap.full_height)) * 0.25;
    let center = vec2<f32>(f32(ap.full_width), f32(ap.full_height)) * 0.5;
    var t = (vec2<f32>(px + 0.5, py + 0.5) - center) / scale;
    t = t / max(ap.zoom, 1e-6);
    let c = cos(-ap.rotation);
    let sn = sin(-ap.rotation);
    t = vec2<f32>(t.x * c - t.y * sn, t.x * sn + t.y * c);
    t = t + vec2<f32>(ap.pan_x, ap.pan_y);
    let zr = 1.0 + ap.persp * d;
    return vec3<f32>(t * zr, -d);
}

// Splat one sample's shadow contribution toward every enabled light.
// The basis derivation must match shadow_map_splat (core/header.wgsl)
// and shadow_map_factor (shade.wgsl) exactly.
fn sm_splat(wpos: vec3<f32>) {
    let rel = wpos - ap.shadow_fit.xyz;
    for (var li = 0u; li < ap.shadow_count; li = li + 1u) {
        let ld = ap.shadow_dirs[li];
        if (ld.w < 0.5) {
            continue;
        }
        let l_dir = ld.xyz;
        var bu = cross(l_dir, vec3<f32>(0.0, 0.0, 1.0));
        if (dot(bu, bu) < 1e-6) {
            bu = cross(l_dir, vec3<f32>(1.0, 0.0, 0.0));
        }
        bu = normalize(bu);
        let bv = cross(l_dir, bu);
        let r = max(ap.shadow_fit.w, 1e-6);
        let mu = dot(rel, bu) / r * 0.5 + 0.5;
        let mv = dot(rel, bv) / r * 0.5 + 0.5;
        if (mu < 0.0 || mu >= 1.0 || mv < 0.0 || mv >= 1.0) {
            continue;
        }
        let res = 1024u;
        let tx = min(u32(mu * f32(res)), res - 1u);
        let ty = min(u32(mv * f32(res)), res - 1u);
        let dl = dot(rel, l_dir);
        let db = bitcast<u32>(dl);
        let de = select(db | 0x80000000u, ~db, (db & 0x80000000u) != 0u);
        atomicMax(&shadow_maps[li * (res * res) + ty * res + tx], de);
    }
}

@compute @workgroup_size(64, 1, 1)
fn accumulate_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= ap.sample_count) {
        return;
    }
    let s = samples[idx];

    // Reject samples outside the bound histogram region. Coords come
    // from the iteration shader as f32 pixel positions (it converts
    // from world space and stores `f32(pixel.x), f32(pixel.y)`); cast
    // to i32 so out-of-range values fail the bounds check rather than
    // wrapping when rounded into u32.
    let px = i32(s.x);
    let py = i32(s.y);
    let bx0 = i32(ap.bound_x);
    let by0 = i32(ap.bound_y);
    let bx1 = bx0 + i32(ap.bound_width);
    let by1 = by0 + i32(ap.bound_height);
    if (px < bx0 || px >= bx1 || py < by0 || py >= by1) {
        return;
    }

    // Light-space shadow splat (Stage 2): tiles partition the image, so
    // running this after the bounds check records each sample exactly
    // once. Runs during the priming dispatch too (depth-only data).
    if (ap.shadow_count > 0u) {
        let cpos = sm_reconstruct(s.x, s.y, s.depth);
        sm_splat(ap.cam_pos.xyz
            + cpos.x * ap.cam_row0.xyz
            + cpos.y * ap.cam_row1.xyz
            + cpos.z * ap.cam_row2.xyz);
    }

    let local_x = u32(px - bx0);
    let local_y = u32(py - by0);
    let pixel_idx = local_y * ap.bound_width + local_x;
    let base = pixel_idx * 4u;

    // Solid rendering: nearest-depth test against this tile's depth
    // region. Runtime-gated (this pass is export-only, not the hot
    // interactive loop) — with solid off the region doesn't exist and
    // this block never touches it. Same encoding + race semantics as
    // the interactive direct path (main_template.wgsl SOLID).
    var solid_weight = 1.0;
    if (ap.solid_strength > 0.0) {
        let sd_bits = bitcast<u32>(s.depth);
        let sd_ord = select(sd_bits | 0x80000000u, ~sd_bits, (sd_bits & 0x80000000u) != 0u);
        let sd_enc = ~sd_ord;
        let solid_slot = ap.bound_width * ap.bound_height * 4u + pixel_idx;
        let solid_prev = atomicMax(&histogram[solid_slot], sd_enc);
        if (ap.depth_prime != 0u) {
            return;  // priming dispatch: depth recorded, nothing plotted
        }
        let near_ord = ~max(solid_prev, sd_enc);
        let near_bits = select(~near_ord, near_ord ^ 0x80000000u, (near_ord & 0x80000000u) != 0u);
        let d_near = bitcast<f32>(near_bits);
        if (s.depth > d_near + ap.surface_thickness) {
            solid_weight = 1.0 - ap.solid_strength;
            if (solid_weight <= 0.0) {
                return;  // fully occluded — depth already recorded
            }
        }
    }

    // All four channels carry the sample's weight so the color
    // recovery ratio Σcolor/Σdensity is weight-invariant. Weight is
    // 1.0 unless depth-density compensation produced it.
    let weighted_scale = ap.color_scale * s.weight * solid_weight;
    let r_u32 = u32(clamp(s.r, 0.0, 1.0) * weighted_scale);
    let g_u32 = u32(clamp(s.g, 0.0, 1.0) * weighted_scale);
    let b_u32 = u32(clamp(s.b, 0.0, 1.0) * weighted_scale);
    let density_u32 = u32(weighted_scale);

    atomicAdd(&histogram[base + 0u], r_u32);
    atomicAdd(&histogram[base + 1u], g_u32);
    atomicAdd(&histogram[base + 2u], b_u32);
    atomicAdd(&histogram[base + 3u], density_u32);
}
