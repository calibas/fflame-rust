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
    /// 0 plateau, 1 hole.
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
    /// The interior as a hole rather than a plateau.
    pub hole: bool,
    /// The plateau's colour where the colouring leaves the interior
    /// undrawn: the background's.
    pub background: [f32; 3],
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

const WALK_WGSL: &str = r#"
@group(0) @binding(1) var hf_height: texture_2d<f32>;
@group(0) @binding(2) var hf_mips: texture_2d<f32>;
@group(0) @binding(3) var<storage, read_write> ifs_geom: array<vec4<u32>>;
@group(0) @binding(4) var<storage, read_write> hf_stats: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read> hf_range: array<u32>;

const HF_MAX_STEPS: u32 = __HF_MAX_STEPS__u;

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

// A raw sample's height: the identity (a tile set from heights), the
// distance's H exp(-d / w), the count's log curve, or the relief's
// linear one, each with the interior's sentinels -- monotone, so the
// mipmap's raw extreme maps to its highest point.
fn hf_f(raw: f32) -> f32 {
    let mode = u32(params.fdata[6].w);
    let top = params.fdata[6].x;
    if (mode == 0u) {
        return raw;
    }
    if (mode == 1u) {
        if (raw >= 1.0e29) {
            return params.eye.w;
        }
        return top * exp(-max(raw, 0.0) / max(params.fdata[6].y, 1.0e-6));
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

// A height sample, the grid's edge held beyond it.
fn hf_h(i: i32, j: i32) -> f32 {
    let q = clamp(vec2<i32>(i, j), vec2<i32>(0, 0), vec2<i32>(i32(params.grid_n) - 1, i32(params.grid_m) - 1));
    return hf_f(textureLoad(hf_height, q, 0).r);
}

// The surface's height at (x, y): the bilinear patch of its cell.
fn hf_height_at(x: f32, y: f32) -> f32 {
    let cx = clamp(x, 0.0, f32(params.grid_n - 1u));
    let cy = clamp(y, 0.0, f32(params.grid_m - 1u));
    let a = min(i32(floor(cx)), i32(params.grid_n) - 2);
    let b = min(i32(floor(cy)), i32(params.grid_m) - 2);
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
}

// The maximum-mipmap traversal. `tmax` bounds the ray; `soft_k` > 0
// also measures the penumbra at the leaves the ray passed over.
fn hf_trace(o: vec3<f32>, d: vec3<f32>, tmax: f32, soft_k: f32) -> HfHit {
    var out: HfHit;
    out.t = -1.0;
    out.hit = false;
    out.soft = 1.0;
    out.steps = 0u;
    let cn = f32(params.grid_n - 1u);
    let cm = f32(params.grid_m - 1u);
    let top_level = i32(params.levels) - 1;
    let top = hf_f(textureLoad(hf_mips, vec2<i32>(0, 0), top_level).r);

    // Clip to the tile's box: x in [0, cn], y in [0, cm], z between the
    // slab's floor and the highest point.
    let floor_z = params.eye.w;
    var t0 = 0.0;
    var t1 = tmax;
    if (abs(d.x) < 1.0e-12) {
        if (o.x < 0.0 || o.x > cn) {
            return out;
        }
    } else {
        let ta = -o.x / d.x;
        let tb = (cn - o.x) / d.x;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
    }
    if (abs(d.y) < 1.0e-12) {
        if (o.y < 0.0 || o.y > cm) {
            return out;
        }
    } else {
        let ta = -o.y / d.y;
        let tb = (cm - o.y) / d.y;
        t0 = max(t0, min(ta, tb));
        t1 = min(t1, max(ta, tb));
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
    let span = max(max(abs(o.x), abs(o.y)), max(cn, cm));
    let eps_base = 1.0e-4 + span * 4.0e-7;
    loop {
        if (out.steps >= HF_MAX_STEPS) {
            break;
        }
        out.steps = out.steps + 1u;
        let p = o + d * t;
        let size = f32(1 << u32(level));
        let dims = vec2<i32>(
            (i32(params.grid_n) - 1 + (1 << u32(level)) - 1) >> u32(level),
            (i32(params.grid_m) - 1 + (1 << u32(level)) - 1) >> u32(level),
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
        let zmax = hf_f(textureLoad(hf_mips, vec2<i32>(a, b), level).r);
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
            // How close the ray passed this cell's surface, at its ends.
            let p1 = o + d * t_far;
            let gap = min(p.z - hf_height_at(p.x, p.y), p1.z - hf_height_at(p1.x, p1.y));
            out.soft = min(out.soft, clamp(soft_k * gap / max(t, 1.0e-3), 0.0, 1.0));
        }
        t = max(t_far, t) + eps;
        level = min(level + 1, top_level);
        if (t >= t1) {
            break;
        }
    }
    return out;
}

// A sample's own normal, from its neighbours (one-sided at the edge).
fn hf_corner_normal(i: i32, j: i32) -> vec3<f32> {
    let i0 = max(i - 1, 0);
    let i1 = min(i + 1, i32(params.grid_n) - 1);
    let j0 = max(j - 1, 0);
    let j1 = min(j + 1, i32(params.grid_m) - 1);
    let dx = (hf_h(i1, j) - hf_h(i0, j)) / f32(max(i1 - i0, 1));
    let dy = (hf_h(i, j1) - hf_h(i, j0)) / f32(max(j1 - j0, 1));
    return vec3<f32>(-dx, -dy, 1.0);
}

// The outward normal of the tile's side face nearest p.
fn hf_wall_normal(p: vec3<f32>) -> vec3<f32> {
    let cn = f32(params.grid_n - 1u);
    let cm = f32(params.grid_m - 1u);
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

// The shading normal at (x, y): the corners' normals, bilinearly, so the
// light is smooth across a cell even where the patch is faceted.
fn hf_normal(x: f32, y: f32) -> vec3<f32> {
    let cx = clamp(x, 0.0, f32(params.grid_n - 1u));
    let cy = clamp(y, 0.0, f32(params.grid_m - 1u));
    let a = min(i32(floor(cx)), i32(params.grid_n) - 2);
    let b = min(i32(floor(cy)), i32(params.grid_m) - 2);
    let u = cx - f32(a);
    let v = cy - f32(b);
    let n0 = mix(hf_corner_normal(a, b), hf_corner_normal(a + 1, b), u);
    let n1 = mix(hf_corner_normal(a, b + 1), hf_corner_normal(a + 1, b + 1), u);
    return normalize(mix(n0, n1, v));
}

// Occlusion from the horizon: in eight directions, the steepest rise
// within the reach, as the sine of its elevation; one minus their mean.
// A height field's own occlusion, and deterministic -- no noise to
// accumulate away.
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
            let rise = hf_height_at(q.x, q.y) - p.z;
            best = max(best, rise * inverseSqrt(rise * rise + dist * dist));
        }
        occ = occ + best;
    }
    return clamp(1.0 - occ / 8.0, 0.0, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }
    let idx = gid.y * params.width + gid.x;
    hf_load_range();
    let o = params.eye.xyz;
    let d = ifs_ray(gid.x, gid.y);
    let h = hf_trace(o, d, 1.0e30, 0.0);
    var steps = h.steps;
    if (!h.hit) {
        ifs_geom[idx] = vec4<u32>(0u, 0u, 0u, 0u);
    } else {
        let p = o + d * h.t;
        var n = hf_normal(p.x, p.y);
        var ao = hf_occlusion(p, params.misc.z);
        // A ray that entered through the tile's side under the surface
        // hit a WALL: the slab's edge, lit by its own outward normal, and
        // open to the sky (the occlusion below the surface is the
        // surface's, not the wall's). Only ON the boundary: elsewhere a
        // point a rounding under a steep flank is the flank.
        let edge = min(min(p.x, f32(params.grid_n - 1u) - p.x), min(p.y, f32(params.grid_m - 1u) - p.y));
        if (edge < 1.0e-2 && hf_height_at(p.x, p.y) - p.z > 1.0e-3) {
            n = hf_wall_normal(p);
            ao = 1.0;
        }
        var sun = vec4<f32>(1.0, 1.0, 1.0, 1.0);
        if (ifs_shadow_strength() > 0.0) {
            let start = p + n * params.misc.w;
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

const RELIGHT_WGSL: &str = r#"
@group(0) @binding(1) var hf_albedo: texture_2d<f32>;
@group(0) @binding(2) var<storage, read> ifs_geom: array<vec4<u32>>;
@group(0) @binding(3) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var hf_sampler: sampler;

// The albedo at (x, y), filtered over `foot` cells: bilinear between
// the samples where a ray's share of a pixel is smaller than a cell,
// the mip chain's averages where it covers several -- so distant ground
// is the average of its texels rather than whichever one a ray struck.
fn hf_albedo_at(x: f32, y: f32, foot: f32) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(hf_albedo));
    let uv = (vec2<f32>(x, y) + vec2<f32>(0.5, 0.5)) / dims;
    let lod = clamp(log2(max(foot, 1.0e-6)), 0.0, f32(textureNumLevels(hf_albedo) - 1u));
    return textureSampleLevel(hf_albedo, hf_sampler, uv, lod);
}

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
    // A ray's share of a pixel at the hit, in cells, widened where the
    // surface is seen at a slant.
    let foot = t * params.fdata[6].z / sqrt(max(abs(dot(n, dir)), 0.05));
    let albedo = hf_albedo_at(p.x, p.y, foot);
    let sun = unpack4x8unorm(g.z);
    let rgb = ifs_rig(albedo.rgb, n, nz_ao.y, sun, dir, t);
    textureStore(out_tex, px, vec4<f32>(rgb, clamp(albedo.a, 0.0, 1.0)));
}
"#;

/// The walk's WGSL, assembled.
pub fn assemble_walk() -> String {
    format!(
        "{}\n{}",
        WALK_COMMON.replace("//__IFS_RIG__", &super::assembler::ifs_rig_jittered()),
        WALK_WGSL.replace("__HF_MAX_STEPS__", &HF_MAX_STEPS.to_string())
    )
}

/// The relight's WGSL, assembled.
pub fn assemble_relight() -> String {
    format!(
        "{}\n{}",
        WALK_COMMON.replace("//__IFS_RIG__", &super::assembler::ifs_rig_jittered()),
        RELIGHT_WGSL
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
    /// Fog (strength per cell, start in cells, colour), as the rig
    /// applies it.
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
}

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
    let any = !SolidShadingSettings::is_default(&view.shading);
    k.push_str(&format!("|{any}"));
    for l in &view.shading.lights {
        k.push_str(&format!("|{}:{}:{}", l.enabled && l.intensity > 0.0, l.azimuth, l.elevation));
    }
    k
}

struct Tile {
    n: u32,
    m: u32,
    /// The slab's floor, in cells (`slab_floor`).
    floor: f32,
    levels: u32,
    /// The walk's map from the raw samples to heights (`hf_f`): 0 the
    /// identity, 1 distance, 2 count, 3 relief.
    mode: u32,
    /// The raw samples (`R32Float`).
    height: (Texture, TextureView),
    /// The albedo with its mip chain (`Rgba16Float`).
    albedo: (Texture, TextureView),
    mips: Texture,
    mips_all: TextureView,
    /// A count's or a relief's range over the escaped samples, in the
    /// ordered encoding (`hf_range`).
    range: Buffer,
    version: u64,
}

/// A tile being made from footprint regions, while the last one is
/// still drawn.
struct Build {
    n: u32,
    m: u32,
    mode: u32,
    params: IngestParamsGpu,
    height: Texture,
    albedo: Texture,
    range: Buffer,
}

/// The terrain renderer: a tile, its mipmap, and the walk and relight
/// over it. Part of the escape engine, beside mode D, whose camera and
/// rig it shares.
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
    sampler: Sampler,
    build: Option<Build>,
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
    tile: Option<Tile>,
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
        let walk_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Walk"),
            entries: &[uniform(0), tex(1), tex(2), storage(3, false), storage(4, false), storage(5, true)],
        });
        let filterable = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let relight_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Relight"),
            entries: &[
                uniform(0),
                filterable(1),
                storage(2, true),
                storage_tex(3, TextureFormat::Rgba32Float),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let albedo_mip_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Terrain Albedo Mips"),
            entries: &[tex(0), storage_tex(1, TextureFormat::Rgba16Float)],
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
        let output = Self::create_output(device, out_w, out_h);
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
            sampler,
            build: None,
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
            tile: None,
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

    /// Bytes a tile sample costs, about: the raw sample, the albedo and
    /// its mips, the mipmap at power-of-two sides. Twice that while a
    /// build replaces a tile.
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
        self.output.0.destroy();
        if let Some(pair) = &self.accum {
            pair[0].0.destroy();
            pair[1].0.destroy();
        }
        if let Some(tile) = &self.tile {
            tile.height.0.destroy();
            tile.albedo.0.destroy();
            tile.mips.destroy();
            tile.range.destroy();
        }
        if let Some(b) = &self.build {
            b.height.destroy();
            b.albedo.destroy();
            b.range.destroy();
        }
    }

    /// Start a new accumulation: the next `accumulate` replaces the mean.
    pub fn reset_accumulation(&mut self) {
        self.accum_count = 0;
    }

    /// The tile's version: a new one for every tile set.
    pub fn tile_version(&self) -> u64 {
        self.tile.as_ref().map_or(0, |t| t.version)
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
        let (height, albedo_tex) = Self::tile_textures(device, n, m);
        queue.write_texture(
            TexelCopyTextureInfo { texture: &height, mip_level: 0, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            bytemuck::cast_slice(heights),
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(m) },
            Extent3d { width: n, height: m, depth_or_array_layers: 1 },
        );
        let halves: Vec<u16> = albedo.iter().flatten().map(|v| half::f16::from_f32(*v).to_bits()).collect();
        queue.write_texture(
            TexelCopyTextureInfo { texture: &albedo_tex, mip_level: 0, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            bytemuck::cast_slice(&halves),
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 8), rows_per_image: Some(m) },
            Extent3d { width: n, height: m, depth_or_array_layers: 1 },
        );
        let range = Self::range_buffer(device, queue);
        self.finish_tile(device, queue, n, m, 0, height, albedo_tex, range, slab_floor(heights, n, m));
    }

    /// A tile from one footprint render (heightfield plan, section 5):
    /// its colour and its height field, both `n x m` and `Rgba32Float`,
    /// on the GPU throughout. The build of one region.
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
    ) {
        self.begin_build(device, queue, n, m, ingest);
        self.ingest_region(device, queue, colour, height_field, 0, 0, n, m);
        self.finish_build(device, queue);
    }

    /// Start a tile of `n x m` samples from footprint regions. The last
    /// tile is drawn until [`finish_build`](Self::finish_build) replaces
    /// it; a build already under way is dropped.
    pub fn begin_build(&mut self, device: &Device, queue: &Queue, n: u32, m: u32, ingest: &TerrainIngest) {
        assert!(n >= 2 && m >= 2, "a tile needs at least 2x2 samples");
        if let Some(b) = self.build.take() {
            b.height.destroy();
            b.albedo.destroy();
            b.range.destroy();
        }
        let (height, albedo) = Self::tile_textures(device, n, m);
        let range = Self::range_buffer(device, queue);
        let params = IngestParamsGpu {
            n,
            m,
            source: ingest.source,
            interior: u32::from(ingest.hole),
            ox: 0,
            oy: 0,
            rw: 0,
            rh: 0,
            background: [ingest.background[0], ingest.background[1], ingest.background[2], 1.0],
        };
        self.build = Some(Build { n, m, mode: ingest.mode(), params, height, albedo, range });
    }

    /// One region of the footprint into the tile being built: its colour
    /// and height field, `rw x rh`, whose top-left is pixel `(ox, oy)` of
    /// the footprint picture. Submits its own work.
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
        let b = self.build.as_ref().expect("begin_build first");
        assert!(ox + rw <= b.n && oy + rh <= b.m, "a region inside the tile");
        let p = IngestParamsGpu { ox, oy, rw, rh, ..b.params };
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Ingest Params"),
            size: std::mem::size_of::<IngestParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&params, 0, bytemuck::bytes_of(&p));
        let hv = b.height.create_view(&TextureViewDescriptor::default());
        let av = b.albedo.create_view(&TextureViewDescriptor { base_mip_level: 0, mip_level_count: Some(1), ..Default::default() });
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Terrain Ingest"),
            layout: &self.ingest_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(colour) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(height_field) },
                BindGroupEntry { binding: 3, resource: b.range.as_entire_binding() },
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

    /// Finish the tile being built -- its mipmap and the albedo's -- and
    /// draw it from now on. The slab's floor is a hundredth of the
    /// tile's side below zero: the raw maps land in [0, H].
    pub fn finish_build(&mut self, device: &Device, queue: &Queue) {
        let b = self.build.take().expect("begin_build first");
        let floor = -0.01 * b.n.max(b.m) as f32;
        self.finish_tile(device, queue, b.n, b.m, b.mode, b.height, b.albedo, b.range, floor);
    }

    /// Whether a build is under way.
    pub fn building(&self) -> bool {
        self.build.is_some()
    }

    /// The range's buffer, emptied: min high, max low, in the ordered
    /// encoding.
    fn range_buffer(device: &Device, queue: &Queue) -> Buffer {
        let range = device.create_buffer(&BufferDescriptor {
            label: Some("Terrain Range"),
            size: 8,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&range, 0, bytemuck::cast_slice(&[u32::MAX, 0u32]));
        range
    }

    /// The tile's raw (`R32Float`) and albedo (`Rgba16Float`, with its
    /// mip chain) textures, unwritten.
    fn tile_textures(device: &Device, n: u32, m: u32) -> (Texture, Texture) {
        let make = |label: &str, format: TextureFormat, mips: u32| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d { width: n, height: m, depth_or_array_layers: 1 },
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
        let albedo_levels = 32 - n.max(m).leading_zeros();
        (make("Terrain Height", TextureFormat::R32Float, 1), make("Terrain Albedo", TextureFormat::Rgba16Float, albedo_levels))
    }

    /// The tile's mipmap from its samples, the albedo's mip chain, and
    /// the tile itself.
    #[allow(clippy::too_many_arguments)]
    fn finish_tile(
        &mut self,
        device: &Device,
        queue: &Queue,
        n: u32,
        m: u32,
        mode: u32,
        height: Texture,
        albedo_tex: Texture,
        range: Buffer,
        floor: f32,
    ) {
        let make = |label: &str, format: TextureFormat, mips: u32, w: u32, h: u32| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
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
        let levels = Self::levels_for(n, m);
        // Power-of-two sides: a texture's mips halve rounding DOWN, the
        // node grid halves rounding UP, and only at powers of two do the
        // two agree -- otherwise the top levels' last nodes fall outside
        // their mip, the build's writes are dropped and the walk reads
        // them as zero. The texels past the cells are never read.
        let mips = make(
            "Terrain Max Mipmap",
            TextureFormat::R32Float,
            levels,
            (n - 1).next_power_of_two(),
            (m - 1).next_power_of_two(),
        );
        let mips_all = mips.create_view(&TextureViewDescriptor::default());
        let height_view = height.create_view(&TextureViewDescriptor::default());
        let albedo_view = albedo_tex.create_view(&TextureViewDescriptor::default());
        self.build_mips(device, queue, &height_view, &mips, n, m, levels, u32::from(mode == 1));
        self.build_albedo_mips(device, queue, &albedo_tex);
        if let Some(old) = self.tile.take() {
            old.height.0.destroy();
            old.albedo.0.destroy();
            old.mips.destroy();
            old.range.destroy();
        }
        self.tile = Some(Tile {
            n,
            m,
            floor,
            levels,
            mode,
            height: (height, height_view),
            albedo: (albedo_tex, albedo_view),
            mips,
            mips_all,
            range,
            version: self.next_version,
        });
        self.next_version += 1;
        self.walked = None;
    }

    /// The albedo's mip chain: each level the box average of the one
    /// below.
    fn build_albedo_mips(&self, device: &Device, queue: &Queue, albedo: &Texture) {
        let levels = albedo.mip_level_count();
        let level_view = |l: u32| {
            albedo.create_view(&TextureViewDescriptor { base_mip_level: l, mip_level_count: Some(1), ..Default::default() })
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
    fn build_mips(&self, device: &Device, queue: &Queue, height: &TextureView, mips: &Texture, n: u32, m: u32, levels: u32, mode: u32) {
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
                base_mip_level: l,
                mip_level_count: Some(1),
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
        let tile = self.tile.as_ref().expect("a tile");
        let mut fdata = [[0.0f32; 4]; 20];
        pack_rig(&view.camera, &view.shading, view.fog, &mut fdata);
        // The map from raw samples to heights, and a ray's share of a
        // pixel per unit of distance: the albedo's filter.
        let per_ray = 2.0 * (view.camera.fov * 0.5).tan() / self.out_h.max(1) as f32 / view.samples_per_axis.max(1) as f32;
        fdata[6] = [view.height, view.width, per_ray, tile.mode as f32];
        // The shadow ray's start above the surface: past a position's
        // rounding at these coordinates, or a surface shadows itself in
        // speckles (seen at 8192 cells, where a cell's thousandth is
        // the rounding).
        let e = view.camera.eye;
        let span = (tile.n.max(tile.m) as f64).max(e[0].abs()).max(e[1].abs()).max(e[2].abs()) as f32;
        let bias = 1.0e-3 + span * 1.0e-6;
        let p = TerrainParamsGpu {
            width: self.out_w,
            height: self.out_h,
            grid_n: tile.n,
            grid_m: tile.m,
            levels: tile.levels,
            flags: u32::from(self.count_steps),
            jitter: view.jitter,
            eye: [view.camera.eye[0] as f32, view.camera.eye[1] as f32, view.camera.eye[2] as f32, tile.floor],
            misc: [view.shadow, view.softness, view.occlusion_reach, bias],
            fdata,
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&p));
    }

    /// Render the view: walk if anything the walk reads changed, then
    /// relight. Submits its own work.
    pub fn render(&mut self, device: &Device, queue: &Queue, view: &TerrainView) {
        let Some(tile) = self.tile.as_ref() else { return };
        let key = walk_key(view, self.out_w, self.out_h, tile.version);
        let walk = self.walked.as_deref() != Some(key.as_str()) || self.count_steps;
        self.write_params(queue, view);
        let tile = self.tile.as_ref().expect("checked");
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
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&tile.height.1) },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&tile.mips_all) },
                    BindGroupEntry { binding: 3, resource: self.geom.as_entire_binding() },
                    BindGroupEntry { binding: 4, resource: self.stats.as_entire_binding() },
                    BindGroupEntry { binding: 5, resource: tile.range.as_entire_binding() },
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
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&tile.albedo.1) },
                    BindGroupEntry { binding: 2, resource: self.geom.as_entire_binding() },
                    BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&self.output.1) },
                    BindGroupEntry { binding: 4, resource: BindingResource::Sampler(&self.sampler) },
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

    /// The tile's raw and albedo textures, for a test to read back.
    #[cfg(test)]
    pub(crate) fn tile_textures_for_test(&self) -> Option<(&Texture, &Texture)> {
        self.tile.as_ref().map(|t| (&t.height.0, &t.albedo.0))
    }

    /// The tile's side and its map, for a test.
    #[cfg(test)]
    pub(crate) fn tile_shape_for_test(&self) -> Option<(u32, u32, u32)> {
        self.tile.as_ref().map(|t| (t.n, t.m, t.mode))
    }

    /// The tile's mipmap texture, for a test to read a level back.
    #[cfg(test)]
    pub(crate) fn mips_texture(&self) -> Option<&Texture> {
        self.tile.as_ref().map(|t| &t.mips)
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

    fn read_buffer(device: &Device, queue: &Queue, src: &Buffer, size: u64) -> Vec<u8> {
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
        for hole in [false, true] {
            let ingest = TerrainIngest { source: 8, hole, background: [0.1, 0.2, 0.3] };
            let mut r = TerrainRenderer::new(&device, 8, 8);
            r.set_tile_from_escape(&device, &queue, escape.output_view(), escape.height_view(), n, n, &ingest);
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
                    let (want_raw, want_a) = match (interior, hole) {
                        (true, false) => {
                            plateau += 1;
                            (0.0, [0.1, 0.2, 0.3, 1.0])
                        }
                        (true, true) => (1.0e30, [0.0; 4]),
                        _ => (g.max(0.0), [c[0], c[1], c[2], 1.0]),
                    };
                    worst_h = worst_h.max((raw[dst] - want_raw).abs() / want_raw.abs().max(1.0));
                    for k in 0..4 {
                        let a = half::f16::from_bits(albedo_bits[dst * 4 + k]).to_f32();
                        worst_a = worst_a.max((a - want_a[k]).abs() / want_a[k].abs().max(1.0));
                    }
                }
            }
            println!("ingest (hole {hole}): worst raw error {worst_h:.2e}, worst albedo error {worst_a:.2e}, {plateau} plateau samples");
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

