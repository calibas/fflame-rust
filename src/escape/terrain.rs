//! The terrain geometry source (docs/projects/heightfield-3d.md, phase
//! T1): a height field viewed through mode D's solid camera and lit by
//! mode D's rig.
//!
//! A TILE is a grid of `n x m` samples and an albedo per sample, in
//! CELL units: sample `(i, j)` sits at `(x, y) = (i, j)`, `y` running
//! north, and heights are in the same units. Between four samples the
//! surface is the bilinear patch through them.
//!
//! A sample holds the footprint's RAW height source -- the distance
//! estimate, the escape count, the relief's field -- and the walk maps
//! it to a height (`hf_f`) through the view's height and width. Every
//! map is monotone, so the mipmap keeps the raw extreme that maps to
//! the highest point (the least distance, the greatest count) and the
//! walk maps that too. So a height or a flank width is a uniform: it
//! re-walks the tile, it does not rebuild it. A tile set directly from
//! heights (`set_tile`) maps by the identity.
//!
//! Three passes:
//! - **The maximum mipmap** (Tevs, Ihrke and Seidel 2008). Level 0 holds
//!   each cell's highest corner, and every level above the maximum of a
//!   2x2 block below. A ray steps over every node it stays above, and
//!   descends only where it comes within a node's maximum of the
//!   surface. Real mip levels of one `R32Float`, so the walk binds one
//!   view.
//! - **The walk.** Per pixel:
//!   - the camera's ray (`ifs_ray`, mode D's);
//!   - the traversal, then the exact intersection with the leaf's
//!     bilinear patch;
//!   - a normal interpolated from the corners' own;
//!   - occlusion from the horizon around the hit;
//!   - a shadow ray per light, softened by how close it passed the
//!     surface.
//!
//!   It writes mode D's sixteen-byte geometry record.
//! - **The relight.** Mode D's rig, the same WGSL text
//!   (`assembler::ifs_rig_jittered`), over the record and the albedo,
//!   into the escape and simulation accumulator contract: rgb linear,
//!   alpha coverage.
//!
//! Antialiasing is accumulation: renders jittered within the pixel,
//! folded into a running mean (`accumulate`), weighted by coverage so a
//! silhouette's edge averages to a fraction of the background.
//!
//! So a lighting edit is a relight and not a walk, as it is for mode D.
//! The lights are WORLD-fixed here (plan H7): azimuth counter-clockwise
//! from east on the plane, elevation above it -- the 2D relief's light.

use super::ifs::SolidCamera;
use crate::config::SolidShadingSettings;
use wgpu::*;

/// Steps a ray may take before it is called a miss. A ray over a
/// 4096-cell tile skipping at the top levels takes a few dozen; this is
/// a guard, not a budget.
const HF_MAX_STEPS: u32 = 4096;

/// The walk's uniform. Mirrored by `TerrainParams` in [`WALK_COMMON`];
/// `fdata` uses mode D's slots so the rig's accessors read the same
/// places they do there.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TerrainParamsGpu {
    width: u32,
    height: u32,
    grid_n: u32,
    grid_m: u32,
    levels: u32,
    /// Bit 0: count traversal steps into the stats buffer.
    flags: u32,
    /// The rays' offset within their pixel, in pixels.
    jitter: [f32; 2],
    /// The eye, in cell units; w the slab's floor (see `slab_floor`).
    eye: [f32; 4],
    /// x: shadow strength (0 traces no shadow rays), y: penumbra
    /// softness k, z: occlusion reach in cells, w: shadow-ray bias.
    misc: [f32; 4],
    /// Mode D's rig slots: [2].w the FOV, [3] forward, [4] right, [5]
    /// up, [7].z the light count, [8]-[10] the material and fog, [11..]
    /// two per light (direction and power, colour).
    fdata: [[f32; 4]; 20],
}

/// One level of the mipmap build. Mirrored by `BuildParams`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BuildParamsGpu {
    dst_w: u32,
    dst_h: u32,
    src_w: u32,
    src_h: u32,
    /// 1 for level 0 (a cell's four corners), 2 above (a 2x2 block).
    step: u32,
    /// 0 keeps the maximum of the raw values, 1 the minimum: whichever
    /// the tile's map takes to the highest point.
    mode: u32,
    pad: [u32; 2],
}

const BUILD_WGSL: &str = r#"
struct BuildParams {
    dst_w: u32,
    dst_h: u32,
    src_w: u32,
    src_h: u32,
    step: u32,
    mode: u32,
    pad1: u32,
    pad2: u32,
};
@group(0) @binding(0) var<uniform> bp: BuildParams;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<r32float, write>;

fn at(x: i32, y: i32) -> f32 {
    let q = clamp(vec2<i32>(x, y), vec2<i32>(0, 0), vec2<i32>(i32(bp.src_w) - 1, i32(bp.src_h) - 1));
    return textureLoad(src, q, 0).r;
}

// Level 0: a cell's highest corner, so no bilinear patch rises above
// it. Above: the highest of the 2x2 block of nodes below. "Highest" is
// the raw extreme the tile's map takes to the top: the maximum, or for
// a distance the minimum.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= bp.dst_w || gid.y >= bp.dst_h) {
        return;
    }
    let s = i32(bp.step);
    let x = i32(gid.x) * s;
    let y = i32(gid.y) * s;
    let a = at(x, y);
    let b = at(x + 1, y);
    let c = at(x, y + 1);
    let d = at(x + 1, y + 1);
    let m = select(max(max(a, b), max(c, d)), min(min(a, b), min(c, d)), bp.mode == 1u);
    textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(m, 0.0, 0.0, 0.0));
}
"#;

/// The albedo's mip chain: each level the 2x2 box average of the one
/// below, the last row or column of an odd level repeated.
const ALBEDO_MIP_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dd = textureDimensions(dst);
    if (gid.x >= dd.x || gid.y >= dd.y) {
        return;
    }
    let sd = vec2<i32>(textureDimensions(src)) - vec2<i32>(1, 1);
    let p = vec2<i32>(gid.xy) * 2;
    let c = textureLoad(src, min(p, sd), 0) + textureLoad(src, min(p + vec2<i32>(1, 0), sd), 0)
        + textureLoad(src, min(p + vec2<i32>(0, 1), sd), 0) + textureLoad(src, min(p + vec2<i32>(1, 1), sd), 0);
    textureStore(dst, vec2<i32>(gid.xy), c * 0.25);
}
"#;

/// The ingest's uniform. Mirrored by `IngestParams`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct IngestParamsGpu {
    n: u32,
    m: u32,
    /// 8 distance, 9 escape count, anything else the relief's source.
    source: u32,
    /// 0 plateau, 1 hole, 2 lake.
    interior: u32,
    /// The region's origin in the footprint picture's pixels (x right,
    /// y down), and its size.
    ox: u32,
    oy: u32,
    rw: u32,
    rh: u32,
    /// The interior's colour on a plateau, where the colouring leaves
    /// it undrawn: the background's.
    background: [f32; 4],
    /// A lake's tint.
    tint: [f32; 4],
}

/// The ingest (heightfield plan, section 5): a REGION of a footprint
/// render -- its colour and height field -- into the tile being built,
/// rows flipped, the picture's top the terrain's north. A footprint
/// larger than one escape render arrives region by region.
///
/// The tile keeps the raw height source; the walk maps it to heights
/// (`hf_f`). Where the set's interior is, a sentinel the map sends to
/// the top (a plateau) or to the floor (a hole): for a distance, 0 and
/// +1e30; for a count or a relief, +1e30 and -1e30 -- the order each
/// map keeps, so the mipmap's extreme still bounds the highest point.
/// A lake's sentinel (3e30 in size) maps to height 0, the plain's, and
/// sorts last in that order (+ for a distance, whose mipmap keeps the
/// minimum; - for the others, which keep the maximum), so the land round
/// a lake still bounds its block.
/// The range pass measures a count's or a relief's range over every
/// region (the distance needs none).
const INGEST_WGSL: &str = r#"
struct IngestParams {
    n: u32,
    m: u32,
    source: u32,
    interior: u32,
    ox: u32,
    oy: u32,
    rw: u32,
    rh: u32,
    background: vec4<f32>,
    tint: vec4<f32>,
};
@group(0) @binding(0) var<uniform> ip: IngestParams;
@group(0) @binding(1) var colour: texture_2d<f32>;
@group(0) @binding(2) var hsrc: texture_2d<f32>;
@group(0) @binding(3) var<storage, read_write> range: array<atomic<u32>>;
@group(0) @binding(4) var out_raw: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var out_a: texture_storage_2d<rgba16float, write>;

// A float's order as an integer, so atomics can take its min and max.
fn ordered(f: f32) -> u32 {
    let b = bitcast<u32>(f);
    return select(b | 0x80000000u, ~b, (b & 0x80000000u) != 0u);
}

fn interior(c: vec4<f32>, g: f32) -> bool {
    return !(c.a > 0.0) || g <= -1.0e29;
}

@compute @workgroup_size(8, 8, 1)
fn range_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= ip.rw || gid.y >= ip.rh || ip.source == 8u) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let g = textureLoad(hsrc, p, 0).g;
    if (interior(textureLoad(colour, p, 0), g)) {
        return;
    }
    atomicMin(&range[0], ordered(g));
    atomicMax(&range[1], ordered(g));
}

@compute @workgroup_size(8, 8, 1)
fn ingest_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= ip.rw || gid.y >= ip.rh) {
        return;
    }
    let src = vec2<i32>(gid.xy);
    // Tile row j is picture row m - 1 - j: the picture's top is north.
    let dst = vec2<i32>(i32(ip.ox + gid.x), i32(ip.m - 1u - (ip.oy + gid.y)));
    let c = textureLoad(colour, src, 0);
    let g = textureLoad(hsrc, src, 0).g;
    let distance = ip.source == 8u;
    if (interior(c, g)) {
        if (ip.interior == 1u) {
            textureStore(out_raw, dst, vec4<f32>(select(-1.0e30, 1.0e30, distance), 0.0, 0.0, 0.0));
            textureStore(out_a, dst, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        } else if (ip.interior == 2u) {
            textureStore(out_raw, dst, vec4<f32>(select(-3.0e30, 3.0e30, distance), 0.0, 0.0, 0.0));
            textureStore(out_a, dst, vec4<f32>(ip.tint.rgb, 1.0));
        } else {
            // The interior's own colour where the colouring draws one,
            // the background's where it does not.
            textureStore(out_raw, dst, vec4<f32>(select(1.0e30, 0.0, distance), 0.0, 0.0, 0.0));
            textureStore(out_a, dst, vec4<f32>(select(ip.background.rgb, c.rgb, c.a > 0.0), 1.0));
        }
        return;
    }
    textureStore(out_raw, dst, vec4<f32>(select(g, max(g, 0.0), distance), 0.0, 0.0, 0.0));
    textureStore(out_a, dst, vec4<f32>(c.rgb, 1.0));
}
"#;

/// The accumulation: a jittered render folded into the running mean,
/// read from one texture of a pair and written to the other. Averaged
/// PREMULTIPLIED and stored straight, as the accumulator contract
/// wants: a sample that misses (alpha 0) leaves the colour alone and
/// lowers the coverage, so an edge pixel is the surface's colour at a
/// fraction of its alpha rather than a darker colour at full alpha.
const ACCUM_WGSL: &str = r#"
struct AccumParams {
    width: u32,
    height: u32,
    count: u32,
    pad: u32,
};
@group(0) @binding(0) var<uniform> ap: AccumParams;
@group(0) @binding(1) var sample_tex: texture_2d<f32>;
@group(0) @binding(2) var prev: texture_2d<f32>;
@group(0) @binding(3) var next: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= ap.width || gid.y >= ap.height) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let s = textureLoad(sample_tex, p, 0);
    if (ap.count == 0u) {
        textureStore(next, p, s);
        return;
    }
    let o = textureLoad(prev, p, 0);
    let w = 1.0 / f32(ap.count + 1u);
    let a = mix(o.a, s.a, w);
    let pre = mix(o.rgb * o.a, s.rgb * s.a, w);
    var rgb = vec3<f32>(0.0);
    if (a > 0.0) {
        rgb = pre / a;
    }
    textureStore(next, p, vec4<f32>(rgb, a));
}
"#;

/// What a footprint's ingest needs to know: what its height source is,
/// and what the interior becomes. The height and the flank width are
/// the view's (`TerrainView`), applied by the walk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainIngest {
    /// The height source the footprint wrote: 8 distance, 9 escape
    /// count, anything else the relief's own.
    pub source: u32,
    /// The interior: 0 a plateau, 1 a hole, 2 a lake.
    pub interior: u32,
    /// The plateau's colour where the colouring leaves the interior
    /// undrawn: the background's.
    pub background: [f32; 3],
    /// A lake's colour under its surface.
    pub tint: [f32; 3],
}

impl TerrainIngest {
    /// The walk's map for this source: 1 distance, 2 the escape count's
    /// log curve, 3 the relief's linear one.
    pub fn mode(&self) -> u32 {
        match self.source {
            8 => 1,
            9 => 2,
            _ => 3,
        }
    }
}

/// Shared by the walk and the relight: the uniform, the rig's
/// accessors over it, and the rig itself.
const WALK_COMMON: &str = r#"
struct TerrainParams {
    width: u32,
    height: u32,
    grid_n: u32,
    grid_m: u32,
    levels: u32,
    flags: u32,
    jitter: vec2<f32>,
    eye: vec4<f32>,
    misc: vec4<f32>,
    fdata: array<vec4<f32>, 20>,
};
@group(0) @binding(0) var<uniform> params: TerrainParams;

fn ifs_fov() -> f32 { return params.fdata[2].w; }
fn ifs_forward() -> vec3<f32> { return params.fdata[3].xyz; }
fn ifs_right() -> vec3<f32> { return params.fdata[4].xyz; }
fn ifs_up() -> vec3<f32> { return params.fdata[5].xyz; }
fn ifs_shading_strength() -> f32 { return params.fdata[8].x; }
fn ifs_ambient() -> f32 { return params.fdata[8].y; }
fn ifs_diffuse() -> f32 { return params.fdata[8].z; }
fn ifs_specular() -> f32 { return params.fdata[8].w; }
fn ifs_shininess() -> f32 { return params.fdata[9].x; }
fn ifs_occlusion_strength() -> f32 { return params.fdata[9].y; }
fn ifs_fog_strength() -> f32 { return params.fdata[9].z; }
fn ifs_fog_start() -> f32 { return params.fdata[9].w; }
fn ifs_fog_color() -> vec3<f32> { return params.fdata[10].xyz; }
fn ifs_shadow_strength() -> f32 { return params.misc.x; }
fn ifs_light_count() -> u32 { return u32(clamp(params.fdata[7].z, 0.0, 4.0)); }
fn ifs_light_dir(i: u32) -> vec3<f32> { return params.fdata[11u + i * 2u].xyz; }
fn ifs_light_power(i: u32) -> f32 { return params.fdata[11u + i * 2u].w; }
fn ifs_light_color(i: u32) -> vec3<f32> { return params.fdata[12u + i * 2u].xyz; }

//__IFS_RIG__
"#;

/// The ground's functions, shared by the walk and the path tracer: the
/// raw samples' map to heights, the section traversal and the ground's,
/// heights, normals and occlusion. Each includer declares the bindings
/// they read (`hf_raw_tex`, `hf_mips`, `hf_range`, `hf_sections`,
/// `hf_nodes`).
const HF_HELPERS_WGSL: &str = r#"
const HF_MAX_STEPS: u32 = __HF_MAX_STEPS__u;

//__HF_GROUND__

// The count's or the relief's range, read once per invocation.
var<private> hf_lo: f32;
var<private> hf_span: f32;

fn hf_unordered(u: u32) -> f32 {
    return bitcast<f32>(select(~u, u & 0x7fffffffu, (u & 0x80000000u) != 0u));
}

fn hf_load_range() {
    let lo = hf_unordered(hf_range[0]);
    let hi = hf_unordered(hf_range[1]);
    // No escaped sample at all leaves the range empty (and NaN).
    if (!(lo <= hi)) {
        hf_lo = 0.0;
        hf_span = 0.0;
    } else {
        hf_lo = lo;
        hf_span = hi - lo;
    }
}

// A raw sample of the current section, its grid's edge held beyond it.
fn hf_raw(i: i32, j: i32) -> f32 {
    let q = clamp(vec2<i32>(i, j), vec2<i32>(0, 0), vec2<i32>(hf_sec.dims.xy) - vec2<i32>(1, 1));
    return textureLoad(hf_raw_tex, q, i32(hf_sec.dims.z), 0).r;
}

// A raw value's height in the WORLD's units: the identity (a tile set
// from heights), the distance's H exp(-d / w) -- d the raw estimate in
// the section's pixels times its texel -- the count's log curve, or the
// relief's linear one, each with the interior's sentinels. Monotone,
// so a section's mipmap of raw extremes bounds its highest point.
fn hf_f(raw: f32) -> f32 {
    // A lake: flat at the plain's level.
    if (abs(raw) >= 2.0e30) {
        return 0.0;
    }
    let mode = u32(params.fdata[1].w);
    let top = params.fdata[6].x;
    if (mode == 0u) {
        return raw;
    }
    if (mode == 1u) {
        if (raw >= 1.0e29) {
            return params.eye.w;
        }
        return top * exp(-max(raw, 0.0) * hf_sec.geo.z / max(params.fdata[6].y, 1.0e-30));
    }
    if (raw >= 1.0e29) {
        return top;
    }
    if (raw <= -1.0e29) {
        return params.eye.w;
    }
    if (mode == 2u) {
        return top * clamp(log(1.0 + max(raw - hf_lo, 0.0)) / max(log(1.0 + hf_span), 1.0e-6), 0.0, 1.0);
    }
    return top * clamp((raw - hf_lo) / max(hf_span, 1.0e-20), 0.0, 1.0);
}

// A sample's height in the current section's cells.
fn hf_h(i: i32, j: i32) -> f32 {
    return hf_f(hf_raw(i, j)) * hf_sec.geo.w;
}

// The surface's height at (x, y) in the current section's cells: the
// bilinear patch of its cell.
fn hf_height_at(x: f32, y: f32) -> f32 {
    let n = i32(hf_sec.dims.x);
    let m = i32(hf_sec.dims.y);
    let cx = clamp(x, 0.0, f32(n - 1));
    let cy = clamp(y, 0.0, f32(m - 1));
    let a = min(i32(floor(cx)), n - 2);
    let b = min(i32(floor(cy)), m - 2);
    let u = cx - f32(a);
    let v = cy - f32(b);
    let h0 = mix(hf_h(a, b), hf_h(a + 1, b), u);
    let h1 = mix(hf_h(a, b + 1), hf_h(a + 1, b + 1), u);
    return mix(h0, h1, v);
}

// The first t in [t0, t1] where the ray meets cell (a, b)'s bilinear
// patch, or -1. Rebased at t0, so a far eye loses no precision in the
// cell's own coordinates. Along the ray the patch is a quadratic in t,
// and so is the gap between them: solved exactly, the smaller root kept.
fn hf_patch(a: i32, b: i32, o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32) -> f32 {
    let h00 = hf_h(a, b);
    let h10 = hf_h(a + 1, b);
    let h01 = hf_h(a, b + 1);
    let h11 = hf_h(a + 1, b + 1);
    let q = o + d * t0;
    let u0 = q.x - f32(a);
    let v0 = q.y - f32(b);
    let bb = h10 - h00;
    let cc = h01 - h00;
    let dd = h00 - h10 - h01 + h11;
    let ha = h00 + bb * u0 + cc * v0 + dd * u0 * v0;
    let hb = bb * d.x + cc * d.y + dd * (u0 * d.y + v0 * d.x);
    let hc = dd * d.x * d.y;
    // gap(s) = alpha + beta s + gamma s^2, the ray above the patch.
    let alpha = q.z - ha;
    let beta = d.z - hb;
    let gamma = -hc;
    let len = t1 - t0;
    if (alpha <= 0.0) {
        return t0;
    }
    var s = -1.0;
    if (abs(gamma) * len < 1.0e-7 * max(abs(beta), 1.0e-20)) {
        if (beta < 0.0) {
            s = -alpha / beta;
        }
    } else {
        let disc = beta * beta - 4.0 * gamma * alpha;
        if (disc >= 0.0) {
            let sq = sqrt(disc);
            let qq = -0.5 * (beta + select(-sq, sq, beta >= 0.0));
            var r1 = -1.0;
            var r2 = -1.0;
            if (abs(qq) > 0.0) {
                r1 = qq / gamma;
                r2 = alpha / qq;
            }
            let lo = min(r1, r2);
            let hi = max(r1, r2);
            s = select(hi, lo, lo >= 0.0);
        }
    }
    if (s >= 0.0 && s <= len) {
        return t0 + s;
    }
    return -1.0;
}

struct HfHit {
    t: f32,
    hit: bool,
    // The closest the ray came to the surface over its distance, times
    // the softness: a penumbra, as mode D's shadow rays measure one.
    soft: f32,
    steps: u32,
    // The section the hit is in.
    sec: i32,
}

// The maximum-mipmap traversal of the current section over [ta, tb], in
// its cells (`o`, `d` already scaled into them; t is the world's ray
// parameter either way). `soft_k` > 0 also measures the penumbra at the
// leaves the ray passed over, in the world's units.
fn hf_trace_section(o: vec3<f32>, d: vec3<f32>, ta: f32, tb: f32, soft_k: f32) -> HfHit {
    var out: HfHit;
    out.t = -1.0;
    out.hit = false;
    out.soft = 1.0;
    out.steps = 0u;
    out.sec = -1;
    let cn = f32(hf_sec.dims.x - 1u);
    let cm = f32(hf_sec.dims.y - 1u);
    let top_level = i32(hf_sec.dims.w) - 1;
    let top = hf_f(textureLoad(hf_mips, vec2<i32>(0, 0), i32(hf_sec.dims.z), top_level).r) * hf_sec.geo.w;

    // Clip to the section's box: x in [0, cn], y in [0, cm], z between
    // the floor and its highest point.
    let floor_z = params.eye.w * hf_sec.geo.w;
    var t0 = ta;
    var t1 = tb;
    if (abs(d.x) < 1.0e-12) {
        if (o.x < 0.0 || o.x > cn) {
            return out;
        }
    } else {
        let ta2 = -o.x / d.x;
        let tb2 = (cn - o.x) / d.x;
        t0 = max(t0, min(ta2, tb2));
        t1 = min(t1, max(ta2, tb2));
    }
    if (abs(d.y) < 1.0e-12) {
        if (o.y < 0.0 || o.y > cm) {
            return out;
        }
    } else {
        let ta2 = -o.y / d.y;
        let tb2 = (cm - o.y) / d.y;
        t0 = max(t0, min(ta2, tb2));
        t1 = min(t1, max(ta2, tb2));
    }
    if (d.z < 0.0) {
        let tz = (top - o.z) / d.z;
        if (o.z > top) {
            t0 = max(t0, tz);
        }
        // Out through the floor.
        t1 = min(t1, (floor_z - o.z) / d.z);
    } else {
        if (o.z > top) {
            return out;
        }
        if (d.z > 0.0) {
            t1 = min(t1, (top - o.z) / d.z);
            if (o.z < floor_z) {
                t0 = max(t0, (floor_z - o.z) / d.z);
            }
        } else if (o.z < floor_z) {
            return out;
        }
    }
    if (!(t0 <= t1)) {
        return out;
    }

    var t = t0;
    var level = top_level;
    // The step past a node's edge: larger than a position's rounding at
    // these coordinates, so a point just over the edge is not rounded
    // back into the node it left (whose exit would then lie behind t).
    // In t, whose scale is the world's: a cell is a texel of it.
    let span = max(max(abs(o.x), abs(o.y)), max(cn, cm));
    let texel = hf_sec.geo.z;
    let eps_base = (1.0e-4 + span * 4.0e-7) * texel;
    loop {
        if (out.steps >= HF_MAX_STEPS) {
            break;
        }
        out.steps = out.steps + 1u;
        let p = o + d * t;
        let size = f32(1 << u32(level));
        let dims = vec2<i32>(
            (i32(hf_sec.dims.x) - 1 + (1 << u32(level)) - 1) >> u32(level),
            (i32(hf_sec.dims.y) - 1 + (1 << u32(level)) - 1) >> u32(level),
        );
        let a = clamp(i32(floor(p.x / size)), 0, dims.x - 1);
        let b = clamp(i32(floor(p.y / size)), 0, dims.y - 1);
        let lo = vec2<f32>(f32(a), f32(b)) * size;
        let hi = min(lo + vec2<f32>(size, size), vec2<f32>(cn, cm));
        var tx = 1.0e30;
        if (d.x > 0.0) {
            tx = (hi.x - o.x) / d.x;
        } else if (d.x < 0.0) {
            tx = (lo.x - o.x) / d.x;
        }
        var ty = 1.0e30;
        if (d.y > 0.0) {
            ty = (hi.y - o.y) / d.y;
        } else if (d.y < 0.0) {
            ty = (lo.y - o.y) / d.y;
        }
        let t_far = min(min(tx, ty), t1);
        let zmax = hf_f(textureLoad(hf_mips, vec2<i32>(a, b), i32(hf_sec.dims.z), level).r) * hf_sec.geo.w;
        let z_lo = min(o.z + d.z * t, o.z + d.z * t_far);
        // Past the node, never backward: a far ray still advances.
        let eps = eps_base + abs(t_far) * 2.0e-7;
        if (z_lo > zmax) {
            // The ray stays above everything in this node.
            t = max(t_far, t) + eps;
            level = min(level + 1, top_level);
            if (t >= t1) {
                break;
            }
            continue;
        }
        if (level > 0) {
            level = level - 1;
            continue;
        }
        let th = hf_patch(a, b, o, d, t, t_far);
        if (th >= 0.0) {
            out.t = th;
            out.hit = true;
            return out;
        }
        if (soft_k > 0.0 && t > 0.0) {
            // How close the ray passed this cell's surface, at its ends,
            // in the world's units over the world's distance.
            let p1 = o + d * t_far;
            let gap = min(p.z - hf_height_at(p.x, p.y), p1.z - hf_height_at(p1.x, p1.y)) * texel;
            out.soft = min(out.soft, clamp(soft_k * gap / max(t, 1.0e-3 * texel), 0.0, 1.0));
        }
        t = max(t_far, t) + eps;
        level = min(level + 1, top_level);
        if (t >= t1) {
            break;
        }
    }
    return out;
}

// The ground's traversal in the world: the ray clipped to the ground's
// slab and its root grid, then region by region -- each the square over
// which one section, the finest ready there, answers (`hf_lookup`) --
// each traced in its section's cells.
fn hf_trace(o: vec3<f32>, d: vec3<f32>, tmax: f32, soft_k: f32) -> HfHit {
    var out: HfHit;
    out.t = -1.0;
    out.hit = false;
    out.soft = 1.0;
    out.steps = 0u;
    out.sec = -1;
    let top = params.fdata[0].w;
    let floor_z = params.eye.w;
    let ro = params.fdata[0].xy;
    let ext = vec2<f32>(params.fdata[1].x, params.fdata[1].y) * params.fdata[0].z;
    var t0 = 0.0;
    var t1 = tmax;
    if (abs(d.x) < 1.0e-12) {
        if (o.x < ro.x || o.x > ro.x + ext.x) {
            return out;
        }
    } else {
        let ta = (ro.x - o.x) / d.x;
        let tb = (ro.x + ext.x - o.x) / d.x;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
    }
    if (abs(d.y) < 1.0e-12) {
        if (o.y < ro.y || o.y > ro.y + ext.y) {
            return out;
        }
    } else {
        let ta = (ro.y - o.y) / d.y;
        let tb = (ro.y + ext.y - o.y) / d.y;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
    }
    if (d.z < 0.0) {
        if (o.z > top) {
            t0 = max(t0, (top - o.z) / d.z);
        }
        t1 = min(t1, (floor_z - o.z) / d.z);
    } else {
        if (o.z > top) {
            return out;
        }
        if (d.z > 0.0) {
            t1 = min(t1, (top - o.z) / d.z);
        }
    }
    if (!(t0 <= t1)) {
        return out;
    }
    var t = t0;
    // Look a hair ahead: a ray entering the ground's box sits exactly on
    // its edge, where a rounding would put it outside.
    let ahead = params.fdata[0].z * 1.0e-6;
    loop {
        if (out.steps >= HF_MAX_STEPS) {
            break;
        }
        let p = o + d * (t + ahead);
        let look = hf_lookup(p.xy);
        if (!(look.size > 0.0)) {
            break;
        }
        let lo = look.lo;
        let hi = lo + vec2<f32>(look.size, look.size);
        var tx = 1.0e30;
        if (d.x > 0.0) {
            tx = (hi.x - o.x) / d.x;
        } else if (d.x < 0.0) {
            tx = (lo.x - o.x) / d.x;
        }
        var ty = 1.0e30;
        if (d.y > 0.0) {
            ty = (hi.y - o.y) / d.y;
        } else if (d.y < 0.0) {
            ty = (lo.y - o.y) / d.y;
        }
        let te = min(min(tx, ty), t1);
        if (look.sec >= 0) {
            hf_sec = hf_sections[look.sec];
            let inv = hf_sec.geo.w;
            let os = vec3<f32>((o.xy - hf_sec.geo.xy) * inv, o.z * inv);
            let h = hf_trace_section(os, d * inv, t, te, soft_k);
            out.steps = out.steps + h.steps;
            out.soft = min(out.soft, h.soft);
            if (h.hit) {
                out.t = h.t;
                out.hit = true;
                out.sec = look.sec;
                return out;
            }
        }
        out.steps = out.steps + 1u;
        let eps = look.size * 1.0e-5 + abs(te) * 2.0e-7;
        t = max(te, t) + eps;
        if (t >= t1) {
            break;
        }
    }
    return out;
}

// The ground's height at q in the world (the floor where there is
// none): the finest ready section's bilinear patch there.
fn hf_ground(q: vec2<f32>) -> f32 {
    let look = hf_lookup(q);
    if (look.sec < 0) {
        return params.eye.w;
    }
    hf_sec = hf_sections[look.sec];
    let s = (q - hf_sec.geo.xy) * hf_sec.geo.w;
    return hf_height_at(s.x, s.y) * hf_sec.geo.z;
}

// A sample's own normal, from its neighbours (one-sided at the edge).
fn hf_corner_normal(i: i32, j: i32) -> vec3<f32> {
    let i0 = max(i - 1, 0);
    let i1 = min(i + 1, i32(hf_sec.dims.x) - 1);
    let j0 = max(j - 1, 0);
    let j1 = min(j + 1, i32(hf_sec.dims.y) - 1);
    let dx = (hf_h(i1, j) - hf_h(i0, j)) / f32(max(i1 - i0, 1));
    let dy = (hf_h(i, j1) - hf_h(i, j0)) / f32(max(j1 - j0, 1));
    return vec3<f32>(-dx, -dy, 1.0);
}

// The outward normal of the section's side face nearest p (cells).
fn hf_wall_normal(p: vec3<f32>) -> vec3<f32> {
    let cn = f32(hf_sec.dims.x - 1u);
    let cm = f32(hf_sec.dims.y - 1u);
    let dist = vec4<f32>(p.x, cn - p.x, p.y, cm - p.y);
    let m = min(min(dist.x, dist.y), min(dist.z, dist.w));
    if (dist.x <= m) {
        return vec3<f32>(-1.0, 0.0, 0.0);
    }
    if (dist.y <= m) {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    if (dist.z <= m) {
        return vec3<f32>(0.0, -1.0, 0.0);
    }
    return vec3<f32>(0.0, 1.0, 0.0);
}

// The shading normal at (x, y) in the current section's cells: the
// corners' normals, bilinearly, so the light is smooth across a cell
// even where the patch is faceted. A section's slopes are the world's.
fn hf_normal(x: f32, y: f32) -> vec3<f32> {
    let n = i32(hf_sec.dims.x);
    let m = i32(hf_sec.dims.y);
    let cx = clamp(x, 0.0, f32(n - 1));
    let cy = clamp(y, 0.0, f32(m - 1));
    let a = min(i32(floor(cx)), n - 2);
    let b = min(i32(floor(cy)), m - 2);
    let u = cx - f32(a);
    let v = cy - f32(b);
    let n0 = mix(hf_corner_normal(a, b), hf_corner_normal(a + 1, b), u);
    let n1 = mix(hf_corner_normal(a, b + 1), hf_corner_normal(a + 1, b + 1), u);
    return normalize(mix(n0, n1, v));
}

// Occlusion from the horizon, in the world: in eight directions, the
// steepest rise within the reach, as the sine of its elevation; one
// minus their mean. A height field's own occlusion, and deterministic
// -- no noise to accumulate away.
fn hf_occlusion(p: vec3<f32>, reach: f32) -> f32 {
    if (!(reach > 0.0)) {
        return 1.0;
    }
    var occ = 0.0;
    for (var k = 0; k < 8; k = k + 1) {
        let ang = f32(k) * 0.785398163;
        let dir = vec2<f32>(cos(ang), sin(ang));
        var best = 0.0;
        for (var r = 1; r <= 3; r = r + 1) {
            let dist = reach * f32(r) / 3.0;
            let q = p.xy + dir * dist;
            let rise = hf_ground(q) - p.z;
            best = max(best, rise * inverseSqrt(rise * rise + dist * dist));
        }
        occ = occ + best;
    }
    return clamp(1.0 - occ / 8.0, 0.0, 1.0);
}

"#;

const WALK_WGSL: &str = r#"
@group(0) @binding(1) var hf_raw_tex: texture_2d_array<f32>;
@group(0) @binding(2) var hf_mips: texture_2d_array<f32>;
@group(0) @binding(3) var<storage, read_write> ifs_geom: array<vec4<u32>>;
@group(0) @binding(4) var<storage, read_write> hf_stats: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read> hf_range: array<u32>;
@group(0) @binding(6) var<storage, read> hf_sections: array<HfSection>;
@group(0) @binding(7) var<storage, read> hf_nodes: array<HfNode>;

//__HF_HELPERS__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }
    let idx = gid.y * params.width + gid.x;
    hf_load_range();
    let o = params.eye.xyz;
    let d = ifs_ray(gid.x, gid.y);
    // The ground ends at the view depth `far` (fdata[6].w), where the
    // fog has reached the background.
    let tmax = params.fdata[6].w / max(dot(d, ifs_forward()), 1.0e-4);
    let h = hf_trace(o, d, tmax, 0.0);
    var steps = h.steps;
    if (!h.hit) {
        ifs_geom[idx] = vec4<u32>(0u, 0u, 0u, 0u);
    } else {
        let p = o + d * h.t;
        hf_sec = hf_sections[h.sec];
        let ps = vec3<f32>((p.xy - hf_sec.geo.xy) * hf_sec.geo.w, p.z * hf_sec.geo.w);
        var n = hf_normal(ps.x, ps.y);
        var ao = hf_occlusion(p, params.misc.z);
        // A ray that entered a section through its side under the
        // surface hit a WALL -- the ground's edge, or a step between
        // sections of different detail -- lit by its own outward normal,
        // and open to the sky. Only ON the boundary: elsewhere a point a
        // rounding under a steep flank is the flank.
        hf_sec = hf_sections[h.sec];
        let edge = min(min(ps.x, f32(hf_sec.dims.x - 1u) - ps.x), min(ps.y, f32(hf_sec.dims.y - 1u) - ps.y));
        if (edge < 1.0e-2 && hf_height_at(ps.x, ps.y) - ps.z > 1.0e-3) {
            n = hf_wall_normal(ps);
            ao = 1.0;
        }
        var sun = vec4<f32>(1.0, 1.0, 1.0, 1.0);
        if (ifs_shadow_strength() > 0.0) {
            // Past a position's rounding at these coordinates, or a
            // surface shadows itself in speckles.
            let bias = hf_sec.geo.z * 1.0e-3 + (length(p) + length(o)) * 1.0e-6;
            let start = p + n * bias;
            for (var li = 0u; li < ifs_light_count(); li = li + 1u) {
                let ld = ifs_light_dir(li);
                if (dot(n, ld) <= 0.0) {
                    continue;
                }
                let s = hf_trace(start, ld, 1.0e30, params.misc.y);
                steps = steps + s.steps;
                sun[li] = select(s.soft, 0.0, s.hit);
            }
        }
        ifs_geom[idx] = ifs_pack_geom(n, ao, sun, h.t);
    }
    if ((params.flags & 1u) != 0u) {
        atomicAdd(&hf_stats[0], h.steps);
        atomicMax(&hf_stats[1], h.steps);
        atomicAdd(&hf_stats[2], 1u);
        atomicAdd(&hf_stats[3], steps);
    }
}
"#;

/// The albedo's filtered lookup, shared by the relight and the path
/// tracer (each declares `hf_albedo` and `hf_sampler`).
const HF_ALBEDO_WGSL: &str = r#"
// The albedo at the world point q, filtered over `foot` world units:
// bilinear between the samples where a ray's share of a pixel is
// smaller than a texel, the mip chain's averages where it covers
// several -- so distant ground is the average of its texels rather than
// whichever one a ray struck.
fn hf_albedo_at(q: vec2<f32>, foot: f32) -> vec4<f32> {
    let look = hf_lookup(q);
    if (look.sec < 0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    hf_sec = hf_sections[look.sec];
    let s = (q - hf_sec.geo.xy) * hf_sec.geo.w;
    let dims = vec2<f32>(textureDimensions(hf_albedo));
    let uv = (s + vec2<f32>(0.5, 0.5)) / dims;
    let lod = clamp(log2(max(foot * hf_sec.geo.w, 1.0e-6)), 0.0, f32(textureNumLevels(hf_albedo) - 1u));
    return textureSampleLevel(hf_albedo, hf_sampler, uv, i32(hf_sec.dims.z), lod);
}

"#;

const RELIGHT_WGSL: &str = r#"
@group(0) @binding(1) var hf_albedo: texture_2d_array<f32>;
@group(0) @binding(2) var<storage, read> ifs_geom: array<vec4<u32>>;
@group(0) @binding(3) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var hf_sampler: sampler;
@group(0) @binding(6) var<storage, read> hf_sections: array<HfSection>;
@group(0) @binding(7) var<storage, read> hf_nodes: array<HfNode>;

//__HF_GROUND__

//__HF_ALBEDO__

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }
    let idx = gid.y * params.width + gid.x;
    let g = ifs_geom[idx];
    let t = bitcast<f32>(g.w);
    let px = vec2<i32>(i32(gid.x), i32(gid.y));
    // No surface: transparent, so the tonemap's background shows.
    if (!(t > 0.0)) {
        textureStore(out_tex, px, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    let dir = ifs_ray(gid.x, gid.y);
    let p = params.eye.xyz + dir * t;
    let nxy = unpack2x16float(g.x);
    let nz_ao = unpack2x16float(g.y);
    let n = vec3<f32>(nxy, nz_ao.x);
    // A ray's share of a pixel at the hit, in the world's units, widened
    // where the surface is seen at a slant.
    let foot = t * params.fdata[6].z / sqrt(max(abs(dot(n, dir)), 0.05));
    let albedo = hf_albedo_at(p.xy, foot);
    let sun = unpack4x8unorm(g.z);
    let rgb = ifs_rig(albedo.rgb, n, nz_ao.y, sun, dir, t);
    // The fog: the ground's coverage fading with view depth past its
    // start (fdata[7].x its density, .y its start), so it melts into
    // the background exactly.
    var alpha = clamp(albedo.a, 0.0, 1.0);
    if (params.fdata[7].x > 0.0) {
        let depth = t * dot(dir, ifs_forward());
        alpha = alpha * exp(-params.fdata[7].x * max(depth - params.fdata[7].y, 0.0));
    }
    textureStore(out_tex, px, vec4<f32>(rgb, alpha));
}
"#;

/// The path tracer (heightfield plan, section 4's path-traced tier;
/// phase T3): each invocation adds `samples` samples of its pixel to a
/// running SUM, in sample order, so the sum is the same bits however
/// the samples are split into dispatches. A sample's random numbers are
/// keyed by (pixel, sample index, seed): reproducible, and independent
/// of the batch.
///
/// A sample: a ray jittered within the pixel; at each surface the
/// lights sampled directly over their angular size with a shadow ray
/// (next-event estimation), then a cosine-weighted bounce whose escape
/// sees the environment -- the background's colour, uniform -- with
/// Russian roulette from the second bounce. Lambert on the albedo; the
/// lights in the rig's units, so a lit surface reads as it does in the
/// lit tier. Coverage is the primary hit's, faded by the fog.
const PATH_WGSL: &str = r#"
@group(0) @binding(1) var hf_raw_tex: texture_2d_array<f32>;
@group(0) @binding(2) var hf_mips: texture_2d_array<f32>;
@group(0) @binding(3) var hf_albedo: texture_2d_array<f32>;
@group(0) @binding(4) var hf_sampler: sampler;
@group(0) @binding(5) var<storage, read> hf_range: array<u32>;
@group(0) @binding(6) var<storage, read> hf_sections: array<HfSection>;
@group(0) @binding(7) var<storage, read> hf_nodes: array<HfNode>;

//__HF_HELPERS__

//__HF_ALBEDO__

// The height field's side of the path tracer (`path_core`). Rays start
// at the eye, in the world's units.
fn pt_eye() -> vec3<f32> {
    return params.eye.xyz;
}

fn pt_scene_begin() {
    hf_load_range();
}

// The surface a trace found: its shading normal (a wall's own on a
// section's side), and its albedo filtered over the footprint of a ray
// that has come `travelled + t` -- that times its share of a pixel.
fn pt_surface(o: vec3<f32>, d: vec3<f32>, h: HfHit, travelled: f32) -> PtHit {
    var out: PtHit;
    out.hit = h.hit;
    out.t = h.t;
    if (!h.hit) {
        return out;
    }
    let p = o + d * h.t;
    hf_sec = hf_sections[h.sec];
    let ps = vec3<f32>((p.xy - hf_sec.geo.xy) * hf_sec.geo.w, p.z * hf_sec.geo.w);
    var n = hf_normal(ps.x, ps.y);
    hf_sec = hf_sections[h.sec];
    let edge = min(min(ps.x, f32(hf_sec.dims.x - 1u) - ps.x), min(ps.y, f32(hf_sec.dims.y - 1u) - ps.y));
    if (edge < 1.0e-2 && hf_height_at(ps.x, ps.y) - ps.z > 1.0e-3) {
        n = hf_wall_normal(ps);
    }
    // The coat: the config's gloss -- or, on a cell whose four corners
    // are all the lake's, water: flat, reflecting 0.02 head on, at the
    // lake's roughness, over the tint.
    out.f0 = pt.mat.x;
    out.rough = pt.mat.y;
    if (hf_lake_cell(ps.x, ps.y)) {
        n = vec3<f32>(0.0, 0.0, 1.0);
        out.f0 = 0.02;
        out.rough = pt.mat.w;
    }
    // The side the ray sees: a height field is two-sided.
    if (dot(n, d) > 0.0) {
        n = -n;
    }
    out.n = n;
    let foot = (travelled + h.t) * pt.misc.x / sqrt(max(abs(dot(n, d)), 0.05));
    out.albedo = hf_albedo_at(p.xy, foot);
    // Off the surface by a thousandth of its cell, and by what f32 can
    // tell apart at its distance from the origin and the eye's.
    out.bias = hf_sec.geo.z * 1.0e-3 + (length(p) + length(params.eye.xyz)) * 1.0e-6;
    return out;
}

// Whether the current section's cell under (x, y) is all lake.
fn hf_lake_cell(x: f32, y: f32) -> bool {
    let n = i32(hf_sec.dims.x);
    let m = i32(hf_sec.dims.y);
    let a = min(i32(floor(clamp(x, 0.0, f32(n - 1)))), n - 2);
    let b = min(i32(floor(clamp(y, 0.0, f32(m - 1)))), m - 2);
    return abs(hf_raw(a, b)) >= 2.0e30 && abs(hf_raw(a + 1, b)) >= 2.0e30
        && abs(hf_raw(a, b + 1)) >= 2.0e30 && abs(hf_raw(a + 1, b + 1)) >= 2.0e30;
}

fn pt_scene_next(o: vec3<f32>, d: vec3<f32>, travelled: f32) -> PtHit {
    return pt_surface(o, d, hf_trace(o, d, 1.0e30, 0.0), travelled);
}

fn pt_scene_visible(o: vec3<f32>, d: vec3<f32>, travelled: f32) -> bool {
    return !hf_trace(o, d, 1.0e30, 0.0).hit;
}

// One sample of the pixel: (radiance times coverage, coverage). The
// coverage is the first surface's albedo's -- a hole lets the
// background through -- faded by the fog with depth, as the lit tier
// fades it; the ground ends at the distance the view renders to.
fn pt_sample(px: u32, py: u32) -> vec4<f32> {
    let j = pt_rand2() - vec2<f32>(0.5, 0.5);
    let ray = pt_ray(px, py, j.x, j.y);
    let tmax = params.fdata[6].w / max(dot(ray.d, ifs_forward()), 1.0e-4);
    let h = pt_surface(ray.o, ray.d, hf_trace(ray.o, ray.d, tmax, 0.0), 0.0);
    if (!h.hit) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    var coverage = clamp(h.albedo.a, 0.0, 1.0);
    if (params.fdata[7].x > 0.0) {
        let depth = h.t * dot(ray.d, ifs_forward());
        coverage = coverage * exp(-params.fdata[7].x * max(depth - params.fdata[7].y, 0.0));
    }
    let l = min(pt_path(ray.o, ray.d, h), vec3<f32>(pt.misc.y));
    return vec4<f32>(l * coverage, coverage);
}
"#;

/// The ground's index, shared by the walk and the relight: its sections
/// and the quadtree over them (`set_ground`).
const GROUND_WGSL: &str = r#"
// A section: its sample (0, 0) in the world, the world's units per cell
// and their inverse; its samples, layer and mip levels.
struct HfSection {
    geo: vec4<f32>,
    dims: vec4<u32>,
};

// A quadtree node: its children (south-west, south-east, north-west,
// north-east; -1 none) and the section that covers its square (-1 none
// ready).
struct HfNode {
    child: vec4<i32>,
    sec: vec4<i32>,
};

// The section the helpers read.
var<private> hf_sec: HfSection;

struct HfLook {
    sec: i32,
    lo: vec2<f32>,
    size: f32,
};

// The finest ready section under q, and the square over which that
// answer holds: down the quadtree from the root grid (fdata[0]: its
// corner and side; fdata[1].xy: its cells), keeping the deepest ready
// section passed; the square is the empty quadrant the descent stopped
// at, or the last node's own where it has no children. Size 0: off the
// ground.
fn hf_lookup(q: vec2<f32>) -> HfLook {
    var out: HfLook;
    out.sec = -1;
    out.size = 0.0;
    let ro = params.fdata[0].xy;
    let rs = params.fdata[0].z;
    let nx = i32(params.fdata[1].x);
    let ny = i32(params.fdata[1].y);
    let g = vec2<i32>(floor((q - ro) / rs));
    if (g.x < 0 || g.y < 0 || g.x >= nx || g.y >= ny) {
        return out;
    }
    var node = g.y * nx + g.x;
    var lo = ro + vec2<f32>(g) * rs;
    var size = rs;
    for (var depth = 0; depth < 48; depth = depth + 1) {
        let nd = hf_nodes[node];
        if (nd.sec.x >= 0) {
            out.sec = nd.sec.x;
        }
        let half = size * 0.5;
        let qx = select(0, 1, q.x >= lo.x + half);
        let qy = select(0, 1, q.y >= lo.y + half);
        let child = nd.child[qy * 2 + qx];
        if (child < 0) {
            if (max(max(nd.child.x, nd.child.y), max(nd.child.z, nd.child.w)) >= 0) {
                lo = lo + vec2<f32>(f32(qx), f32(qy)) * half;
                size = half;
            }
            break;
        }
        node = child;
        lo = lo + vec2<f32>(f32(qx), f32(qy)) * half;
        size = half;
    }
    out.lo = lo;
    out.size = size;
    return out;
}
"#;

/// The walk's WGSL, assembled.
pub fn assemble_walk() -> String {
    format!(
        "{}\n{}",
        WALK_COMMON.replace("//__IFS_RIG__", &super::assembler::ifs_rig_jittered()),
        WALK_WGSL.replace(
            "//__HF_HELPERS__",
            &HF_HELPERS_WGSL
                .replace("__HF_MAX_STEPS__", &HF_MAX_STEPS.to_string())
                .replace("//__HF_GROUND__", GROUND_WGSL)
        )
    )
}

/// The relight's WGSL, assembled.
pub fn assemble_relight() -> String {
    format!(
        "{}\n{}",
        WALK_COMMON.replace("//__IFS_RIG__", &super::assembler::ifs_rig_jittered()),
        RELIGHT_WGSL.replace("//__HF_GROUND__", GROUND_WGSL).replace("//__HF_ALBEDO__", HF_ALBEDO_WGSL)
    )
}

/// The path tracer's WGSL, assembled: the rig's accessors and camera, the
/// shared core (`path_core`), the ground's functions, the albedo's
/// lookup, and the height field's side of the integrator.
pub fn assemble_path() -> String {
    format!(
        "{}\n{}\n{}",
        WALK_COMMON.replace("//__IFS_RIG__", &super::assembler::ifs_rig_jittered()),
        super::path_core::PT_CORE_WGSL.replace("//__LENS_APPLY_RAY__", ""),
        PATH_WGSL
            .replace(
                "//__HF_HELPERS__",
                &HF_HELPERS_WGSL
                    .replace("__HF_MAX_STEPS__", &HF_MAX_STEPS.to_string())
                    .replace("//__HF_GROUND__", GROUND_WGSL)
            )
            .replace("//__HF_ALBEDO__", HF_ALBEDO_WGSL)
    )
}

/// The mipmap build's WGSL.
pub fn assemble_build() -> String {
    BUILD_WGSL.to_string()
}

/// How a terrain is seen and lit: the camera, in the tile's CELL units,
/// and the solid lighting, with what the terrain adds to it.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainView {
    pub camera: SolidCamera,
    pub shading: SolidShadingSettings,
    /// Fog (density per unit of view depth, its start, and a colour the
    /// rig no longer uses): the relight fades the ground's coverage, so
    /// the tonemap's own background shows through.
    pub fog: (f32, f32, [f32; 3]),
    /// Shadow strength: 0 traces no shadow rays.
    pub shadow: f32,
    /// Penumbra softness: how close a shadow ray may pass the surface,
    /// relative to its distance, before it darkens. Larger is harder.
    pub softness: f32,
    /// Occlusion reach, in cells.
    pub occlusion_reach: f32,
    /// The rays' offset within their pixel, in pixels ([-0.5, 0.5]): one
    /// sample of an accumulated render.
    pub jitter: [f32; 2],
    /// The terrain's height H and, for a distance, the flanks' width w,
    /// in cells: the walk's map from the tile's raw source to heights.
    /// Unused by a tile set from heights.
    pub height: f32,
    pub width: f32,
    /// Rays per pixel along each axis, the accumulation's grid: a ray's
    /// share of a pixel, which the albedo's filter covers.
    pub samples_per_axis: u32,
    /// The view depth past which there is no ground, in the world's
    /// units (the fog has reached the background there).
    pub far: f32,
}

pub use super::path_core::PathSettings;
use super::path_core::{PathParamsGpu, PathSum};

/// The default sun when the lighting panel is untouched (plan H7): the
/// 2D relief's own light, azimuth 135 (upper left) at elevation 30.
pub const DEFAULT_SUN: (f32, f32) = (135.0, 30.0);

/// A world-fixed light's direction: azimuth counter-clockwise from east
/// (+x) on the plane, elevation above it, +z up.
pub fn light_direction(azimuth_deg: f32, elevation_deg: f32) -> [f32; 3] {
    let (az, el) = ((azimuth_deg as f64).to_radians(), (elevation_deg as f64).to_radians());
    [(el.cos() * az.cos()) as f32, (el.cos() * az.sin()) as f32, el.sin() as f32]
}

/// The rig's slots of `fdata` for a terrain: mode D's material, fog and
/// lights, the lights world-fixed. An untouched panel means a default
/// sun, as mode D's untouched panel means a default key light.
fn pack_rig(cam: &SolidCamera, shading: &SolidShadingSettings, fog: (f32, f32, [f32; 3]), out: &mut [[f32; 4]; 20]) {
    out[2][3] = cam.fov;
    out[3] = [cam.forward[0] as f32, cam.forward[1] as f32, cam.forward[2] as f32, 0.0];
    out[4] = [cam.right[0] as f32, cam.right[1] as f32, cam.right[2] as f32, 0.0];
    out[5] = [cam.up[0] as f32, cam.up[1] as f32, cam.up[2] as f32, 0.0];
    let any = !SolidShadingSettings::is_default(shading);
    let (strength, ambient, diffuse, specular, shininess, ssao) = if any {
        (
            shading.shading_strength,
            shading.ambient,
            shading.diffuse,
            shading.specular,
            shading.shininess,
            shading.ssao_strength,
        )
    } else {
        (1.0, 0.12, 0.88, 0.0, 32.0, 1.0)
    };
    out[8] = [strength, ambient, diffuse, specular];
    out[9] = [shininess, ssao, fog.0, fog.1];
    out[10] = [fog.2[0], fog.2[1], fog.2[2], 0.0];
    let mut n = 0usize;
    for l in shading.lights.iter() {
        if any && (!l.enabled || l.intensity <= 0.0) {
            continue;
        }
        let (az, el) = if any { (l.azimuth, l.elevation) } else { DEFAULT_SUN };
        let dir = light_direction(az, el);
        let (colour, intensity) = if any { (l.color, l.intensity.max(0.0)) } else { ([1.0; 3], 1.0) };
        out[11 + n * 2] = [dir[0], dir[1], dir[2], intensity];
        out[12 + n * 2] = [colour[0], colour[1], colour[2], 0.0];
        n += 1;
        if n == 4 || !any {
            break;
        }
    }
    out[7][2] = n as f32;
}

/// What the walk reads, as a key: a change to anything else is a
/// relight. The light DIRECTIONS and whether each is on decide which
/// shadow rays are traced; their colour and power, the material and
/// the fog do not.
fn walk_key(view: &TerrainView, w: u32, h: u32, tile: u64) -> String {
    let c = &view.camera;
    let mut k = format!(
        "{w}x{h}|{tile}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{}|{}",
        c.eye,
        c.forward,
        c.right,
        c.up,
        c.fov,
        view.shadow > 0.0,
        view.softness,
        view.occlusion_reach,
        view.jitter,
        view.height,
        view.width
    );
    k.push_str(&format!("|{}", view.far));
    let any = !SolidShadingSettings::is_default(&view.shading);
    k.push_str(&format!("|{any}"));
    for l in &view.shading.lights {
        k.push_str(&format!("|{}:{}:{}", l.enabled && l.intensity > 0.0, l.azimuth, l.elevation));
    }
    k
}

/// The sections' storage: texture arrays of one layer a section -- the
/// raw samples, the albedo with its mip chain, the maximum mipmap at
/// power-of-two sides.
struct Atlas {
    /// A layer's samples.
    w: u32,
    h: u32,
    capacity: u32,
    /// The maximum mipmap's levels.
    levels: u32,
    raw: Texture,
    albedo: Texture,
    mips: Texture,
    raw_all: TextureView,
    albedo_all: TextureView,
    mips_all: TextureView,
}

/// A section of the ground: an atlas layer of `n x m` samples whose
/// sample (0, 0) sits at `origin` in the world, `texel` apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundSection {
    pub layer: u32,
    pub origin: [f64; 2],
    pub texel: f64,
    pub n: u32,
    pub m: u32,
}

/// A quadtree node of the ground's index: its children (south-west,
/// south-east, north-west, north-east; -1 none) and the section that
/// covers its square (-1: none ready).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundNode {
    pub children: [i32; 4],
    pub section: i32,
}

/// The ground: its sections and the quadtree that finds the finest
/// ready one under a point. The roots are a grid of `root_dims` squares
/// of `root_side` from `root_origin`, nodes `0..nx*ny` row by row.
/// Heights run from `floor` to `top` in the world; `mode` is the walk's
/// map from the sections' raw samples (`hf_f`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ground {
    pub root_origin: [f64; 2],
    pub root_side: f64,
    pub root_dims: [u32; 2],
    pub nodes: Vec<GroundNode>,
    pub sections: Vec<GroundSection>,
    pub top: f64,
    pub floor: f64,
    pub mode: u32,
}

/// The ground on the GPU, and its version: a new one for every set.
struct GroundGpu {
    ground: Ground,
    sections: Buffer,
    nodes: Buffer,
    version: u64,
}

/// The terrain renderer: the ground's sections in an atlas, their
/// mipmaps, and the walk and relight over them. Part of the escape
/// engine, beside mode D, whose camera and rig it shares.
pub struct TerrainRenderer {
    build_layout: BindGroupLayout,
    walk_layout: BindGroupLayout,
    relight_layout: BindGroupLayout,
    build_pipeline: ComputePipeline,
    walk_pipeline: ComputePipeline,
    relight_pipeline: ComputePipeline,
    ingest_layout: BindGroupLayout,
    range_pipeline: ComputePipeline,
    ingest_pipeline: ComputePipeline,
    accum_layout: BindGroupLayout,
    accum_pipeline: ComputePipeline,
    albedo_mip_layout: BindGroupLayout,
    albedo_mip_pipeline: ComputePipeline,
    path_layout: BindGroupLayout,
    path_pipeline: ComputePipeline,
    /// The path tracer's per-pixel sum, and the samples in it.
    path: PathSum,
    sampler: Sampler,
    atlas: Option<Atlas>,
    ground: Option<GroundGpu>,
    /// The layer being made from footprint regions, and its ingest.
    building: Option<(u32, IngestParamsGpu)>,
    /// A count's or a relief's range over every section ingested since
    /// the last `reset_range`, in the ordered encoding (`hf_range`).
    range: Buffer,
    /// The accumulation's pair, allocated on the first `accumulate`;
    /// `accum_front` is the one holding the mean.
    accum: Option<[(Texture, TextureView); 2]>,
    accum_front: usize,
    /// Samples in the mean; 0 starts a new one.
    accum_count: u32,
    params: Buffer,
    stats: Buffer,
    geom: Buffer,
    geom_px: u32,
    output: (Texture, TextureView),
    out_w: u32,
    out_h: u32,
    next_version: u64,
    walked: Option<String>,
    /// Walks run so far, for the relight-cache gate.
    pub walks: u32,
    /// Count traversal steps into the stats buffer.
    pub count_steps: bool,
}

impl TerrainRenderer {
    pub fn new(device: &Device, out_w: u32, out_h: u32) -> Self {
        let tex = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let uniform = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let storage = |binding: u32, read_only: bool| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_tex = |binding: u32, format: TextureFormat| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        };
        let build_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Build"),
            entries: &[uniform(0), tex(1), storage_tex(2, TextureFormat::R32Float)],
        });
        let tex_array = |binding: u32, filterable: bool| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable },
                view_dimension: TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        };
        let walk_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Walk"),
            entries: &[
                uniform(0),
                tex_array(1, false),
                tex_array(2, false),
                storage(3, false),
                storage(4, false),
                storage(5, true),
                storage(6, true),
                storage(7, true),
            ],
        });
        let relight_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Relight"),
            entries: &[
                uniform(0),
                tex_array(1, true),
                storage(2, true),
                storage_tex(3, TextureFormat::Rgba32Float),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                storage(6, true),
                storage(7, true),
            ],
        });
        let albedo_mip_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Albedo Mips"),
            entries: &[tex(0), storage_tex(1, TextureFormat::Rgba16Float)],
        });
        let path_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Path"),
            entries: &[
                uniform(0),
                tex_array(1, false),
                tex_array(2, false),
                tex_array(3, true),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                storage(5, true),
                storage(6, true),
                storage(7, true),
                storage(8, false),
                uniform(9),
            ],
        });
        let ingest_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Ingest"),
            entries: &[
                uniform(0),
                tex(1),
                tex(2),
                storage(3, false),
                storage_tex(4, TextureFormat::R32Float),
                storage_tex(5, TextureFormat::Rgba16Float),
            ],
        });
        let accum_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Accumulate"),
            entries: &[uniform(0), tex(1), tex(2), storage_tex(3, TextureFormat::Rgba32Float)],
        });
        let entry = |label: &str, layout: &BindGroupLayout, src: &str, entry: &str| {
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(label),
                source: ShaderSource::Wgsl(src.into()),
            });
            let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = |label: &str, layout: &BindGroupLayout, src: String| entry(label, layout, &src, "main");
        let build_pipeline = pipeline("Terrain Build", &build_layout, assemble_build());
        let walk_pipeline = pipeline("Terrain Walk", &walk_layout, assemble_walk());
        let relight_pipeline = pipeline("Terrain Relight", &relight_layout, assemble_relight());
        let range_pipeline = entry("Terrain Range", &ingest_layout, INGEST_WGSL, "range_main");
        let ingest_pipeline = entry("Terrain Ingest", &ingest_layout, INGEST_WGSL, "ingest_main");
        let accum_pipeline = entry("Terrain Accumulate", &accum_layout, ACCUM_WGSL, "main");
        let albedo_mip_pipeline = entry("Terrain Albedo Mips", &albedo_mip_layout, ALBEDO_MIP_WGSL, "main");
        let path_pipeline = pipeline("Terrain Path", &path_layout, assemble_path());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Terrain Albedo"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        });
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Params"),
            size: std::mem::size_of::<TerrainParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stats = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Stats"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let geom = Self::create_geom(device, out_w * out_h);
        let path = PathSum::new(device, out_w * out_h);
        let output = Self::create_output(device, out_w, out_h);
        let range = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Range"),
            size: 8,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        TerrainRenderer {
            build_layout,
            walk_layout,
            relight_layout,
            build_pipeline,
            walk_pipeline,
            relight_pipeline,
            ingest_layout,
            range_pipeline,
            ingest_pipeline,
            accum_layout,
            accum_pipeline,
            albedo_mip_layout,
            albedo_mip_pipeline,
            path_layout,
            path_pipeline,
            path,
            sampler,
            atlas: None,
            ground: None,
            building: None,
            range,
            accum: None,
            accum_front: 0,
            accum_count: 0,
            params,
            stats,
            geom,
            geom_px: out_w * out_h,
            output,
            out_w,
            out_h,
            next_version: 1,
            walked: None,
            walks: 0,
            count_steps: false,
        }
    }

    fn create_geom(device: &Device, px: u32) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Geometry"),
            size: (px.max(1) as u64) * 16,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn create_output(device: &Device, w: u32, h: u32) -> (Texture, TextureView) {
        let t = device.create_texture(&TextureDescriptor {
            label: Some("Terrain Output"),
            size: Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let v = t.create_view(&TextureViewDescriptor::default());
        (t, v)
    }

    /// Resize the output; the next render walks.
    pub fn resize(&mut self, device: &Device, out_w: u32, out_h: u32) {
        if (out_w, out_h) == (self.out_w, self.out_h) {
            return;
        }
        self.output = Self::create_output(device, out_w, out_h);
        self.accum = None;
        self.accum_count = 0;
        if out_w * out_h > self.geom_px {
            self.geom = Self::create_geom(device, out_w * out_h);
            self.geom_px = out_w * out_h;
        }
        self.path.ensure(device, out_w * out_h);
        self.out_w = out_w;
        self.out_h = out_h;
        self.walked = None;
    }

    pub fn output_view(&self) -> &TextureView {
        &self.output.1
    }

    pub fn output_texture(&self) -> &Texture {
        &self.output.0
    }

    /// Bytes a pixel of the output costs: the geometry record, the
    /// render and the accumulation's pair.
    pub const BYTES_PER_PIXEL: u64 = 16 + 3 * 16;

    /// Bytes a section's sample costs, about: the raw sample, the albedo
    /// and its mips, the mipmap at power-of-two sides.
    pub const BYTES_PER_SAMPLE: u64 = 4 + 11 + 6;

    /// Why a `w x h` terrain view cannot be held on this device: the
    /// geometry record is one storage binding, and the textures have a
    /// side limit.
    pub fn allocation_error(device: &Device, w: u32, h: u32) -> Option<String> {
        let lim = device.limits();
        let side = lim.max_texture_dimension_2d;
        if w > side || h > side {
            return Some(format!("a terrain view of {w}x{h} is past this device's texture side of {side}"));
        }
        let geom = w as u64 * h as u64 * 16;
        let cap = lim.max_buffer_size.min(lim.max_storage_buffer_binding_size as u64);
        if geom > cap {
            return Some(format!(
                "a terrain view of {w}x{h} needs a {} MB geometry record, past this device's {} MB binding",
                geom >> 20,
                cap >> 20
            ));
        }
        None
    }

    /// Free the GPU memory now: dropping frees nothing on WebGPU.
    pub fn destroy(&self) {
        self.params.destroy();
        self.stats.destroy();
        self.geom.destroy();
        self.path.destroy();
        self.output.0.destroy();
        if let Some(pair) = &self.accum {
            pair[0].0.destroy();
            pair[1].0.destroy();
        }
        self.range.destroy();
        if let Some(a) = &self.atlas {
            a.raw.destroy();
            a.albedo.destroy();
            a.mips.destroy();
        }
        if let Some(g) = &self.ground {
            g.sections.destroy();
            g.nodes.destroy();
        }
    }

    /// Start a new accumulation: the next `accumulate` replaces the mean.
    pub fn reset_accumulation(&mut self) {
        self.accum_count = 0;
    }

    /// The ground's version: a new one for every ground set, and 0
    /// before any.
    pub fn tile_version(&self) -> u64 {
        self.ground.as_ref().map_or(0, |g| g.version)
    }

    /// Samples in the accumulation so far.
    pub fn accumulated_samples(&self) -> u32 {
        self.accum_count
    }

    /// Fold the last render into the accumulation. Submits its own work.
    pub fn accumulate(&mut self, device: &Device, queue: &Queue) {
        let (w, h) = (self.out_w, self.out_h);
        let pair = self.accum.get_or_insert_with(|| [Self::create_output(device, w, h), Self::create_output(device, w, h)]);
        let (prev, next) = (self.accum_front, 1 - self.accum_front);
        let buf = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Accumulate Params"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytemuck::cast_slice(&[w, h, self.accum_count, 0u32]));
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Terrain Accumulate"),
            layout: &self.accum_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&self.output.1) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&pair[prev].1) },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&pair[next].1) },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Accumulate") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Accumulate"), timestamp_writes: None });
            pass.set_pipeline(&self.accum_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        queue.submit(std::iter::once(enc.finish()));
        self.accum_front = next;
        self.accum_count += 1;
    }

    /// The accumulated mean, once anything has been folded in.
    pub fn accumulated_view(&self) -> Option<&TextureView> {
        match (&self.accum, self.accum_count) {
            (Some(pair), 1..) => Some(&pair[self.accum_front].1),
            _ => None,
        }
    }

    /// The accumulated mean's texture, for a test to read back.
    #[cfg(test)]
    pub(crate) fn accumulated_texture_for_test(&self) -> Option<&Texture> {
        match (&self.accum, self.accum_count) {
            (Some(pair), 1..) => Some(&pair[self.accum_front].0),
            _ => None,
        }
    }

    /// Levels of the mipmap for an `n x m` tile: level 0 is its cells,
    /// each level above halves them (rounding up), to one node.
    pub fn levels_for(n: u32, m: u32) -> u32 {
        let (mut w, mut h) = (n.saturating_sub(1).max(1), m.saturating_sub(1).max(1));
        let mut levels = 1;
        while w > 1 || h > 1 {
            w = w.div_ceil(2);
            h = h.div_ceil(2);
            levels += 1;
        }
        levels
    }

    /// Upload a tile: `n x m` heights in cell units, row-major from
    /// `y = 0` northward, and an albedo per sample (linear rgb, alpha
    /// as coverage). Builds the mipmap.
    pub fn set_tile(&mut self, device: &Device, queue: &Queue, n: u32, m: u32, heights: &[f32], albedo: &[[f32; 4]]) {
        assert!(n >= 2 && m >= 2, "a tile needs at least 2x2 samples");
        assert_eq!(heights.len(), (n * m) as usize);
        assert_eq!(albedo.len(), (n * m) as usize);
        self.ensure_atlas(device, n, m, 1);
        let atlas = self.atlas.as_ref().expect("made above");
        let at = Origin3d { x: 0, y: 0, z: 0 };
        queue.write_texture(
            TexelCopyTextureInfo { texture: &atlas.raw, mip_level: 0, origin: at, aspect: TextureAspect::All },
            bytemuck::cast_slice(heights),
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(m) },
            Extent3d { width: n, height: m, depth_or_array_layers: 1 },
        );
        let halves: Vec<u16> = albedo.iter().flatten().map(|v| half::f16::from_f32(*v).to_bits()).collect();
        queue.write_texture(
            TexelCopyTextureInfo { texture: &atlas.albedo, mip_level: 0, origin: at, aspect: TextureAspect::All },
            bytemuck::cast_slice(&halves),
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 8), rows_per_image: Some(m) },
            Extent3d { width: n, height: m, depth_or_array_layers: 1 },
        );
        self.build_layer(device, queue, 0, 0);
        // A lake's sentinel stands at the plain's level, 0.
        let heights: Vec<f32> = heights.iter().map(|&h| if h.abs() >= 2.0e30 { 0.0 } else { h }).collect();
        let top = heights.iter().cloned().fold(f32::NEG_INFINITY, f32::max) as f64;
        self.set_ground(device, queue, Self::one_section_ground(n, m, 0, top, slab_floor(&heights, n, m) as f64));
    }

    /// A ground of one section of `n x m` cells from the origin, its own
    /// root.
    fn one_section_ground(n: u32, m: u32, mode: u32, top: f64, floor: f64) -> Ground {
        Ground {
            root_origin: [0.0, 0.0],
            root_side: (n.max(m) - 1) as f64,
            root_dims: [1, 1],
            nodes: vec![GroundNode { children: [-1; 4], section: 0 }],
            sections: vec![GroundSection { layer: 0, origin: [0.0, 0.0], texel: 1.0, n, m }],
            top,
            floor,
            mode,
        }
    }

    /// A one-section ground from one footprint render (heightfield plan,
    /// section 5): its colour and its height field, both `n x m` and
    /// `Rgba32Float`, on the GPU throughout. Cells are its pixels; the
    /// floor a hundredth of its side below zero, the top `top`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_tile_from_escape(
        &mut self,
        device: &Device,
        queue: &Queue,
        colour: &TextureView,
        height_field: &TextureView,
        n: u32,
        m: u32,
        ingest: &TerrainIngest,
        top: f64,
    ) {
        self.ensure_atlas(device, n, m, 1);
        self.reset_range(queue);
        self.begin_section(0, n, m, ingest);
        self.ingest_region(device, queue, colour, height_field, 0, 0, n, m);
        self.finish_section(device, queue);
        let floor = -0.01 * n.max(m) as f64;
        self.set_ground(device, queue, Self::one_section_ground(n, m, ingest.mode(), top, floor));
    }

    /// The atlas for `capacity` layers of `w x h` samples; true when it
    /// was (re)made, and every layer is empty.
    pub fn ensure_atlas(&mut self, device: &Device, w: u32, h: u32, capacity: u32) -> bool {
        if self.atlas.as_ref().is_some_and(|a| a.w == w && a.h == h && a.capacity == capacity) {
            return false;
        }
        if let Some(a) = self.atlas.take() {
            a.raw.destroy();
            a.albedo.destroy();
            a.mips.destroy();
        }
        let make = |label: &str, format: TextureFormat, mips: u32, tw: u32, th: u32| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d { width: tw, height: th, depth_or_array_layers: capacity },
                mip_level_count: mips,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let levels = Self::levels_for(w, h);
        let raw = make("Terrain Raw", TextureFormat::R32Float, 1, w, h);
        let albedo = make("Terrain Albedo", TextureFormat::Rgba16Float, 32 - w.max(h).leading_zeros(), w, h);
        // Power-of-two sides: a texture's mips halve rounding DOWN, the
        // node grid halves rounding UP, and only at powers of two do the
        // two agree -- otherwise the top levels' last nodes fall outside
        // their mip, the build's writes are dropped and the walk reads
        // them as zero. The texels past the cells are never read.
        let mips = make(
            "Terrain Max Mipmap",
            TextureFormat::R32Float,
            levels,
            (w - 1).next_power_of_two(),
            (h - 1).next_power_of_two(),
        );
        let array = |t: &Texture| t.create_view(&TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..Default::default() });
        self.atlas = Some(Atlas {
            w,
            h,
            capacity,
            levels,
            raw_all: array(&raw),
            albedo_all: array(&albedo),
            mips_all: array(&mips),
            raw,
            albedo,
            mips,
        });
        self.ground = None;
        self.walked = None;
        true
    }

    /// The atlas's layer side and capacity, once made.
    pub fn atlas_shape(&self) -> Option<(u32, u32, u32)> {
        self.atlas.as_ref().map(|a| (a.w, a.h, a.capacity))
    }

    /// Empty the count's and the relief's range: a new picture.
    pub fn reset_range(&mut self, queue: &Queue) {
        queue.write_buffer(&self.range, 0, bytemuck::cast_slice(&[u32::MAX, 0u32]));
    }

    /// Start making atlas layer `layer` -- `n x m` samples -- from
    /// footprint regions.
    pub fn begin_section(&mut self, layer: u32, n: u32, m: u32, ingest: &TerrainIngest) {
        let atlas = self.atlas.as_ref().expect("ensure_atlas first");
        assert!(layer < atlas.capacity && n <= atlas.w && m <= atlas.h, "a section inside the atlas");
        let params = IngestParamsGpu {
            n,
            m,
            source: ingest.source,
            interior: ingest.interior,
            ox: 0,
            oy: 0,
            rw: 0,
            rh: 0,
            background: [ingest.background[0], ingest.background[1], ingest.background[2], 1.0],
            tint: [ingest.tint[0], ingest.tint[1], ingest.tint[2], 1.0],
        };
        self.building = Some((layer, params));
    }

    /// One region of the footprint into the section being made: its
    /// colour and height field, `rw x rh`, whose top-left is pixel
    /// `(ox, oy)` of the section's picture. Submits its own work.
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_region(
        &mut self,
        device: &Device,
        queue: &Queue,
        colour: &TextureView,
        height_field: &TextureView,
        ox: u32,
        oy: u32,
        rw: u32,
        rh: u32,
    ) {
        let (layer, base) = self.building.expect("begin_section first");
        assert!(ox + rw <= base.n && oy + rh <= base.m, "a region inside the section");
        let atlas = self.atlas.as_ref().expect("ensure_atlas first");
        let p = IngestParamsGpu { ox, oy, rw, rh, ..base };
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Ingest Params"),
            size: std::mem::size_of::<IngestParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&params, 0, bytemuck::bytes_of(&p));
        let one = |t: &Texture| {
            t.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let (hv, av) = (one(&atlas.raw), one(&atlas.albedo));
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Terrain Ingest"),
            layout: &self.ingest_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(colour) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(height_field) },
                BindGroupEntry { binding: 3, resource: self.range.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&hv) },
                BindGroupEntry { binding: 5, resource: BindingResource::TextureView(&av) },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Ingest") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Ingest"), timestamp_writes: None });
            pass.set_bind_group(0, &bg, &[]);
            pass.set_pipeline(&self.range_pipeline);
            pass.dispatch_workgroups(rw.div_ceil(8), rh.div_ceil(8), 1);
            pass.set_pipeline(&self.ingest_pipeline);
            pass.dispatch_workgroups(rw.div_ceil(8), rh.div_ceil(8), 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// Finish the section being made: its maximum mipmap and its albedo's
    /// mip chain. It is drawn once a ground names its layer.
    pub fn finish_section(&mut self, device: &Device, queue: &Queue) {
        let (layer, base) = self.building.take().expect("begin_section first");
        let mode = TerrainIngest { source: base.source, interior: base.interior, background: [0.0; 3], tint: [0.0; 3] }.mode();
        self.build_layer(device, queue, layer, u32::from(mode == 1));
    }

    /// Whether a section is being made.
    pub fn building(&self) -> bool {
        self.building.is_some()
    }

    /// A layer's maximum mipmap (the raw maximum, or for `min_mode` the
    /// minimum) and its albedo's mip chain.
    fn build_layer(&mut self, device: &Device, queue: &Queue, layer: u32, min_mode: u32) {
        let atlas = self.atlas.as_ref().expect("ensure_atlas first");
        let raw = atlas.raw.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        });
        self.build_mips(device, queue, &raw, &atlas.mips, layer, atlas.w, atlas.h, atlas.levels, min_mode);
        self.build_albedo_mips(device, queue, &atlas.albedo, layer);
        self.walked = None;
    }

    /// Draw this ground from now on: its sections (atlas layers already
    /// made) and its index.
    pub fn set_ground(&mut self, device: &Device, queue: &Queue, ground: Ground) {
        let atlas = self.atlas.as_ref().expect("ensure_atlas first");
        let mut sections: Vec<[u32; 8]> = ground
            .sections
            .iter()
            .map(|s| {
                let geo = [s.origin[0] as f32, s.origin[1] as f32, s.texel as f32, (1.0 / s.texel) as f32];
                [
                    geo[0].to_bits(),
                    geo[1].to_bits(),
                    geo[2].to_bits(),
                    geo[3].to_bits(),
                    s.n,
                    s.m,
                    s.layer,
                    atlas.levels,
                ]
            })
            .collect();
        if sections.is_empty() {
            sections.push([0; 8]);
        }
        let mut nodes: Vec<[i32; 8]> = ground
            .nodes
            .iter()
            .map(|n| [n.children[0], n.children[1], n.children[2], n.children[3], n.section, 0, 0, 0])
            .collect();
        if nodes.is_empty() {
            nodes.push([-1, -1, -1, -1, -1, 0, 0, 0]);
        }
        let upload = |label: &str, bytes: &[u8], old: Option<&Buffer>| -> Buffer {
            match old {
                Some(b) if b.size() >= bytes.len() as u64 => {
                    queue.write_buffer(b, 0, bytes);
                    b.clone()
                }
                _ => {
                    let b = device.create_buffer(&BufferDescriptor {
                        label: Some(label),
                        size: (bytes.len() as u64).next_power_of_two().max(64),
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    queue.write_buffer(&b, 0, bytes);
                    b
                }
            }
        };
        let old = self.ground.take();
        let sections_buf = upload("Terrain Sections", bytemuck::cast_slice(&sections), old.as_ref().map(|g| &g.sections));
        let nodes_buf = upload("Terrain Nodes", bytemuck::cast_slice(&nodes), old.as_ref().map(|g| &g.nodes));
        self.ground = Some(GroundGpu { ground, sections: sections_buf, nodes: nodes_buf, version: self.next_version });
        self.next_version += 1;
        self.walked = None;
    }

    /// A layer's albedo mip chain: each level the box average of the one
    /// below.
    fn build_albedo_mips(&self, device: &Device, queue: &Queue, albedo: &Texture, layer: u32) {
        let levels = albedo.mip_level_count();
        let level_view = |l: u32| {
            albedo.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                base_mip_level: l,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Albedo Mips") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Albedo Mips"), timestamp_writes: None });
            pass.set_pipeline(&self.albedo_mip_pipeline);
            for l in 1..levels {
                let (src, dst) = (level_view(l - 1), level_view(l));
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Terrain Albedo Mips"),
                    layout: &self.albedo_mip_layout,
                    entries: &[
                        BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&src) },
                        BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&dst) },
                    ],
                });
                pass.set_bind_group(0, &bg, &[]);
                let w = (albedo.width() >> l).max(1);
                let h = (albedo.height() >> l).max(1);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// Level 0 from the heights, then each level from the one below.
    /// Each dispatch reads one mip as a sampled view and writes the next
    /// as a storage view: distinct subresources of one texture.
    #[allow(clippy::too_many_arguments)]
    fn build_mips(
        &self,
        device: &Device,
        queue: &Queue,
        height: &TextureView,
        mips: &Texture,
        layer: u32,
        n: u32,
        m: u32,
        levels: u32,
        mode: u32,
    ) {
        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let stride = (std::mem::size_of::<BuildParamsGpu>() as u64).div_ceil(align) * align;
        let mut sizes = vec![(n - 1, m - 1)];
        for _ in 1..levels {
            let (w, h) = *sizes.last().unwrap();
            sizes.push((w.div_ceil(2), h.div_ceil(2)));
        }
        let mut bytes = vec![0u8; (stride * levels as u64) as usize];
        for l in 0..levels as usize {
            let (src_w, src_h) = if l == 0 { (n, m) } else { sizes[l - 1] };
            let p = BuildParamsGpu {
                dst_w: sizes[l].0,
                dst_h: sizes[l].1,
                src_w,
                src_h,
                step: if l == 0 { 1 } else { 2 },
                mode,
                pad: [0; 2],
            };
            let at = l * stride as usize;
            bytes[at..at + std::mem::size_of::<BuildParamsGpu>()].copy_from_slice(bytemuck::bytes_of(&p));
        }
        let buf = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Build Params"),
            size: bytes.len() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, &bytes);
        let level_view = |l: u32| {
            mips.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                base_mip_level: l,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Build") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Build"), timestamp_writes: None });
            pass.set_pipeline(&self.build_pipeline);
            for l in 0..levels {
                let src = if l == 0 { height.clone() } else { level_view(l - 1) };
                let dst = level_view(l);
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Terrain Build"),
                    layout: &self.build_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: &buf,
                                offset: l as u64 * stride,
                                size: std::num::NonZeroU64::new(std::mem::size_of::<BuildParamsGpu>() as u64),
                            }),
                        },
                        BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&src) },
                        BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&dst) },
                    ],
                });
                pass.set_bind_group(0, &bg, &[]);
                let (w, h) = sizes[l as usize];
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    fn write_params(&self, queue: &Queue, view: &TerrainView) {
        let g = &self.ground.as_ref().expect("a ground").ground;
        let mut fdata = [[0.0f32; 4]; 20];
        // The terrain's fog fades its COVERAGE (the relight), so the
        // tonemap composites the background itself: a colour fog mixed
        // in linear light could never match a background composited
        // after the tonemap's gamma. The rig gets none.
        pack_rig(&view.camera, &view.shading, (0.0, 0.0, view.fog.2), &mut fdata);
        fdata[7][0] = view.fog.0;
        fdata[7][1] = view.fog.1;
        // The ground's index, its top, and its map.
        fdata[0] = [g.root_origin[0] as f32, g.root_origin[1] as f32, g.root_side as f32, g.top as f32];
        fdata[1] = [g.root_dims[0] as f32, g.root_dims[1] as f32, g.sections.len() as f32, g.mode as f32];
        // The map's height and width, a ray's share of a pixel per unit
        // of distance (the albedo's filter), and the view depth past
        // which there is no ground.
        let per_ray = 2.0 * (view.camera.fov * 0.5).tan() / self.out_h.max(1) as f32 / view.samples_per_axis.max(1) as f32;
        fdata[6] = [view.height, view.width, per_ray, view.far];
        let p = TerrainParamsGpu {
            width: self.out_w,
            height: self.out_h,
            grid_n: 0,
            grid_m: 0,
            levels: 0,
            flags: u32::from(self.count_steps),
            jitter: view.jitter,
            eye: [view.camera.eye[0] as f32, view.camera.eye[1] as f32, view.camera.eye[2] as f32, g.floor as f32],
            misc: [view.shadow, view.softness, view.occlusion_reach, 0.0],
            fdata,
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&p));
    }

    /// Render the view: walk if anything the walk reads changed, then
    /// relight. Submits its own work.
    pub fn render(&mut self, device: &Device, queue: &Queue, view: &TerrainView) {
        let (Some(ground), Some(atlas)) = (self.ground.as_ref(), self.atlas.as_ref()) else { return };
        let key = walk_key(view, self.out_w, self.out_h, ground.version);
        let walk = self.walked.as_deref() != Some(key.as_str()) || self.count_steps;
        self.write_params(queue, view);
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain") });
        let (gx, gy) = (self.out_w.div_ceil(8), self.out_h.div_ceil(8));
        if walk {
            if self.count_steps {
                queue.write_buffer(&self.stats, 0, &[0u8; 16]);
            }
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Terrain Walk"),
                layout: &self.walk_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&atlas.raw_all) },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&atlas.mips_all) },
                    BindGroupEntry { binding: 3, resource: self.geom.as_entire_binding() },
                    BindGroupEntry { binding: 4, resource: self.stats.as_entire_binding() },
                    BindGroupEntry { binding: 5, resource: self.range.as_entire_binding() },
                    BindGroupEntry { binding: 6, resource: ground.sections.as_entire_binding() },
                    BindGroupEntry { binding: 7, resource: ground.nodes.as_entire_binding() },
                ],
            });
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Walk"), timestamp_writes: None });
            pass.set_pipeline(&self.walk_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        {
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Terrain Relight"),
                layout: &self.relight_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&atlas.albedo_all) },
                    BindGroupEntry { binding: 2, resource: self.geom.as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&self.output.1) },
                    BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.sampler) },
                    BindGroupEntry { binding: 6, resource: ground.sections.as_entire_binding() },
                    BindGroupEntry { binding: 7, resource: ground.nodes.as_entire_binding() },
                ],
            });
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Relight"), timestamp_writes: None });
            pass.set_pipeline(&self.relight_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
        if walk {
            self.walks += 1;
            self.walked = Some(key);
        }
    }

    /// Start the path tracer's sum over: the next `render_path` replaces
    /// it.
    pub fn reset_path(&mut self) {
        self.path.reset();
    }

    /// Samples in the path tracer's sum.
    pub fn path_samples(&self) -> u32 {
        self.path.count()
    }

    /// Add `samples` path-traced samples of the view to the sum -- in
    /// dispatches of at most `per_dispatch` -- and resolve the mean into
    /// the output. Submits its own work. The samples are the next ones
    /// in order, so the sum is the same bits however they are split.
    pub fn render_path(
        &mut self,
        device: &Device,
        queue: &Queue,
        view: &TerrainView,
        settings: &PathSettings,
        samples: u32,
        per_dispatch: u32,
    ) {
        let (Some(ground), Some(atlas)) = (self.ground.as_ref(), self.atlas.as_ref()) else { return };
        let _ = ground;
        self.write_params(queue, view);
        let ground = self.ground.as_ref().expect("checked");
        let (gx, gy) = (self.out_w.div_ceil(8), self.out_h.div_ceil(8));
        let radius = super::path_core::light_radius(view.softness);
        // A ray's share of a pixel per unit of distance: half a pixel,
        // since the jitter already spreads the samples over the whole.
        let per_ray = (view.camera.fov * 0.5).tan() / self.out_h.max(1) as f32;
        let mut done = 0;
        while done < samples {
            let n = per_dispatch.max(1).min(samples - done);
            let buf = PathParamsGpu::new(settings, self.path.count(), n, per_ray, view.shadow, radius, (0, self.out_h))
                .buffer(device, queue);
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Terrain Path"),
                layout: &self.path_layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&atlas.raw_all) },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&atlas.mips_all) },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&atlas.albedo_all) },
                    BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.sampler) },
                    BindGroupEntry { binding: 5, resource: self.range.as_entire_binding() },
                    BindGroupEntry { binding: 6, resource: ground.sections.as_entire_binding() },
                    BindGroupEntry { binding: 7, resource: ground.nodes.as_entire_binding() },
                    BindGroupEntry { binding: 8, resource: self.path.buffer().as_entire_binding() },
                    BindGroupEntry { binding: 9, resource: buf.as_entire_binding() },
                ],
            });
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Terrain Path") });
            {
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Terrain Path"), timestamp_writes: None });
                pass.set_pipeline(&self.path_pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
            self.path.add(n);
            done += n;
        }
        // The mean, into the output.
        self.path.resolve(device, queue, &self.output.1, self.out_w, self.out_h, 0);
        // The output no longer holds a lit render.
        self.walked = None;
    }

    /// The path tracer's sum, for a test to read back.
    #[cfg(test)]
    pub(crate) fn path_sum_for_test(&self) -> &Buffer {
        self.path.buffer()
    }

    /// The geometry buffer, for a test to read hits back.
    #[cfg(test)]
    pub(crate) fn geometry_buffer(&self) -> &Buffer {
        &self.geom
    }

    /// The step counters, for a test or a measurement: (primary steps
    /// summed, the most any primary ray took, rays, all steps summed
    /// including shadow rays).
    #[cfg(test)]
    pub(crate) fn stats_buffer(&self) -> &Buffer {
        &self.stats
    }

    /// The atlas's raw and albedo textures (layer 0 first), for a test to
    /// read back.
    #[cfg(test)]
    pub(crate) fn tile_textures_for_test(&self) -> Option<(&Texture, &Texture)> {
        self.atlas.as_ref().map(|a| (&a.raw, &a.albedo))
    }

    /// The first section's side and the ground's map, for a test.
    #[cfg(test)]
    pub(crate) fn tile_shape_for_test(&self) -> Option<(u32, u32, u32)> {
        self.ground.as_ref().and_then(|g| g.ground.sections.first().map(|s| (s.n, s.m, g.ground.mode)))
    }

    /// The atlas's mipmap texture, for a test to read a level back.
    #[cfg(test)]
    pub(crate) fn mips_texture(&self) -> Option<&Texture> {
        self.atlas.as_ref().map(|a| &a.mips)
    }

    /// The ground being drawn, for a test.
    #[cfg(test)]
    pub(crate) fn ground_for_test(&self) -> Option<&Ground> {
        self.ground.as_ref().map(|g| &g.ground)
    }
}

/// The slab's floor: a hundredth of the tile's longer side below its
/// lowest point. The tile is a slab whose sides are walls down to here;
/// a ray that passes under it misses.
pub fn slab_floor(heights: &[f32], n: u32, m: u32) -> f32 {
    let lowest = heights.iter().cloned().fold(f32::INFINITY, f32::min);
    lowest - 0.01 * n.max(m) as f32
}

/// The CPU reference: the first hit of a ray with the tile's bilinear
/// surface, walking EVERY cell the ray crosses (a 2D DDA, no mipmap) and
/// solving each cell's patch in f64. What the GPU traversal is tested
/// against; it shares nothing with it but the definition of the surface.
pub fn trace_reference(heights: &[f32], n: usize, m: usize, o: [f64; 3], d: [f64; 3]) -> Option<f64> {
    let h = |i: usize, j: usize| heights[j * n + i] as f64;
    let (cn, cm) = ((n - 1) as f64, (m - 1) as f64);
    let top = heights.iter().cloned().fold(f32::NEG_INFINITY, f32::max) as f64;
    let floor = slab_floor(heights, n as u32, m as u32) as f64;
    // Clip to the box.
    let mut t0 = 0.0f64;
    let mut t1 = f64::INFINITY;
    for (oc, dc, hi) in [(o[0], d[0], cn), (o[1], d[1], cm)] {
        if dc.abs() < 1e-15 {
            if oc < 0.0 || oc > hi {
                return None;
            }
        } else {
            let (ta, tb) = ((0.0 - oc) / dc, (hi - oc) / dc);
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        }
    }
    if d[2] < 0.0 {
        if o[2] > top {
            t0 = t0.max((top - o[2]) / d[2]);
        }
        t1 = t1.min((floor - o[2]) / d[2]);
    } else {
        if o[2] > top {
            return None;
        }
        if d[2] > 0.0 {
            t1 = t1.min((top - o[2]) / d[2]);
            if o[2] < floor {
                t0 = t0.max((floor - o[2]) / d[2]);
            }
        } else if o[2] < floor {
            return None;
        }
    }
    if t0 > t1 {
        return None;
    }
    // Walk the cells along the ray from t0.
    let p0 = [o[0] + d[0] * t0, o[1] + d[1] * t0];
    let mut a = (p0[0].floor() as i64).clamp(0, n as i64 - 2);
    let mut b = (p0[1].floor() as i64).clamp(0, m as i64 - 2);
    let mut t = t0;
    loop {
        // This cell's exit.
        let tx = if d[0] > 0.0 {
            ((a + 1) as f64 - o[0]) / d[0]
        } else if d[0] < 0.0 {
            (a as f64 - o[0]) / d[0]
        } else {
            f64::INFINITY
        };
        let ty = if d[1] > 0.0 {
            ((b + 1) as f64 - o[1]) / d[1]
        } else if d[1] < 0.0 {
            (b as f64 - o[1]) / d[1]
        } else {
            f64::INFINITY
        };
        let t_far = tx.min(ty).min(t1);
        let (ai, bi) = (a as usize, b as usize);
        let (h00, h10, h01, h11) = (h(ai, bi), h(ai + 1, bi), h(ai, bi + 1), h(ai + 1, bi + 1));
        let q = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
        let (u0, v0) = (q[0] - a as f64, q[1] - b as f64);
        let (bb, cc, dd) = (h10 - h00, h01 - h00, h00 - h10 - h01 + h11);
        let ha = h00 + bb * u0 + cc * v0 + dd * u0 * v0;
        let hb = bb * d[0] + cc * d[1] + dd * (u0 * d[1] + v0 * d[0]);
        let hc = dd * d[0] * d[1];
        let (alpha, beta, gamma) = (q[2] - ha, d[2] - hb, -hc);
        let len = t_far - t;
        if alpha <= 0.0 {
            return Some(t);
        }
        let mut roots: Vec<f64> = Vec::new();
        if gamma.abs() < 1e-14 {
            if beta < 0.0 {
                roots.push(-alpha / beta);
            }
        } else {
            let disc = beta * beta - 4.0 * gamma * alpha;
            if disc >= 0.0 {
                let sq = disc.sqrt();
                let qq = -0.5 * (beta + if beta >= 0.0 { sq } else { -sq });
                if qq != 0.0 {
                    roots.push(qq / gamma);
                    roots.push(alpha / qq);
                }
            }
        }
        if let Some(s) = roots.into_iter().filter(|s| *s >= 0.0 && *s <= len).reduce(f64::min) {
            return Some(t + s);
        }
        if t_far >= t1 {
            return None;
        }
        // Into the neighbour through whichever face the ray left by.
        if tx <= ty {
            a += if d[0] > 0.0 { 1 } else { -1 };
        } else {
            b += if d[1] > 0.0 { 1 } else { -1 };
        }
        if a < 0 || b < 0 || a > n as i64 - 2 || b > m as i64 - 2 {
            return None;
        }
        t = t_far;
    }
}

/// The CPU reference's shading normal at (x, y): the same corner normals,
/// bilinearly, normalised.
pub fn normal_reference(heights: &[f32], n: usize, m: usize, x: f64, y: f64) -> [f64; 3] {
    let h = |i: i64, j: i64| heights[(j.clamp(0, m as i64 - 1) as usize) * n + i.clamp(0, n as i64 - 1) as usize] as f64;
    let corner = |i: i64, j: i64| {
        let (i0, i1) = ((i - 1).max(0), (i + 1).min(n as i64 - 1));
        let (j0, j1) = ((j - 1).max(0), (j + 1).min(m as i64 - 1));
        let dx = (h(i1, j) - h(i0, j)) / ((i1 - i0).max(1) as f64);
        let dy = (h(i, j1) - h(i, j0)) / ((j1 - j0).max(1) as f64);
        [-dx, -dy, 1.0]
    };
    let cx = x.clamp(0.0, (n - 1) as f64);
    let cy = y.clamp(0.0, (m - 1) as f64);
    let a = (cx.floor() as i64).min(n as i64 - 2);
    let b = (cy.floor() as i64).min(m as i64 - 2);
    let (u, v) = (cx - a as f64, cy - b as f64);
    let lerp = |p: [f64; 3], q: [f64; 3], t: f64| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t];
    let nrm = lerp(lerp(corner(a, b), corner(a + 1, b), u), lerp(corner(a, b + 1), corner(a + 1, b + 1), u), v);
    let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
    [nrm[0] / l, nrm[1] / l, nrm[2] / l]
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga;

    fn validate(src: &str, what: &str) {
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{what} parse: {e}\n{src}"));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{what} validate: {e:?}"));
        assert!(
            crate::variations::shader_lint::self_operations(src).is_empty(),
            "{what}: fast-math self-op -- {:?}",
            crate::variations::shader_lint::self_operations(src)
        );
        assert!(
            crate::variations::shader_lint::subnormal_literals(src).is_empty(),
            "{what}: subnormal literal -- {:?}",
            crate::variations::shader_lint::subnormal_literals(src)
        );
    }

    #[test]
    fn the_terrain_shaders_validate() {
        validate(INGEST_WGSL, "ingest");
        validate(&assemble_build(), "build");
        validate(&assemble_walk(), "walk");
        validate(&assemble_relight(), "relight");
        validate(&assemble_path(), "path");
        validate(super::super::path_core::PATH_RESOLVE_WGSL, "path resolve");
    }

    #[test]
    fn the_uniform_matches_its_wgsl_mirror() {
        // 8 words, two vec4s, twenty vec4s.
        assert_eq!(std::mem::size_of::<TerrainParamsGpu>(), 32 + 32 + 320);
    }

    #[test]
    fn levels_reach_one_node() {
        assert_eq!(TerrainRenderer::levels_for(2, 2), 1);
        assert_eq!(TerrainRenderer::levels_for(3, 3), 2);
        assert_eq!(TerrainRenderer::levels_for(257, 193), 9);
        assert_eq!(TerrainRenderer::levels_for(2049, 2049), 12);
    }

    #[test]
    fn a_world_fixed_light_points_where_its_angles_say() {
        let east = light_direction(0.0, 0.0);
        assert!((east[0] - 1.0).abs() < 1e-6 && east[1].abs() < 1e-6 && east[2].abs() < 1e-6);
        let north_up = light_direction(90.0, 45.0);
        assert!(north_up[0].abs() < 1e-6 && (north_up[1] - north_up[2]).abs() < 1e-6 && north_up[1] > 0.0);
    }

    /// The CPU reference against a dense march with bisection, which
    /// knows nothing of patches or quadratics: the reference is only a
    /// reference if it is right.
    #[test]
    fn the_reference_agrees_with_a_dense_march() {
        let (n, m) = (33usize, 29usize);
        let heights: Vec<f32> = (0..n * m)
            .map(|c| {
                let (x, y) = ((c % n) as f32, (c / n) as f32);
                2.0 + 1.5 * (x * 0.4).sin() * (y * 0.3).cos() + if (c * 7919) % 97 == 0 { 4.0 } else { 0.0 }
            })
            .collect();
        let bilinear = |x: f64, y: f64| -> f64 {
            let a = (x.floor() as usize).min(n - 2);
            let b = (y.floor() as usize).min(m - 2);
            let (u, v) = (x - a as f64, y - b as f64);
            let h = |i: usize, j: usize| heights[j * n + i] as f64;
            (h(a, b) * (1.0 - u) + h(a + 1, b) * u) * (1.0 - v) + (h(a, b + 1) * (1.0 - u) + h(a + 1, b + 1) * u) * v
        };
        let mut seed = 12345u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 1_000_000) as f64 / 1_000_000.0
        };
        let mut checked = 0;
        for _ in 0..400 {
            let o = [rnd() * 32.0 - 0.0, rnd() * 28.0 - 6.0, 9.0 + rnd() * 3.0];
            let tgt = [rnd() * 32.0, rnd() * 28.0, rnd() * 2.0];
            let mut d = [tgt[0] - o[0], tgt[1] - o[1], tgt[2] - o[2]];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            d = [d[0] / l, d[1] / l, d[2] / l];
            let r = trace_reference(&heights, n, m, o, d);
            // Dense march from the box's entry: 1/256 of a cell, then
            // bisect.
            let mut t_in = 0.0f64;
            let mut t_out = f64::INFINITY;
            for (oc, dc, hi) in [(o[0], d[0], (n - 1) as f64), (o[1], d[1], (m - 1) as f64)] {
                let (ta, tb) = ((0.0 - oc) / dc, (hi - oc) / dc);
                t_in = t_in.max(ta.min(tb));
                t_out = t_out.min(ta.max(tb));
            }
            let gap = |t: f64| o[2] + d[2] * t - bilinear(o[0] + d[0] * t, o[1] + d[1] * t);
            let mut march = None;
            if t_in <= t_out {
                if gap(t_in) <= 0.0 {
                    march = Some(t_in);
                } else {
                    let mut prev = t_in;
                    let mut t = t_in;
                    while t < t_out {
                        t = (t + 1.0 / 256.0).min(t_out);
                        if gap(t) <= 0.0 {
                            let (mut lo, mut hi) = (prev, t);
                            for _ in 0..60 {
                                let mid = 0.5 * (lo + hi);
                                if gap(mid) > 0.0 { lo = mid } else { hi = mid }
                            }
                            march = Some(hi);
                            break;
                        }
                        prev = t;
                        if t >= t_out {
                            break;
                        }
                    }
                }
            }
            match (r, march) {
                (Some(a), Some(b)) => {
                    assert!((a - b).abs() < 1e-3, "reference {a} against march {b}");
                    checked += 1;
                }
                (None, None) => {}
                (a, b) => panic!("reference {a:?} against march {b:?}"),
            }
        }
        assert!(checked > 100, "only {checked} rays hit");
    }
}

/// The T1 gates (docs/projects/heightfield-3d.md, section 9), on the GPU.
#[cfg(test)]
pub(crate) mod gpu_tests {
    use super::*;
    use wgpu::*;

    pub(crate) fn device() -> Option<(Device, Queue)> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("terrain tests"),
            // The flame renderer's config load clears textures; the
            // footprint tests render through it.
            required_features: adapter.features() & Features::CLEAR_TEXTURE,
            required_limits: adapter.limits(),
            memory_hints: MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()?;
        device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu error in terrain tests: {e}")));
        Some((device, queue))
    }

    pub(crate) fn read_buffer(device: &Device, queue: &Queue, src: &Buffer, size: u64) -> Vec<u8> {
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        queue.submit(std::iter::once(enc.finish()));
        let slice = staging.slice(..);
        slice.map_async(MapMode::Read, |_| {});
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let out = slice.get_mapped_range().to_vec();
        staging.unmap();
        out
    }

    /// Texels of one mip of a texture, 4 or 16 bytes each.
    pub(crate) fn read_texture(device: &Device, queue: &Queue, tex: &Texture, level: u32, w: u32, h: u32, texel: u32) -> Vec<u8> {
        let row = (w * texel).div_ceil(256) * 256;
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            TexelCopyTextureInfo { texture: tex, mip_level: level, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            TexelCopyBufferInfo {
                buffer: &staging,
                layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        let slice = staging.slice(..);
        slice.map_async(MapMode::Read, |_| {});
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((w * h * texel) as usize);
        for y in 0..h {
            let at = (y * row) as usize;
            out.extend_from_slice(&data[at..at + (w * texel) as usize]);
        }
        drop(data);
        staging.unmap();
        out
    }

    /// A camera orbiting `target` (cell units) at `distance`, `pitch`
    /// above the horizon, turned by `yaw` -- the solid camera's own frame.
    fn camera(target: [f64; 3], pitch: f64, yaw: f64, distance: f64, fov: f32) -> SolidCamera {
        let (right, up, forward) = super::super::ifs::solid_frame(pitch, yaw, 0.0, 0.0);
        let eye_rel = [-forward[0] * distance, -forward[1] * distance, -forward[2] * distance];
        SolidCamera {
            eye: [target[0] + eye_rel[0], target[1] + eye_rel[1], target[2] + eye_rel[2]],
            target,
            forward,
            right,
            up,
            fov,
            distance,
            eye_rel,
        }
    }

    /// The ray `ifs_ray` builds for pixel (px, py), in f64.
    fn ray(cam: &SolidCamera, px: u32, py: u32, w: u32, h: u32) -> [f64; 3] {
        let uv = [(px as f64 + 0.5) / w as f64 - 0.5, (py as f64 + 0.5) / h as f64 - 0.5];
        let aspect = w as f64 / h as f64;
        let th = (cam.fov as f64 * 0.5).tan();
        let d = [
            cam.forward[0] + cam.right[0] * uv[0] * aspect * 2.0 * th - cam.up[0] * uv[1] * 2.0 * th,
            cam.forward[1] + cam.right[1] * uv[0] * aspect * 2.0 * th - cam.up[1] * uv[1] * 2.0 * th,
            cam.forward[2] + cam.right[2] * uv[0] * aspect * 2.0 * th - cam.up[2] * uv[1] * 2.0 * th,
        ];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        [d[0] / l, d[1] / l, d[2] / l]
    }

    /// The four analytic terrains of the T1 gate, n x m samples.
    fn terrain(kind: &str, n: usize, m: usize) -> Vec<f32> {
        let mut seed = 0x9e37_79b9u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        (0..n * m)
            .map(|c| {
                let (x, y) = ((c % n) as f32, (c / n) as f32);
                match kind {
                    "sinusoid" => 8.0 + 6.0 * (x * 0.07).sin() * (y * 0.05).cos(),
                    "cone" => {
                        let r = ((x - n as f32 * 0.5).powi(2) + (y - m as f32 * 0.5).powi(2)).sqrt();
                        (40.0 - 0.5 * r).max(0.0)
                    }
                    "step" => if x > n as f32 * 0.5 { 20.0 + y * 0.02 } else { y * 0.02 },
                    // Like the escape count: low, with rare tall spikes.
                    _ => 2.0 + rnd() + if rnd() < 0.01 { 60.0 * rnd() } else { 0.0 },
                }
            })
            .collect()
    }

    fn view(cam: SolidCamera) -> TerrainView {
        TerrainView {
            camera: cam,
            shading: SolidShadingSettings::default(),
            fog: (0.0, 0.0, [0.0; 3]),
            shadow: 1.0,
            softness: 8.0,
            occlusion_reach: 6.0,
            jitter: [0.0, 0.0],
            height: 0.0,
            width: 0.0,
            samples_per_axis: 1,
            far: 1.0e30,
        }
    }

    /// The mipmap's every level is the CPU's maximum: level 0 of a
    /// cell's four corners, each level above of a 2x2 block below.
    #[test]
    fn the_max_mipmap_is_the_cpu_maximum() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (n, m) = (37u32, 23u32);
        let h = terrain("spikes", n as usize, m as usize);
        let mut r = TerrainRenderer::new(&device, 8, 8);
        r.set_tile(&device, &queue, n, m, &h, &vec![[1.0; 4]; (n * m) as usize]);
        let levels = TerrainRenderer::levels_for(n, m);
        let mut cpu: Vec<f32> = Vec::new();
        let (mut w, mut hh) = (n - 1, m - 1);
        for y in 0..hh {
            for x in 0..w {
                let at = |i: u32, j: u32| h[(j * n + i) as usize];
                cpu.push(at(x, y).max(at(x + 1, y)).max(at(x, y + 1)).max(at(x + 1, y + 1)));
            }
        }
        for l in 0..levels {
            let got: Vec<f32> = bytemuck::cast_slice(&read_texture(&device, &queue, r.mips_texture().unwrap(), l, w, hh, 4)).to_vec();
            assert_eq!(got, cpu, "level {l}");
            let (nw, nh) = (w.div_ceil(2), hh.div_ceil(2));
            let mut next = Vec::new();
            for y in 0..nh {
                for x in 0..nw {
                    let at = |i: u32, j: u32| cpu[(j.min(hh - 1) * w + i.min(w - 1)) as usize];
                    next.push(at(2 * x, 2 * y).max(at(2 * x + 1, 2 * y)).max(at(2 * x, 2 * y + 1)).max(at(2 * x + 1, 2 * y + 1)));
                }
            }
            cpu = next;
            (w, hh) = (nw, nh);
        }
    }

    /// The traversal finds what the exhaustive CPU walk finds: every
    /// pixel's hit distance and normal, on four terrains, through the
    /// mipmap.
    #[test]
    fn terrain_hits_match_the_cpu_reference() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (n, m) = (257usize, 193usize);
        let (w, hgt) = (160u32, 120u32);
        let cam = camera([128.0, 96.0, 10.0], 0.5, 0.6, 300.0, 0.9);
        let mut r = TerrainRenderer::new(&device, w, hgt);
        for kind in ["sinusoid", "cone", "step", "spikes"] {
            let h = terrain(kind, n, m);
            r.set_tile(&device, &queue, n as u32, m as u32, &h, &vec![[1.0; 4]; n * m]);
            r.render(&device, &queue, &view(cam));
            let raw = read_buffer(&device, &queue, r.geometry_buffer(), (w * hgt) as u64 * 16);
            let geom: &[[u32; 4]] = bytemuck::cast_slice(&raw);
            let (mut hits, mut worst_t, mut worst_n, mut disagree, mut walls) = (0usize, 0.0f64, 0.0f64, 0usize, 0usize);
            for py in 0..hgt {
                for px in 0..w {
                    let g = geom[(py * w + px) as usize];
                    let t_gpu = f32::from_bits(g[3]) as f64;
                    let d = ray(&cam, px, py, w, hgt);
                    let cpu = trace_reference(&h, n, m, cam.eye, d);
                    match (cpu, t_gpu > 0.0) {
                        (Some(t), true) => {
                            hits += 1;
                            worst_t = worst_t.max((t - t_gpu).abs());
                            // The normal where the GPU hit, so the position's own
                            // error (above) is not counted twice: on a spike's
                            // flank the normal turns fast.
                            let p = [cam.eye[0] + d[0] * t_gpu, cam.eye[1] + d[1] * t_gpu, cam.eye[2] + d[2] * t_gpu];
                            let mut nc = normal_reference(&h, n, m, p[0], p[1]);
                            // A wall: the face's own normal.
                            let surface = {
                                let a = (p[0].clamp(0.0, (n - 1) as f64).floor() as usize).min(n - 2);
                                let b = (p[1].clamp(0.0, (m - 1) as f64).floor() as usize).min(m - 2);
                                let (u, v) = (p[0].clamp(0.0, (n - 1) as f64) - a as f64, p[1].clamp(0.0, (m - 1) as f64) - b as f64);
                                let hh = |i: usize, j: usize| h[j * n + i] as f64;
                                (hh(a, b) * (1.0 - u) + hh(a + 1, b) * u) * (1.0 - v) + (hh(a, b + 1) * (1.0 - u) + hh(a + 1, b + 1) * u) * v
                            };
                            let dist = [p[0], (n - 1) as f64 - p[0], p[1], (m - 1) as f64 - p[1]];
                            if dist.iter().cloned().fold(f64::MAX, f64::min) < 1e-2 && surface - p[2] > 1e-3 {
                                walls += 1;
                                let k = (0..4).min_by(|x, y| dist[*x].total_cmp(&dist[*y])).unwrap();
                                nc = [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 1.0, 0.0]][k];
                            }
                            let nxy = [
                                half::f16::from_bits((g[0] & 0xffff) as u16).to_f64(),
                                half::f16::from_bits((g[0] >> 16) as u16).to_f64(),
                            ];
                            let nz = half::f16::from_bits((g[1] & 0xffff) as u16).to_f64();
                            let dn = ((nxy[0] - nc[0]).powi(2) + (nxy[1] - nc[1]).powi(2) + (nz - nc[2]).powi(2)).sqrt();
                            worst_n = worst_n.max(dn);
                        }
                        (None, false) => {}
                        _ => disagree += 1,
                    }
                }
            }
            println!(
                "{kind}: {hits} hits ({walls} on the walls), worst |t| error {worst_t:.2e} cells, worst normal error {worst_n:.2e}, {disagree} hit/miss disagreements of {}",
                w * hgt
            );
            assert!(hits > (w * hgt) as usize / 5, "{kind}: only {hits} hits");
            // 1e-4 of the tile's width.
            assert!(worst_t < 1e-4 * n as f64, "{kind}: t off by {worst_t}");
            assert!(worst_n < 3e-3, "{kind}: normal off by {worst_n}");
            assert!(disagree * 1000 < (w * hgt) as usize, "{kind}: {disagree} disagreements");
        }
    }

    /// A flat plane under one sun, nothing else: the rig's Lambert term,
    /// exactly -- albedo times the light's colour and power times the
    /// cosine of its elevation.
    #[test]
    fn a_flat_plane_under_a_sun_is_albedo_times_e_cos_theta() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (n, m) = (64u32, 64u32);
        let albedo = [0.5f32, 0.25, 1.0, 1.0];
        let mut r = TerrainRenderer::new(&device, 32, 32);
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &vec![albedo; (n * m) as usize]);
        let mut shading = SolidShadingSettings::default();
        shading.shading_strength = 1.0;
        shading.ambient = 0.0;
        shading.diffuse = 1.0;
        shading.specular = 0.0;
        shading.ssao_strength = 0.0;
        shading.lights[0].enabled = true;
        shading.lights[0].azimuth = 30.0;
        shading.lights[0].elevation = 50.0;
        shading.lights[0].intensity = 2.0;
        shading.lights[0].color = [1.0, 0.5, 0.25];
        let mut v = view(camera([32.0, 32.0, 0.0], 1.0, 0.3, 40.0, 0.5));
        v.shading = shading;
        r.render(&device, &queue, &v);
        let out: Vec<[f32; 4]> = bytemuck::cast_slice(&read_texture(&device, &queue, r.output_texture(), 0, 32, 32, 16)).to_vec();
        let e = 2.0 * 50f32.to_radians().sin();
        let want = [albedo[0] * 1.0 * e, albedo[1] * 0.5 * e, albedo[2] * 0.25 * e];
        let mut worst = 0.0f32;
        for p in &out {
            assert_eq!(p[3], 1.0, "every pixel sees the plane");
            for c in 0..3 {
                worst = worst.max((p[c] - want[c]).abs());
            }
        }
        println!("flat plane: want {want:?}, worst error {worst:.2e}");
        assert!(worst < 1e-5, "{worst}");
    }

    /// Distant ground is the average of its texels: a one-cell
    /// checkerboard seen from far enough that a pixel covers about eight
    /// cells comes out an even grey, where sampling one texel per ray
    /// would come out a snow of black and white.
    #[test]
    fn distant_ground_is_filtered() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let n = 1025u32;
        let albedo: Vec<[f32; 4]> = (0..n * n)
            .map(|k| if ((k % n) + (k / n)) % 2 == 0 { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] })
            .collect();
        let mut r = TerrainRenderer::new(&device, 64, 64);
        r.set_tile(&device, &queue, n, n, &vec![0.0; (n * n) as usize], &albedo);
        let mut shading = SolidShadingSettings::default();
        shading.shading_strength = 0.0;
        // Straight down from far enough that a pixel spans ~8 cells.
        let fov = 0.5f32;
        let distance = 8.0 * 64.0 / (2.0 * (fov as f64 * 0.5).tan());
        let mut v = view(camera([512.0, 512.0, 0.0], 1.5, -std::f64::consts::FRAC_PI_2, distance, fov));
        v.shading = shading;
        v.shadow = 0.0;
        v.occlusion_reach = 0.0;
        r.render(&device, &queue, &v);
        let read = |r: &TerrainRenderer| -> Vec<f32> {
            let out: Vec<[f32; 4]> =
                bytemuck::cast_slice(&read_texture(&device, &queue, r.output_texture(), 0, 64, 64, 16)).to_vec();
            out.iter().map(|p| p[0]).collect()
        };
        let lum = read(&r);
        // The same view of a tile that IS the average: a uniform grey.
        let mut grey = TerrainRenderer::new(&device, 64, 64);
        grey.set_tile(&device, &queue, n, n, &vec![0.0; (n * n) as usize], &vec![[0.5, 0.5, 0.5, 1.0]; (n * n) as usize]);
        grey.render(&device, &queue, &v);
        let want = read(&grey);
        let mean = lum.iter().sum::<f32>() / lum.len() as f32;
        let want_mean = want.iter().sum::<f32>() / want.len() as f32;
        let sd = (lum.iter().map(|l| (l - mean) * (l - mean)).sum::<f32>() / lum.len() as f32).sqrt();
        println!("filtered checkerboard: mean {mean:.3} (the grey tile's {want_mean:.3}), sd {sd:.4}");
        assert!((mean - want_mean).abs() < 0.03 * want_mean, "{mean} vs {want_mean}");
        assert!(sd < 0.05 * want_mean, "{sd}");
    }

    /// Gently curving ground under a sun shadows nothing, however far the
    /// eye: its slopes stay under 4 degrees and the sun is at 30. At
    /// twelve thousand cells, where a position's rounding is a
    /// thousandth of a cell, a shadow ray started a fixed thousandth
    /// above the hit began under the curving surface, and the ground
    /// shadowed itself in speckles (seen on the seahorse's plains at
    /// 8192²). The start is now past the rounding. A FLAT plane does not
    /// show it: there the interpolated normal is the patch's own.
    #[test]
    fn far_gentle_ground_does_not_shadow_itself() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let n = 4097u32;
        let mut r = TerrainRenderer::new(&device, 128, 96);
        let tau = std::f32::consts::TAU;
        let heights: Vec<f32> = (0..n * n)
            .map(|k| {
                let (x, y) = ((k % n) as f32, (k / n) as f32);
                20.0 * (tau * x / 2000.0).sin() * (tau * y / 1500.0).sin()
            })
            .collect();
        r.set_tile(&device, &queue, n, n, &heights, &vec![[0.8, 0.8, 0.8, 1.0]; (n * n) as usize]);
        let mut shading = SolidShadingSettings::default();
        shading.shading_strength = 1.0;
        shading.lights[0].enabled = true;
        shading.lights[0].azimuth = 135.0;
        shading.lights[0].elevation = 30.0;
        let mut v = view(camera([2048.0, 2048.0, 0.0], 0.45, -std::f64::consts::FRAC_PI_2, 12_000.0, 0.25));
        v.shading = shading;
        v.shadow = 1.0;
        v.softness = 12.0;
        r.render(&device, &queue, &v);
        let geom: Vec<[u32; 4]> = bytemuck::cast_slice(&read_buffer(&device, &queue, r.geometry_buffer(), 128 * 96 * 16)).to_vec();
        let (mut hits, mut shadowed) = (0usize, 0usize);
        for g in &geom {
            if f32::from_bits(g[3]) > 0.0 {
                hits += 1;
                if (g[2] & 0xff) < 255 {
                    shadowed += 1;
                }
            }
        }
        println!("far gentle ground: {hits} hits, {shadowed} shadowed");
        assert!(hits > 128 * 96 / 2, "{hits}");
        assert_eq!(shadowed, 0, "the plane shadows itself");
    }

    /// Reads the path tracer's resolved output.
    fn read_output(device: &Device, queue: &Queue, r: &TerrainRenderer, w: u32, h: u32) -> Vec<[f32; 4]> {
        bytemuck::cast_slice(&read_texture(device, queue, r.output_texture(), 0, w, h, 16)).to_vec()
    }

    /// Lighting with no light at all: every light switched off.
    fn dark() -> SolidShadingSettings {
        let mut s = SolidShadingSettings::default();
        s.shading_strength = 1.0;
        s.diffuse = 1.0;
        for l in s.lights.iter_mut() {
            l.enabled = false;
        }
        s
    }

    /// The white furnace (plan T3): albedo 1 under a uniform environment
    /// L and no light. On a flat plane every path escapes after one
    /// bounce, so every sample IS L, at any bounce count from one; in a
    /// sinusoid's valleys paths bounce between slopes, and Russian
    /// roulette keeps them unbiased: the mean converges to L.
    #[test]
    fn the_white_furnace() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let l = [0.5f32, 0.25, 0.75];
        let mut r = TerrainRenderer::new(&device, w, h);
        let (n, m) = (129u32, 97u32);
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &vec![[1.0; 4]; (n * m) as usize]);
        let mut v = view(camera([64.0, 48.0, 0.0], 0.9, 0.3, 60.0, 0.6));
        v.shading = dark();
        for bounces in [1u32, 2, 4] {
            let settings = PathSettings { bounces, environment: l, clamp: 1.0e30, seed: 7, ..PathSettings::default() };
            r.reset_path();
            r.render_path(&device, &queue, &v, &settings, 8, 8);
            let out = read_output(&device, &queue, &r, w, h);
            let worst = out.iter().filter(|p| p[3] > 0.0).flat_map(|p| (0..3).map(move |k| (p[k] - l[k]).abs())).fold(0.0f32, f32::max);
            println!("flat furnace, {bounces} bounces: worst {worst:.2e}");
            assert!(worst < 1e-5, "{bounces}: {worst}");
        }
        // The valleys: the image's mean against L, to 1%.
        let hs = terrain("sinusoid", n as usize, m as usize);
        r.set_tile(&device, &queue, n, m, &hs, &vec![[1.0; 4]; (n * m) as usize]);
        let settings = PathSettings { bounces: 24, environment: l, clamp: 1.0e30, seed: 7, ..PathSettings::default() };
        r.reset_path();
        r.render_path(&device, &queue, &v, &settings, 1024, 64);
        let out = read_output(&device, &queue, &r, w, h);
        let hit: Vec<&[f32; 4]> = out.iter().filter(|p| p[3] > 0.0).collect();
        for k in 0..3 {
            let mean = hit.iter().map(|p| p[k] as f64).sum::<f64>() / hit.len() as f64;
            println!("valley furnace, channel {k}: mean {mean:.4} against {}", l[k]);
            assert!((mean / l[k] as f64 - 1.0).abs() < 0.01, "channel {k}: {mean}");
        }
    }

    /// Sun only: with no environment and a near-point sun, the path
    /// tracer's flat plane is the lit tier's, `albedo E cos theta`.
    #[test]
    fn a_sunlit_plane_is_the_lit_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (32u32, 32u32);
        let (n, m) = (64u32, 64u32);
        let albedo = [0.5f32, 0.25, 1.0, 1.0];
        let mut r = TerrainRenderer::new(&device, w, h);
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &vec![albedo; (n * m) as usize]);
        let mut shading = dark();
        shading.ambient = 0.0;
        shading.specular = 0.0;
        shading.ssao_strength = 0.0;
        shading.lights[0].enabled = true;
        shading.lights[0].azimuth = 30.0;
        shading.lights[0].elevation = 50.0;
        shading.lights[0].intensity = 2.0;
        shading.lights[0].color = [1.0, 0.5, 0.25];
        let mut v = view(camera([32.0, 32.0, 0.0], 1.0, 0.3, 40.0, 0.5));
        v.shading = shading;
        v.softness = 1.0e6;
        r.render(&device, &queue, &v);
        let lit = read_output(&device, &queue, &r, w, h);
        r.reset_path();
        r.render_path(&device, &queue, &v, &PathSettings { bounces: 2, environment: [0.0; 3], clamp: 1.0e30, seed: 1, ..PathSettings::default() }, 4, 4);
        let path = read_output(&device, &queue, &r, w, h);
        let worst = lit.iter().zip(&path).flat_map(|(a, b)| (0..4).map(move |k| (a[k] - b[k]).abs())).fold(0.0f32, f32::max);
        println!("sunlit plane: path against lit, worst {worst:.2e}");
        assert!(worst < 1e-4, "{worst}");
    }

    /// The material (T3b). Glow alone -- no light, no sky -- is the
    /// albedo times the emission, exactly. Under a uniform sky a
    /// dielectric coat (0.04) keeps a flat albedo-1 plane within the
    /// single-scattering microfacet's known loss of L; a near-mirror
    /// (reflectance 1, roughness 0.05) reflects the sky, L again.
    #[test]
    fn the_coat_and_the_glow() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (32u32, 24u32);
        let (n, m) = (65u32, 65u32);
        let mut r = TerrainRenderer::new(&device, w, h);
        let albedo = [0.5f32, 0.25, 0.75, 1.0];
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &vec![albedo; (n * m) as usize]);
        let mut v = view(camera([32.0, 32.0, 0.0], 0.8, 0.3, 50.0, 0.6));
        v.shading = dark();
        let glow = PathSettings { bounces: 0, emission: 2.0, ..PathSettings::default() };
        r.reset_path();
        r.render_path(&device, &queue, &v, &glow, 4, 4);
        let out = read_output(&device, &queue, &r, w, h);
        let worst = out.iter().filter(|p| p[3] > 0.0).flat_map(|p| (0..3).map(move |k| (p[k] - 2.0 * albedo[k]).abs())).fold(0.0f32, f32::max);
        println!("glow: worst {worst:.2e}");
        assert!(worst < 1e-5, "{worst}");
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &vec![[1.0; 4]; (n * m) as usize]);
        let l = [0.5f32, 0.25, 0.75];
        for (gloss, roughness, tolerance) in [(0.04f32, 0.5f32, 0.03f64), (1.0, 0.05, 0.02)] {
            let s = PathSettings { bounces: 2, environment: l, gloss, roughness, ..PathSettings::default() };
            r.reset_path();
            r.render_path(&device, &queue, &v, &s, 512, 64);
            let out = read_output(&device, &queue, &r, w, h);
            let hit: Vec<&[f32; 4]> = out.iter().filter(|p| p[3] > 0.0).collect();
            for k in 0..3 {
                let mean = hit.iter().map(|p| p[k] as f64).sum::<f64>() / hit.len() as f64;
                println!("coat {gloss}/{roughness}, channel {k}: mean {mean:.4} against {}", l[k]);
                assert!(mean <= l[k] as f64 * 1.005, "no energy from nowhere: {mean}");
                assert!(mean >= l[k] as f64 * (1.0 - tolerance), "{gloss}/{roughness} channel {k}: {mean}");
            }
        }
    }

    /// The lake (T3d): a tile all lake, black under the surface, under a
    /// uniform sky L, seen from 46 degrees above. Water's Fresnel mirrors
    /// the sky more as the view grazes it: about 0.02 L near head on (the
    /// frame's near rows), rising toward the horizon, never past the sky.
    /// A black plateau reflects nothing.
    #[test]
    fn the_lake_is_water() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let (n, m) = (129u32, 129u32);
        let l = [0.5f32, 0.6, 0.7];
        let mut v = view(camera([64.0, 64.0, 0.0], 0.8, 0.25, 40.0, 0.6));
        v.shading = dark();
        let mut r = TerrainRenderer::new(&device, w, h);
        let shot = |r: &mut TerrainRenderer, lake: bool| {
            let heights = vec![if lake { -3.0e30 } else { 0.0 }; (n * m) as usize];
            r.set_tile(&device, &queue, n, m, &heights, &vec![[0.0, 0.0, 0.0, 1.0]; (n * m) as usize]);
            let s = PathSettings { bounces: 2, environment: l, lake_roughness: 0.02, ..PathSettings::default() };
            r.reset_path();
            r.render_path(&device, &queue, &v, &s, 256, 64);
            read_output(&device, &queue, r, w, h)
        };
        let plateau = shot(&mut r, false);
        let worst = plateau.iter().filter(|p| p[3] > 0.0).map(|p| p[0].max(p[1]).max(p[2])).fold(0.0f32, f32::max);
        assert!(worst < 1e-6, "a black plateau reflects nothing: {worst}");
        let lake = shot(&mut r, true);
        let rows = |y0: u32, y1: u32| {
            let px: Vec<&[f32; 4]> = (y0..y1).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| &lake[(y * w + x) as usize]).filter(|p| p[3] > 0.0).collect();
            px.iter().map(|p| p[0] as f64 / l[0] as f64).sum::<f64>() / px.len().max(1) as f64
        };
        let (far, near) = (rows(0, h / 3), rows(2 * h / 3, h));
        let brightest = lake.iter().filter(|p| p[3] > 0.0).map(|p| p[2] / l[2]).fold(0.0f32, f32::max);
        println!("lake: far rows {far:.3} L, near rows {near:.3} L, brightest {brightest:.3} L");
        assert!((near - 0.02).abs() < 0.005, "water reflects 2% head on: {near}");
        assert!(far > near * 1.2, "the mirror strengthens toward the horizon: {near} {far}");
        assert!(brightest <= 1.01, "no brighter than the sky: {brightest}");
    }

    /// The lens (T3b): a pinhole -- aperture 0 -- is the same bits
    /// whatever the focus; an open lens keeps the focal plane: a plane at
    /// the focus, seen face on, converges to the pinhole's picture.
    #[test]
    fn the_lens_keeps_its_focal_plane() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (32u32, 24u32);
        let (n, m) = (257u32, 257u32);
        let albedo: Vec<[f32; 4]> = (0..n * m)
            .map(|k| if ((k % n) / 16 + (k / n) / 16) % 2 == 0 { [0.9, 0.9, 0.9, 1.0] } else { [0.1, 0.1, 0.1, 1.0] })
            .collect();
        let mut r = TerrainRenderer::new(&device, w, h);
        r.set_tile(&device, &queue, n, m, &vec![0.0; (n * m) as usize], &albedo);
        // Straight down from 200 cells: the plane is the focal plane.
        let mut v = view(camera([128.0, 128.0, 0.0], 1.5707, -std::f64::consts::FRAC_PI_2, 200.0, 0.6));
        v.shading = dark();
        let env = [1.0f32, 1.0, 1.0];
        let sum = |r: &mut TerrainRenderer, s: &PathSettings, samples: u32| {
            r.reset_path();
            r.render_path(&device, &queue, &v, s, samples, samples);
            read_buffer(&device, &queue, r.path_sum_for_test(), (w * h * 16) as u64)
        };
        let pin_a = sum(&mut r, &PathSettings { bounces: 1, environment: env, focus: 50.0, ..PathSettings::default() }, 16);
        let pin_b = sum(&mut r, &PathSettings { bounces: 1, environment: env, focus: 300.0, ..PathSettings::default() }, 16);
        assert_eq!(pin_a, pin_b, "a pinhole ignores the focus");
        let pinhole = {
            r.reset_path();
            r.render_path(&device, &queue, &v, &PathSettings { bounces: 1, environment: env, ..PathSettings::default() }, 256, 64);
            read_output(&device, &queue, &r, w, h)
        };
        let lens = {
            r.reset_path();
            let s = PathSettings { bounces: 1, environment: env, aperture: 20.0, focus: 200.0, ..PathSettings::default() };
            r.render_path(&device, &queue, &v, &s, 256, 64);
            read_output(&device, &queue, &r, w, h)
        };
        let worst = pinhole.iter().zip(&lens).flat_map(|(a, b)| (0..3).map(move |k| (a[k] - b[k]).abs())).fold(0.0f32, f32::max);
        println!("in focus: lens against pinhole, worst {worst:.3}");
        assert!(worst < 0.03, "{worst}");
        // Out of focus, the checks blur toward their mean.
        let blurred = {
            r.reset_path();
            let s = PathSettings { bounces: 1, environment: env, aperture: 20.0, focus: 60.0, ..PathSettings::default() };
            r.render_path(&device, &queue, &v, &s, 256, 64);
            read_output(&device, &queue, &r, w, h)
        };
        let spread = |img: &[[f32; 4]]| {
            let mean = img.iter().map(|p| p[0]).sum::<f32>() / img.len() as f32;
            (img.iter().map(|p| (p[0] - mean).powi(2)).sum::<f32>() / img.len() as f32).sqrt()
        };
        println!("contrast: focused {:.3}, blurred {:.3}", spread(&lens), spread(&blurred));
        assert!(spread(&blurred) < 0.7 * spread(&lens), "out of focus blurs");
    }

    /// The same samples in any number of dispatches are the same bits:
    /// twelve samples as twelve, three or one dispatch -- over a sinusoid
    /// with a sun, an environment and three bounces -- and again.
    #[test]
    fn path_samples_are_batch_invariant() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (40u32, 30u32);
        let (n, m) = (129u32, 97u32);
        let hs = terrain("sinusoid", n as usize, m as usize);
        let albedo: Vec<[f32; 4]> = (0..n * m).map(|k| [0.3 + 0.5 * ((k % 7) as f32 / 7.0), 0.6, 0.4, 1.0]).collect();
        let mut r = TerrainRenderer::new(&device, w, h);
        r.set_tile(&device, &queue, n, m, &hs, &albedo);
        let v = view(camera([64.0, 48.0, 8.0], 0.6, 0.4, 150.0, 0.9));
        let settings = PathSettings { bounces: 3, environment: [0.3, 0.4, 0.6], clamp: 10.0, seed: 3, ..PathSettings::default() };
        let mut sums = Vec::new();
        for per in [1u32, 4, 12, 12] {
            r.reset_path();
            r.render_path(&device, &queue, &v, &settings, 12, per);
            sums.push(read_buffer(&device, &queue, r.path_sum_for_test(), (w * h * 16) as u64));
        }
        assert!(sums.windows(2).all(|p| p[0] == p[1]), "the sums differ across batchings");
        let f: &[f32] = bytemuck::cast_slice(&sums[0]);
        assert!(f.iter().any(|v| *v > 0.0), "something was traced");
    }

    /// A light's colour or power is a relight: no walk, and the picture a
    /// fresh walk would make. Its direction is a walk.
    #[test]
    fn a_lighting_edit_is_a_relight() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (n, m) = (129u32, 97u32);
        let h = terrain("sinusoid", n as usize, m as usize);
        let albedo = vec![[0.8, 0.7, 0.6, 1.0]; (n * m) as usize];
        let cam = camera([64.0, 48.0, 8.0], 0.6, 0.4, 150.0, 0.9);
        let mut lit = SolidShadingSettings::default();
        lit.shading_strength = 1.0;
        lit.ambient = 0.2;
        lit.diffuse = 0.8;
        lit.lights[0].enabled = true;
        lit.lights[0].azimuth = 135.0;
        lit.lights[0].elevation = 25.0;
        let mut v = view(cam);
        v.shading = lit.clone();
        let mut r = TerrainRenderer::new(&device, 96, 72);
        r.set_tile(&device, &queue, n, m, &h, &albedo);
        r.render(&device, &queue, &v);
        assert_eq!(r.walks, 1);
        // Colour and power: the cache.
        v.shading.lights[0].color = [1.0, 0.6, 0.3];
        v.shading.lights[0].intensity = 1.7;
        v.fog = (0.002, 50.0, [0.2, 0.3, 0.4]);
        r.render(&device, &queue, &v);
        assert_eq!(r.walks, 1, "a colour, power or fog edit walked");
        let cached = read_texture(&device, &queue, r.output_texture(), 0, 96, 72, 16);
        let mut fresh = TerrainRenderer::new(&device, 96, 72);
        fresh.set_tile(&device, &queue, n, m, &h, &albedo);
        fresh.render(&device, &queue, &v);
        let walked = read_texture(&device, &queue, fresh.output_texture(), 0, 96, 72, 16);
        assert_eq!(cached, walked, "the relight is the walk's picture");
        // Direction: a walk.
        v.shading.lights[0].azimuth = 200.0;
        r.render(&device, &queue, &v);
        assert_eq!(r.walks, 2, "a light's direction is a walk input");
    }

    /// A look at it: the prototype's Mandelbrot terrain (height
    /// `H exp(-DE / w)` over the seahorse valley, the escape count's
    /// palette for albedo) through the GPU renderer, to
    /// `output/heightfield_t1/`.
    #[test]
    #[ignore = "demo"]
    fn terrain_demo_mandelbrot() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let n = 1025usize;
        let (cx, cy, span) = (-0.7453f64, 0.1127f64, 0.012f64);
        let mut heights = vec![0.0f32; n * n];
        let mut albedo = vec![[0.0f32; 4]; n * n];
        let pal = [[0.05, 0.1, 0.35], [0.2, 0.55, 0.75], [0.95, 0.85, 0.5], [0.8, 0.3, 0.1], [0.05, 0.1, 0.35]];
        let w_de = span / 150.0;
        for j in 0..n {
            for i in 0..n {
                // Row j runs north: Im increases with j.
                let c = (cx - span / 2.0 + span * i as f64 / (n - 1) as f64, cy - span / 2.0 + span * j as f64 / (n - 1) as f64);
                let (mut zx, mut zy, mut dx, mut dy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                let mut out = None;
                for it in 0..2000 {
                    let (ndx, ndy) = (2.0 * (zx * dx - zy * dy) + 1.0, 2.0 * (zx * dy + zy * dx));
                    (dx, dy) = (ndx, ndy);
                    (zx, zy) = (zx * zx - zy * zy + c.0, 2.0 * zx * zy + c.1);
                    let r2 = zx * zx + zy * zy;
                    if r2 > 65536.0 {
                        let r = r2.sqrt();
                        let de = r * r.ln() / (dx * dx + dy * dy).sqrt();
                        let mu = it as f64 + 1.0 - (r.ln()).log2();
                        out = Some((de, mu));
                        break;
                    }
                }
                let k = j * n + i;
                match out {
                    Some((de, mu)) => {
                        // Cell units: the tile is n - 1 cells across span.
                        heights[k] = ((0.06 * (n - 1) as f64) * (-de / w_de).exp()) as f32;
                        let t = ((1.0 + mu).ln() * 0.9).fract() * 4.0;
                        let (i0, f) = (t.floor() as usize, t.fract());
                        let a = pal[i0.min(4)];
                        let b = pal[(i0 + 1).min(4)];
                        albedo[k] = [
                            (a[0] + (b[0] - a[0]) * f) as f32,
                            (a[1] + (b[1] - a[1]) * f) as f32,
                            (a[2] + (b[2] - a[2]) * f) as f32,
                            1.0,
                        ];
                    }
                    None => {
                        heights[k] = (0.06 * (n - 1) as f64) as f32;
                        albedo[k] = [0.9, 0.9, 0.92, 1.0];
                    }
                }
            }
        }
        let (w, h) = (960u32, 600u32);
        let mut r = TerrainRenderer::new(&device, w, h);
        r.set_tile(&device, &queue, n as u32, n as u32, &heights, &albedo);
        // From the south, looking north, 30 degrees down -- the
        // prototype's framing. The solid camera's yaw is the eye's
        // azimuth about the target, so -90 degrees puts it south.
        let cam = camera([512.0, 480.0, 20.0], 0.55, -std::f64::consts::FRAC_PI_2, 1050.0, 0.8);
        let mut v = view(cam);
        v.occlusion_reach = 8.0;
        r.render(&device, &queue, &v);
        let px: Vec<[f32; 4]> = bytemuck::cast_slice(&read_texture(&device, &queue, r.output_texture(), 0, w, h, 16)).to_vec();
        let enc = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
        let mut img = image::RgbaImage::new(w, h);
        for (k, p) in px.iter().enumerate() {
            let bg = [0.55f32, 0.65, 0.8];
            let c = [0, 1, 2].map(|i| p[i] * p[3] + bg[i] * (1.0 - p[3]));
            img.put_pixel(k as u32 % w, k as u32 / w, image::Rgba([enc(c[0]), enc(c[1]), enc(c[2]), 255]));
        }
        std::fs::create_dir_all("output/heightfield_t1").unwrap();
        img.save("output/heightfield_t1/mandelbrot_de.png").unwrap();
        println!("wrote output/heightfield_t1/mandelbrot_de.png");
    }

    /// A Mandelbrot footprint, rendered by the escape renderer as a
    /// terrain's footprint: the config, the palette's renderer, and the
    /// render run to settlement. Returns (colour, height field) as rows
    /// of `Rgba32Float` texels, and the renderer.
    fn footprint(
        device: &Device,
        queue: &Queue,
        n: u32,
        source: crate::config::escape::TerrainSource,
    ) -> (crate::config::escape::EscapeConfig, crate::escape::EscapeRenderer, crate::renderer::compute_kernel::FlameRenderer) {
        let mut config = crate::config::FractalConfig::default();
        config.escape.center_re = "-0.75".into();
        config.escape.center_im = "0.1".into();
        config.escape.zoom_log2 = 3.0;
        config.escape.max_iter = 2000;
        config.escape.terrain.enabled = true;
        config.escape.terrain.source = source;
        let mut flame = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            device,
            queue,
            TextureFormat::Rgba8Unorm,
            n,
            n,
            &config.flame,
            config.palette_size,
        );
        flame.update_palette(
            device,
            queue,
            &config.palette,
            config.palette_rotation,
            config.palette_squeeze,
            config.palette_squeeze_mode,
            config.palette_squeeze_falloff,
            config.palette_log_strength,
            config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(device, n, n);
        let mut guard = 0;
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("footprint") });
            let settled = escape.render(device, queue, &mut enc, &config.escape, flame.palette_view(), flame.palette_generation());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "the footprint did not settle");
        }
        (config.escape, escape, flame)
    }

    /// The f64 Mandelbrot at a footprint pixel, as the escape template
    /// iterates it (z0 = 0, dz0 = 0, escape on |z|^2 > bailout): the
    /// distance estimate in pixels and the smooth count, or None inside.
    fn mandelbrot_px(esc: &crate::config::escape::EscapeConfig, n: u32, px: u32, py: u32) -> Option<(f64, f64)> {
        let span = 4.0 / 2f64.powf(esc.zoom_log2);
        let pix = span / n as f64;
        let (cx, cy) = esc.center_f64();
        let c = (cx + ((px as f64 + 0.5) / n as f64 - 0.5) * span, cy - ((py as f64 + 0.5) / n as f64 - 0.5) * span);
        let (mut zx, mut zy, mut dx, mut dy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for it in 0..esc.max_iter {
            (dx, dy) = (2.0 * (zx * dx - zy * dy) + 1.0, 2.0 * (zx * dy + zy * dx));
            (zx, zy) = (zx * zx - zy * zy + c.0, 2.0 * zx * zy + c.1);
            if zx * zx + zy * zy > esc.bailout as f64 {
                let r = (zx * zx + zy * zy).sqrt();
                let de = r * r.ln() / ((dx * dx + dy * dy).sqrt() * pix);
                let mu = (it + 1) as f64 + 1.0 - r.ln().ln() / 2f64.ln();
                return Some((de, mu));
            }
        }
        None
    }

    /// A terrain footprint writes its height source into the height
    /// field's green channel: the distance estimate in pixels, the smooth
    /// escape count, and the interior's sentinel -- against the f64
    /// Mandelbrot at the same pixels.
    #[test]
    fn a_footprint_writes_the_terrain_height_source() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let n = 96u32;
        use crate::config::escape::TerrainSource;
        for source in [TerrainSource::Distance, TerrainSource::EscapeCount] {
            let (esc, escape, _flame) = footprint(&device, &queue, n, source);
            let hf: Vec<[f32; 4]> =
                bytemuck::cast_slice(&read_texture(&device, &queue, escape.height_texture_for_test(), 0, n, n, 16)).to_vec();
            let colour: Vec<[f32; 4]> =
                bytemuck::cast_slice(&read_texture(&device, &queue, escape.output_texture_for_test(), 0, n, n, 16)).to_vec();
            // Escaped or not by COVERAGE and the sentinel, as the ingest
            // reads it: an interior pixel the colouring does not draw
            // keeps a height of 0, not the sentinel.
            let (mut escaped, mut worst, mut off, mut wrong_side, mut early) = (0usize, 0.0f64, 0usize, 0usize, 0usize);
            for py in 0..n {
                for px in 0..n {
                    let k = (py * n + px) as usize;
                    let g = hf[k][1] as f64;
                    let gpu_interior = !(colour[k][3] > 0.0) || g <= -1e29;
                    match (mandelbrot_px(&esc, n, px, py), gpu_interior) {
                        (Some((de, mu)), false) => {
                            escaped += 1;
                            // The precision check where an f32 orbit is
                            // still the f64 one: pixels that escape early
                            // and lie at least a hundredth of a pixel out.
                            // Closer, the two orbits part: |dz| runs past
                            // 1e8 and f32's distance is off by percents
                            // (2.9x at 1e-6 px), and the count by up to
                            // two iterations -- where a distance height
                            // has long since saturated to the plateau.
                            if mu < 200.0 && de > 0.01 {
                                early += 1;
                                let want = if source == TerrainSource::Distance { de } else { mu };
                                let rel = (g - want).abs() / want.abs().max(1e-6);
                                worst = worst.max(rel);
                                if rel > 1e-3 {
                                    off += 1;
                                }
                            }
                        }
                        (None, true) => {}
                        _ => wrong_side += 1,
                    }
                }
            }
            println!(
                "{source:?}: {escaped} escaped pixels, {early} early and clear of the set, of which {off} more than 0.1% off \
                 (worst {worst:.2e}), {wrong_side} on the other side of the set"
            );
            assert!(early > 1000, "{early}");
            assert!(off * 100 < early, "{off} of {early} off");
            assert!(wrong_side * 200 < (n * n) as usize, "{wrong_side} disagree on escaping");
        }
    }

    /// The ingest turns a footprint into the tile: the raw height source
    /// (the distance, never below 0), the interior a plateau -- its raw
    /// 0, the distance the map takes to the top -- in the background
    /// colour, the albedo the picture's colour; every row flipped, the
    /// picture's top the tile's north. As a hole, the interior's raw is
    /// the far sentinel and its albedo clear.
    #[test]
    fn the_ingest_makes_the_tile_from_a_footprint() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let n = 96u32;
        let (_esc, escape, _flame) = footprint(&device, &queue, n, crate::config::escape::TerrainSource::Distance);
        let colour: Vec<[f32; 4]> =
            bytemuck::cast_slice(&read_texture(&device, &queue, escape.output_texture_for_test(), 0, n, n, 16)).to_vec();
        let hf: Vec<[f32; 4]> =
            bytemuck::cast_slice(&read_texture(&device, &queue, escape.height_texture_for_test(), 0, n, n, 16)).to_vec();
        for interior in [0u32, 1, 2] {
            let hole = interior == 1;
            let ingest = TerrainIngest { source: 8, interior, background: [0.1, 0.2, 0.3], tint: [0.4, 0.5, 0.6] };
            let mut r = TerrainRenderer::new(&device, 8, 8);
            r.set_tile_from_escape(&device, &queue, escape.output_view(), escape.height_view(), n, n, &ingest, 1.0);
            assert_eq!(r.tile_shape_for_test(), Some((n, n, 1)), "a distance tile maps by the distance");
            let (ht, at) = r.tile_textures_for_test().unwrap();
            let raw: Vec<f32> = bytemuck::cast_slice(&read_texture(&device, &queue, ht, 0, n, n, 4)).to_vec();
            let albedo_bits: Vec<u16> = bytemuck::cast_slice(&read_texture(&device, &queue, at, 0, n, n, 8)).to_vec();
            let (mut worst_h, mut worst_a, mut plateau) = (0.0f32, 0.0f32, 0usize);
            for j in 0..n {
                for i in 0..n {
                    let src = ((n - 1 - j) * n + i) as usize;
                    let dst = (j * n + i) as usize;
                    let (c, g) = (colour[src], hf[src][1]);
                    let interior = !(c[3] > 0.0) || g <= -1e29;
                    let (want_raw, want_a) = match (interior, ingest.interior) {
                        (true, 0) => {
                            plateau += 1;
                            (0.0, [0.1, 0.2, 0.3, 1.0])
                        }
                        (true, 1) => (1.0e30, [0.0; 4]),
                        (true, _) => {
                            plateau += 1;
                            (3.0e30, [0.4, 0.5, 0.6, 1.0])
                        }
                        _ => (g.max(0.0), [c[0], c[1], c[2], 1.0]),
                    };
                    worst_h = worst_h.max((raw[dst] - want_raw).abs() / want_raw.abs().max(1.0));
                    for k in 0..4 {
                        let a = half::f16::from_bits(albedo_bits[dst * 4 + k]).to_f32();
                        worst_a = worst_a.max((a - want_a[k]).abs() / want_a[k].abs().max(1.0));
                    }
                }
            }
            println!("ingest (interior {interior}): worst raw error {worst_h:.2e}, worst albedo error {worst_a:.2e}, {plateau} interior samples");
            assert!(hole || (plateau > 0 && plateau < (n * n) as usize), "{plateau}");
            assert!(worst_h < 1e-6, "{worst_h}");
            assert!(worst_a < 1e-3, "{worst_a}");
        }
    }

    /// The T1 measurement: steps per ray and time at 1080p, on the four
    /// terrains at 2049^2 (the footprint's default size).
    #[test]
    #[ignore = "measurement"]
    fn terrain_cost_at_1080p() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (n, m) = (2049usize, 2049usize);
        let (w, hgt) = (1920u32, 1080u32);
        let cam = camera([1024.0, 1024.0, 10.0], 0.5, 0.6, 2200.0, 0.9);
        let mut r = TerrainRenderer::new(&device, w, hgt);
        for kind in ["sinusoid", "cone", "step", "spikes"] {
            let h = terrain(kind, n, m);
            r.set_tile(&device, &queue, n as u32, m as u32, &h, &vec![[0.7, 0.7, 0.7, 1.0]; n * m]);
            r.count_steps = true;
            r.render(&device, &queue, &view(cam));
            let stats: Vec<u32> = bytemuck::cast_slice(&read_buffer(&device, &queue, r.stats_buffer(), 16)).to_vec();
            r.count_steps = false;
            // Timed in batches of twenty submissions and one wait: a single
            // submit-and-wait measured latency, not the work (rays that
            // took four thousand steps "cost" the same 0.6 ms). Best of
            // three batches. A walk each: the softness moves the key.
            const BATCH: usize = 20;
            let mut best = f64::MAX;
            for round in 0..3 {
                let t0 = std::time::Instant::now();
                for i in 0..BATCH {
                    let mut v = view(cam);
                    v.softness = 8.0 + (round * BATCH + i) as f32 * 1e-3;
                    r.render(&device, &queue, &v);
                }
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                best = best.min(t0.elapsed().as_secs_f64() * 1e3 / BATCH as f64);
            }
            // Relights only: the fog moves, the walk's key does not.
            let mut relight = f64::MAX;
            for round in 0..3 {
                let t0 = std::time::Instant::now();
                for i in 0..BATCH {
                    let mut v = view(cam);
                    v.softness = 8.0 + 59.0 * 1e-3;
                    v.fog = (0.001 + (round * BATCH + i) as f32 * 1e-6, 0.0, [0.5; 3]);
                    r.render(&device, &queue, &v);
                }
                let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
                relight = relight.min(t0.elapsed().as_secs_f64() * 1e3 / BATCH as f64);
            }
            println!(
                "{kind}: primary steps mean {:.1}, max {}; all rays {:.1} steps a pixel; walk+relight {best:.2} ms, relight alone {relight:.2} ms",
                stats[0] as f64 / stats[2].max(1) as f64,
                stats[1],
                stats[3] as f64 / stats[2].max(1) as f64,
            );
            assert!(stats[1] < HF_MAX_STEPS, "{kind}: a ray reached the step cap");
        }
    }
}

