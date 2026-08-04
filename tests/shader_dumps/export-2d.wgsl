// Hard-coded shader constants (compiled at shader build time)
const NUM_TRANSFORMS: u32 = 1u;
const COLOR_MODE: u32 = 0u;
const HAS_POST_AFFINE: bool = false;
const USE_INLINED_WEIGHTS: bool = false;
const USE_INLINED_TRANSFORMS: bool = false;

// Shared header for all trajectory shaders
// Contains struct definitions and bind group declarations

// Transform structure (matching CPU-side Transform)
struct Transform {
    // Affine matrix coefficients
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,

    // Z offset for 3D mode
    g: f32,

    // Probability weight
    weight: f32,

    // Post-affine matrix (applied after variations)
    post_a: f32,
    post_b: f32,
    post_c: f32,
    post_d: f32,
    post_e: f32,
    post_f: f32,
    post_g: f32,
    post_enabled: f32, // 0.0 = disabled, 1.0 = enabled

    // Variation weights (100 slots: supports all Apophysis 7X + future expansion)
    variations: array<f32, 100>,

    // Color (palette position) + color_speed + opacity + direct_color (vec4 for alignment)
    color: f32,
    color_speed: f32,
    opacity: f32,
    direct_color: f32,

    // JWildfire-extension plane affines, indexed `[a, c, b, d, e, f]`
    // matching the XML attribute write order. See
    // `docs/projects/jwf-features.md` ("zxCoefs / yzCoefs") for the
    // composition rule transcribed from `TransformationAffineFullStep`.
    yz_coefs: array<f32, 6>,
    zx_coefs: array<f32, 6>,
    yz_post_coefs: array<f32, 6>,
    zx_post_coefs: array<f32, 6>,

    // Bit flags: bit 0 = YZ active, bit 1 = ZX active,
    // bit 2 = YZ post active, bit 3 = ZX post active. The host
    // computes these on upload by comparing each plane to identity.
    // When the relevant bit is 0, the corresponding step in
    // `apply_affine` / `apply_post_affine` is skipped — flat 2D math
    // (the Apophysis path) takes over with zero added cost.
    plane_flags: u32,

    // Analytic-blur routing slot (i32; -1 = not eligible). Matches
    // `GpuTransform::analytic_blur_slot`. See analytic-blur-buffer.md.
    analytic_blur_slot: i32,

    // Analytic-blur routing knobs (matches GpuTransform). `strength` scales
    // the mean-splat density; `residual` keeps routing the next N plots
    // through the blur buffer. Carved from the former 2-u32 pad.
    analytic_blur_strength: f32,
    analytic_blur_residual: u32,
}

// Dispatch parameters
struct Params {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,
    height: u32,
    seed: u32,
    color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    render_mode: u32,  // 0 = 2D, 1 = 3D
    splat_size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,  // Rotation in radians (2D, around Z)
    speed_factor: f32,  // Blend factor for speed-based coloring
    perspective_strength: f32,  // Strength for perspective projection
    // Depth-density compensation strength s: 3D samples weighted by
    // zr^(-2s) so apparent brightness is depth-invariant. 0 = off.
    depth_density_compensation: f32,
    // Far density fade: sample density weighted by
    // exp(-(far_density_fade_start - camera_z)^2 * far_density_fade)
    // beyond the start depth. 0 = off.
    far_density_fade: f32,
    far_density_fade_start: f32,
    camera_rotation_x: f32,  // 3D camera pitch (rotation around X)
    camera_rotation_y: f32,  // 3D camera yaw (rotation around Z — Apo ZXY Euler)
    camera_bank: f32,        // 3D camera bank — rotation around Y (JWildfire bank parameter)
    camera_x: f32,  // 3D camera X position (world space)
    camera_y: f32,  // 3D camera Y position (world space)
    camera_z: f32,  // 3D camera Z position (height / world space)
    dof_focus_distance: f32,  // Depth of field: distance where image is sharpest
    dof_blur_strength: f32,  // Depth of field: blur amount (0.0 = disabled)
    fog_strength: f32,  // Depth fog: exponential fog density (0.0 = disabled)
    fog_start: f32,  // Depth fog: distance where fog begins
    bits_per_transform: u32,  // Bits needed per transform index (1-5 based on num_transforms)
    path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=Prefix (Distinct), 3=Suffix (Distinct)
    path_capture_mode: u32,  // 0=FirstHit, 1=FirstAfterBurnIn, 2=LastHit
    path_tracking_mode: u32,  // 0=First (first 32 iterations), 1=Recent (rolling window of 32 most recent)
    num_path_filters: u32,  // Number of active path filters (0 = disabled)
    min_suffix_filter_length: u32,  // Minimum length among depth=0 filters (for optimization)
    background_r: f32,  // Background color R (for depth fog)
    background_g: f32,  // Background color G (for depth fog)
    background_b: f32,  // Background color B (for depth fog)
    // Solid rendering (Phase 0). These three fields occupy what used to be
    // the 12-byte std140 pad before `post_symmetry` (37 scalars × 4 = 148
    // bytes; the struct must start at a 16-byte boundary = 160), so the
    // layout is unchanged. Only read when the SOLID builder flag is set.
    // Mirror in `src/gpu/buffers.rs`.
    solid_strength: f32,     // Occlusion strength: 0 = off (transparent), 1 = hard surface
    surface_thickness: f32,  // Depth shell accepted as "the surface" (world units)
    depth_prime: u32,        // 1 = depth-priming batch (record depth, plot nothing)
    post_symmetry: PostSymmetry,  // Plot-time symmetry (gated by HAS_POST_SYMMETRY)
    // Light-space shadow maps (solid rendering Stage 2): ortho fit +
    // world-space light directions. shadow_count = 0 disables the
    // splat at runtime. Mirror in src/gpu/buffers.rs.
    shadow_center_x: f32,
    shadow_center_y: f32,
    shadow_center_z: f32,
    shadow_radius: f32,
    shadow_count: u32,
    _pad_shadow0: u32,
    _pad_shadow1: u32,
    _pad_shadow2: u32,
    shadow_dirs: array<vec4<f32>, 4>,
}

// Plot-time symmetry. Matches `GpuPostSymmetry` in src/gpu/buffers.rs.
// When `kind == 0` the shader builder strips the symmetry block via
// HAS_POST_SYMMETRY, so these fields don't get read.
struct PostSymmetry {
    kind: u32,         // 0=None, 1=XAxis, 2=YAxis, 3=Point
    order: u32,        // K for Point mode, clamped to [1, 32]
    center_x: f32,
    center_y: f32,
    distance: f32,     // Pan along the symmetry axis (axis modes only)
    rotation: f32,     // Pre-rotation, radians (axis modes only)
    _pad_a: f32,
    _pad_b: f32,
}

// Variation parameters for one transform — single flat array
// shared by every active variation on the transform. Each variation
// gets a contiguous run starting at the offset assigned by
// `compute_packed_layout` (Rust side). The shader builder reads /
// writes through `get_param(xform_id, variation_id, slot)` which
// resolves the offset from a per-transform header — no
// `variation_id * N + slot` constant stride anymore. Individual
// variations can declare as many params as they need (the `complex`
// variation already uses 64); the 1600-slot ceiling is on the
// transform-wide total across all its active variations.
struct VariationParams {
    params: array<f32, 1600>,
}

// Path storage for PathMap color mode
// Stores up to 32 iterations losslessly (4 bits per transform, up to 16 transforms)
// Also stores initial random X/Y coordinates for complete path reconstruction
struct PathEntry {
    path0: u32,  // Iterations 0-7 (4 bits each, LSB = iteration 0)
    path1: u32,  // Iterations 8-15
    path2: u32,  // Iterations 16-23
    path3: u32,  // Iterations 24-31
    iteration_count: u32,  // Actual iteration when pixel was hit (not capped at 32)
    initial_x: f32,  // Initial random X coordinate [-1, 1]
    initial_y: f32,  // Initial random Y coordinate [-1, 1]
}

// Path filter for blocking specific transform sequences
// depth=0: suffix match (block paths ending with pattern at any depth)
// depth>0: exact depth match (block paths matching pattern at specific iteration)
struct PathFilter {
    pattern: u32,  // Packed pattern (up to 8 iterations at 4 bits each, LSB = first)
    length: u32,   // Number of iterations in pattern (1-8)
    depth: u32,    // 0 = suffix match, >0 = match at this exact depth
    _padding: u32, // Padding for 16-byte alignment
}

// Per-normal-transform attachment list — entries hold global xform_ids
// pointing into the concatenated transforms[] array. The main loop walks
// these after the chaos game picks a normal transform: linkeds advance
// the dynamics state (their output feeds the next iteration), finals
// shape the plotted point only (output discarded for IFS).
// See docs/projects/per-transform-linked-and-final.md.
struct AttachmentList {
    linked: array<u32, 1>,
    linked_count: u32,
    final_: array<u32, 1>,
    final_count: u32,
}


// Sample-emit output: shader writes one Sample per plotted point to a
// streaming buffer, host-side accumulate pass scatters into per-tile
// histograms. Used by multi-tile strategies (above the single-binding
// histogram size limit). See docs/projects/unified-render-pipeline.md.
//
// WGSL storage array stride must be a multiple of 16 bytes; 5 floats
// (20B) padded to 32B (8 floats).
struct Sample {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    // Density weight (depth-density compensation; 1.0 = neutral).
    // Scales all four histogram adds in the accumulate pass.
    weight: f32,
    _pad2: f32,
    _pad3: f32,
}

// Atomic counter — host pre-zeros .count, shader bumps it per emitted
// sample, accumulate pass reads .count to know how many to consume.
struct SampleCounter {
    count: atomic<u32>,
}


// Bindings
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: Params;

// Sample stream: shader writes one entry per plotted point.
@group(0) @binding(2) var<storage, read_write> samples: array<Sample>;


@group(0) @binding(3) var palette_texture: texture_2d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;
@group(0) @binding(5) var<storage, read> variation_params: array<VariationParams>;

// Sample buffer write cursor.
@group(0) @binding(6) var<storage, read_write> sample_counter: SampleCounter;

@group(0) @binding(7) var<storage, read_write> path_buffer: array<PathEntry>;
@group(0) @binding(8) var<storage, read> path_filters: array<PathFilter>;
// Xaos (chaos) transition weights: xaos_weights[src * num_transforms + dst]
// Modifies probability of selecting dst transform when coming from src
@group(0) @binding(9) var<storage, read> xaos_weights: array<f32>;
// Per-normal-transform attachment lists. Indexed by the normal's
// xform_id (0..num_transforms). See AttachmentList struct above.
@group(0) @binding(10) var<storage, read> attachments: array<AttachmentList>;

// Per-subflame metadata: where each subflame's normals + finals live
// inside the *unified* `transforms[]` buffer. Indexed by
// `subflame_id` (the variation parameter). Pre-v2 there was a
// separate `subflame_transforms` array at @binding(11); that's gone
// — subflame xforms now live in the parent's `transforms` buffer at
// `[xform_id_base + normals_offset, xform_id_base + normals_offset +
// normals_count)` (and similarly for finals). See `SubflameMeta` in
// `src/gpu/buffers.rs` for the matching Rust struct.
struct SubflameMeta {
    normals_offset: u32,
    normals_count: u32,
    finals_offset: u32,
    finals_count: u32,
    _reserved_render_mode: u32,  // was render_mode; render mode is scene-global in v3
    // Base added to (normals_offset/finals_offset + picked) to form the
    // synthetic xform_id the variation system sees. v1: always 128.
    // v2 will set this per-subflame to the unified-array start position
    // so per-xform buffers (variation_params, transforms, thread_state)
    // resolve real slots for subflame xforms.
    xform_id_base: u32,
    _pad0: u32,
    _pad1: u32,
}
@group(0) @binding(12) var<storage, read> subflame_metadata: array<SubflameMeta>;

// Per-transform analytic-blur mean-splat buffers at LOW resolution,
// concatenated: slice `b` occupies `[b·lw·lh·4, (b+1)·lw·lh·4)` (lw,lh =
// blur_convolve_params.lowres_*), same `[Rsum,Gsum,Bsum,density]` 4-u32
// layout. A transform with `analytic_blur_slot = b` (and `b < count`) routes
// its mean-splat here at `mean_pixel ÷ D` instead of the main histogram; a
// later pass convolves each slice with its kernel and upscale-adds it back.
// Always bound (a 1-element dummy when inactive). See analytic-blur-buffer.md.
@group(0) @binding(13) var<storage, read_write> blur_histograms: array<atomic<u32>>;

// Analytic-blur convolution params (mirrors `BlurConvolveParams` in
// gpu/buffers.rs). The chaos-game routing reads `downscale`, `lowres_*`, and
// `count` to splat into the low-res buffer above. Always bound. Only read in
// the HAS_ANALYTIC_BLUR routing, so naga strips it when the feature is off.
struct BlurConvolveParams {
    full_width: u32,
    full_height: u32,
    lowres_width: u32,
    lowres_height: u32,
    downscale: u32,
    count: u32,
    frame_seed: u32,
    _pad1: u32,
    slot_meta: array<vec4<u32>, 4>,
}
@group(0) @binding(14) var<uniform> blur_params: BlurConvolveParams;

// PCG random number generator
// Provides deterministic random number generation for shader execution

// RNG state
struct RngState {
    state: u32,
}

// PCG hash function
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Initialize RNG with thread ID and global seed
fn rng_init(thread_id: u32, seed: u32) -> RngState {
    var rng: RngState;
    rng.state = pcg_hash(thread_id ^ seed);
    return rng;
}

// Generate random u32
fn rng_next(rng: ptr<function, RngState>) -> u32 {
    let old_state = (*rng).state;
    (*rng).state = old_state * 747796405u + 2891336453u;
    let xor_shifted = ((old_state >> ((old_state >> 28u) + 4u)) ^ old_state) * 277803737u;
    return (xor_shifted >> 22u) ^ xor_shifted;
}

// Generate random f32 in [0, 1)
fn rng_nextf(rng: ptr<function, RngState>) -> f32 {
    return f32(rng_next(rng)) / 4294967296.0;
}

// Affine transformations for 2D mode

// Apply pre-affine transformation (2D)
// Standard affine formula: x' = ax + by + e, y' = cx + dy + f
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f
    );
}

// Apply post-affine transformation (2D)
// Same simultaneous affine formula, applied after variations
fn apply_post_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
        xform.post_c * p.x + xform.post_d * p.y + xform.post_f
    );
}

fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;
}

// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)
// When has_dc=true, takes a `vc` pointer (the iteration-local color register
// Apophysis calls `vc`) so DC variations (writes_color: true) can write to it.
// When has_rgb=true, takes a `vrc` pointer (the direct-RGB register) so
// variations with `Feature::WritesRgb` can write a vec3<f32>. Both gated
// independently — a flame mixing DC and RGB variations gets both pointers.
// When has_dc=false, the parameter is omitted — no DC variation in the active
// set means no inner call references vc, so it's pure overhead.
fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>, hide: ptr<function, bool>) -> vec2<f32> {
    var temp = p;

    // Phase 2: Precalculation handled per-variation

    // Phase 3: Normal variations (weighted sum from modified input)
    var result = vec2<f32>(0.0, 0.0);

    // 0: Linear (NORMAL)
    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_linear(temp);
    }

    return result;
}

// Per-flame packed get_param: each active variation has its own
// contiguous slot range in variation_params, with offsets baked
// at flame compile time. See build_packed_get_param in
// shader_builder_v2.rs.
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    var offset: u32 = 0u;
    switch (variation_id) {
        case 0u: { offset = 0u; }  // linear: 0 slots
        default: { offset = 0u; }
    }
    return variation_params[xform_id].params[offset + param_slot];
}


// Complex arithmetic + 2x2 complex matrix helpers for variations that
// operate over the Riemann sphere (Möbius transformations, Kleinian
// groups, complex trig/exp variants).
//
// Convention: a Complex is a vec2<f32> with .x = real, .y = imaginary.
// Functions are namespaced with `c` prefix (cmul, cdiv, csqrt, ...).
// 2×2 complex matrices use the `CMat2` struct with entries (a, b, c, d)
// representing [[a, b], [c, d]].
//
// f32 precision applies (no f64 in WGSL). Branch cuts follow standard
// principal-value conventions: csqrt returns the root with non-negative
// real part. Edge cases (cdiv by ~0, csqrt of 0) clamped via select to
// avoid NaN/Inf propagation; near-singular outputs are clipped naturally
// by the histogram.
//
// Seeded by arthomnix/fractal_viewer (MIT) for cmul/cdiv/csquare and
// DonKarlssonSan's GLSL gist as a textbook reference for csqrt.
// Implementation written from scratch; ~90 LoC.

fn cadd(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a + b;
}

fn csub(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a - b;
}

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn cmul_real(a: vec2<f32>, s: f32) -> vec2<f32> {
    return a * s;
}

fn cconj(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x, -a.y);
}

fn cmag2(a: vec2<f32>) -> f32 {
    // |a|² = ar² + ai² (cheaper than sqrt when only the squared
    // magnitude is needed, e.g., for cdiv).
    return a.x * a.x + a.y * a.y;
}

fn cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    // a / b = a · conj(b) / |b|²
    let denom = cmag2(b);
    let safe_denom = select(denom, 1e-30, denom < 1e-30);
    return cmul(a, cconj(b)) / safe_denom;
}

fn csquare(a: vec2<f32>) -> vec2<f32> {
    // (ar + ai·i)² = (ar² - ai²) + 2·ar·ai·i
    return vec2<f32>(a.x * a.x - a.y * a.y, 2.0 * a.x * a.y);
}

fn csqrt(a: vec2<f32>) -> vec2<f32> {
    // Principal branch: result has non-negative real part. Standard
    // formula via |a| split:
    //   r = sqrt((|a| + ar) / 2)
    //   i = sign(ai) · sqrt((|a| - ar) / 2)
    // Edge case: a = 0 returns (0, 0).
    let mag = sqrt(cmag2(a));
    let real_part = sqrt(max(0.5 * (mag + a.x), 0.0));
    let imag_mag = sqrt(max(0.5 * (mag - a.x), 0.0));
    let imag_part = select(-imag_mag, imag_mag, a.y >= 0.0);
    return vec2<f32>(real_part, imag_part);
}

// 2×2 complex matrix:  [[a, b], [c, d]]
struct CMat2 {
    a: vec2<f32>,
    b: vec2<f32>,
    c: vec2<f32>,
    d: vec2<f32>,
}

fn cmat2_make(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>) -> CMat2 {
    var m: CMat2;
    m.a = a;
    m.b = b;
    m.c = c;
    m.d = d;
    return m;
}

// Möbius transformation:  f(z) = (a·z + b) / (c·z + d)
fn cmat2_apply(m: CMat2, z: vec2<f32>) -> vec2<f32> {
    let num = cadd(cmul(m.a, z), m.b);
    let den = cadd(cmul(m.c, z), m.d);
    return cdiv(num, den);
}

// Inverse for an SL(2,ℂ) matrix (determinant = 1):  [[d, -b], [-c, a]].
// Kleinian generator matrices are normalized to det = 1 by construction
// (Indra's Pearls Ch. 4), so this shortcut suffices for klein_group and
// related ports. For general 2×2 complex matrices the full formula
// would divide by det — out of scope here.
fn cmat2_inverse_sl2(m: CMat2) -> CMat2 {
    return cmat2_make(m.d, vec2<f32>(-m.b.x, -m.b.y), vec2<f32>(-m.c.x, -m.c.y), m.a);
}

// Utility functions shared by 2D and 3D shaders
//
// NOTE: `get_param` is generated per-flame by the shader builder
// (see `build_packed_get_param` in src/shader_builder_v2.rs) and
// injected before this file is included. The injected version uses
// per-variation packed offsets baked from the active variation set,
// so each variation occupies exactly `parameters.len() +
// init_param_count` slots in the variation_params buffer instead of
// a fixed 16.

// Select transform based on cumulative weights (uses params.num_transforms)
fn select_transform(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight
    for (var i = 0u; i < params.num_transforms; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < params.num_transforms; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return params.num_transforms - 1u;
}

// Select transform using hard-coded NUM_TRANSFORMS constant
// Enables loop unrolling optimization by compiler
fn select_transform_const(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight (loop can be unrolled with known NUM_TRANSFORMS)
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// Select transform with xaos (chaos) weighting
// Uses hard-coded NUM_TRANSFORMS for loop unrolling
// prev_xform: Index of the transform that was just applied
// Xaos modifies probability: P(src→dst) = weight[dst] × xaos[src][dst]
fn select_transform_xaos(rand_val: f32, prev_xform: u32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Base index into xaos_weights array for this source transform
    let xaos_base = prev_xform * NUM_TRANSFORMS;

    // Calculate total modified weight
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        total_weight += base_weight * xaos_modifier;
    }

    let threshold = rand_val * total_weight;

    // Select based on modified weights
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        cumulative += base_weight * xaos_modifier;
        if (threshold <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// sRGB → linear decoding. Palette colors (.flame XML hex, .palette JSON)
// are authored against an sRGB monitor; the pipeline math is linear. Use
// the gamma-2.2 approximation so encode (pow 1/2.2 at fragment-shader tail)
// composed with decode is identity on round-tripped values.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Convert speed to color using palette lookup
fn speed_to_color(speed: f32) -> vec3<f32> {
    // Normalize speed to [0, 1] range
    // Use logarithmic scale for better visualization
    let normalized_speed = clamp(log(speed * 10.0 + 1.0) / 3.0, 0.0, 1.0);

    // Sample from palette texture (2D texture with height=1, so y=0.5)
    let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(normalized_speed, 0.5), 0.0).rgb;
    return srgb_to_linear(srgb);
}

// Build the 4-angle Apophysis / JWildfire camera matrix.
//
// Originally a verbatim transcription of `createProjectionMatrix`
// from `output/FlameRendererView.java`. That turned out to be subtly
// wrong: JWF *applies* its matrix transposed (`applyCameraMatrix`
// multiplies point components against matrix COLUMNS, see
// FlameRendererView.java lines 262-266), while our `camera_transform`
// applies rows. Transposing a product reverses the factor order, so
// a verbatim copy applied row-wise reverses the rotation composition.
// The empirical yaw↔roll slot swap at the call site repaired the two
// OUTER factors, but left bank and pitch in reversed relative order —
// bank acted outside pitch (world-axis-ish behavior) instead of
// inside (camera-axis-ish). Symptom: at pitch 90° the Bank slider
// yawed around world-Z in our app while JWF rolls around the look
// axis. At pitch 0 the two orders coincide, which is why this
// survived the original axis-by-axis slider tuning.
//
// This body now constructs the factorization directly:
//
//   M(yaw, pitch, bank, roll) = Rz(−yaw)·Rx(−pitch)·Ry(−bank)·Rz(−roll)
//
// which, with the call-site slot mapping (yaw slot ← −rotation,
// pitch ← −pitch, bank ← −bank, roll slot ← yaw), yields the
// effective world→camera chain
//
//   M_eff = Rz(rotation)·Rx(pitch)·Ry(bank)·Rz(−yaw)
//
// — exactly JWF's effective (transposed) chain
// Rz(camRoll)·Rx(camPitch)·Ry(camBank)·Rz(−camYaw). With bank = 0
// this is algebraically identical to the old transcription, so all
// existing flames without bank render unchanged. Verified numerically
// against the JWF source formula + transposed application.
//
// All four angles in radians. Each WGSL column holds
// `(m[0][col], m[1][col], m[2][col])` of the row-major matrix, same
// storage convention as before.
fn build_camera_matrix(yaw: f32, pitch: f32, bank: f32, roll: f32) -> mat3x3<f32> {
    let sy = sin(yaw);
    let cy = cos(yaw);
    let sp = sin(pitch);
    let cp = cos(pitch);
    let sb = sin(bank);
    let cb = cos(bank);
    let sr = sin(roll);
    let cr = cos(roll);

    return mat3x3<f32>(
        // Column 0 — m[0][0], m[1][0], m[2][0]
        vec3<f32>(
             cy*cb*cr - sy*cp*sr + sy*sp*sb*cr,
            -sy*cb*cr - cy*cp*sr + cy*sp*sb*cr,
             sp*sr + cp*sb*cr
        ),
        // Column 1 — m[0][1], m[1][1], m[2][1]
        vec3<f32>(
             cy*cb*sr + sy*cp*cr + sy*sp*sb*sr,
            -sy*cb*sr + cy*cp*cr + cy*sp*sb*sr,
            -sp*cr + cp*sb*sr
        ),
        // Column 2 — m[0][2], m[1][2], m[2][2]
        vec3<f32>(
            -cy*sb + sy*sp*cb,
             sy*sb + cy*sp*cb,
             cp*cb
        )
    );
}

// Apply Apophysis camera transformation: translate by camera position,
// then rotate. Uses ALL nine matrix elements — the pre-port 2-angle
// matrix had `m[2][0] = 0` always, so this function used to drop the
// `m[2][0]·z_translated` contribution to x entirely. The new 4-angle
// matrix populates `m[2][0] = cos(bank)·sin(pitch)·sin(roll) +
// cos(roll)·sin(bank)`, so the term has to come back.
//
// The translation was Z-only pre-PR ("camera moves up and down");
// now it's the full position vector (camera_x, camera_y, camera_z)
// so the camera can be anywhere in world space. Existing flames have
// camera_x = camera_y = 0 by default so behavior is preserved.
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_pos: vec3<f32>) -> vec3<f32> {
    let p_t = p - camera_pos;
    let x = camera_matrix[0][0] * p_t.x + camera_matrix[1][0] * p_t.y + camera_matrix[2][0] * p_t.z;
    let y = camera_matrix[0][1] * p_t.x + camera_matrix[1][1] * p_t.y + camera_matrix[2][1] * p_t.z;
    let z = camera_matrix[0][2] * p_t.x + camera_matrix[1][2] * p_t.y + camera_matrix[2][2] * p_t.z;
    return vec3<f32>(x, y, z);
}

// Apply Apophysis perspective projection.
// Source: Apophysis ControlPoint.pas:606
//
// The classic Apo formula `zr = 1 - persp · z` is a stylistic effect,
// not a true perspective matrix. For points the camera can see
// (camera-space z < 0 in our convention) `zr > 1` and behavior is
// sensible. For points behind the focal plane, however, `zr` shrinks
// to zero (singularity at z = 1/persp) and then flips sign, which
// mirrors the point onto the opposite side of the screen — the
// "wraparound where the sky lands above the floor" artifact that
// becomes obvious at persp ≥ ~0.5.
//
// We clip points where zr drops below a small positive threshold:
// returning an out-of-frame sentinel makes the downstream bounds
// check drop the sample naturally. This matches JWildfire behavior
// (Apo had the same wraparound bug; we deliberately don't reproduce
// it here). Existing Apo flames at typical persp ≈ 0.1 are unaffected
// because their content never gets close to the z = 10 singularity.
//
// TODO: expose this as a "Behind-camera wraparound (Apo-compatible)"
// toggle in the View panel if anyone needs the original Apo behavior
// back — e.g., a flame author who deliberately leans on the
// wraparound as a stylistic effect. Default would stay clipped.
fn apply_perspective(p: vec3<f32>, persp_strength: f32) -> vec2<f32> {
    if (abs(persp_strength) < 1e-6) {
        // Orthographic: no perspective
        return p.xy;
    }

    // Apophysis formula: zr = 1 - cameraPersp * z
    let zr = 1.0 - persp_strength * p.z;

    // Clip behind-camera and near-focal-plane points. The threshold
    // is small but positive so we drop both the singularity itself
    // and the extreme-magnification halo just in front of it.
    if (zr < 1e-3) {
        return vec2<f32>(1e30, 1e30);
    }

    // Perspective divide: x' = x / zr, y' = y / zr
    return p.xy / zr;
}

// Complete Apophysis 3D → 2D projection pipeline. Takes all four
// camera rotation angles (yaw, pitch, bank, roll) plus camera height
// and perspective strength. JWildfire's reference path negates yaw
// at the call site — we mirror that here so on-disk values render
// the same in both apps.
fn project_3d_to_2d_apophysis(
    p: vec3<f32>,
    pitch: f32,
    yaw: f32,
    bank: f32,
    roll: f32,
    camera_pos: vec3<f32>,
    persp_strength: f32
) -> vec2<f32> {
    // Convention mapping — routes our slider inputs to the right
    // matrix slots. Two adjustments vs. the naive (yaw, pitch, bank,
    // roll) pass-through:
    //
    //   1. **Yaw ↔ roll slot swap.** Our `yaw` parameter goes into
    //      the matrix's `roll` slot, and our `roll` parameter goes
    //      into the matrix's `yaw` slot. Root cause (confirmed from
    //      JWF source, `FlameRendererView.applyCameraMatrix`): JWF
    //      applies its matrix TRANSPOSED, which reverses the factor
    //      order; the swap re-lands the two outer factors. The
    //      middle factors (bank vs. pitch) couldn't be fixed by any
    //      slot routing — that order is corrected inside
    //      `build_camera_matrix` itself (see its comment).
    //
    //   2. **Sign tuning.** Each angle's input is negated or
    //      passed direct to match JWildfire's per-slider direction
    //      empirically. The negation pattern was tuned by testing
    //      each slider against JWildfire's renderer one axis at a
    //      time:
    //
    //          yaw    → roll slot,  direct
    //          pitch  → pitch slot, negated
    //          bank   → bank slot,  negated
    //          roll   → yaw slot,   direct
    //
    // Combined, this gives slider behavior matching JWildfire and
    // our pre-branch app for all four axes.
    let camera_matrix = build_camera_matrix(
        roll,     // matrix's `yaw` slot ← our `roll` input
        -pitch,
        -bank,
        yaw,      // matrix's `roll` slot ← our `yaw` input
    );
    let camera_space = camera_transform(p, camera_matrix, camera_pos);
    return apply_perspective(camera_space, persp_strength);
}

// Convert 3D fractal coords to pixel coords (with Apophysis camera system).
//
// `params.rotation` (XML `rotate`) is deliberately NOT passed into
// the camera matrix here. The roll factor sits outermost in the
// camera chain (`Rz(R)·Rx(P)·Ry(B)·Rz(−Y)`) and never touches
// camera-space z, so it commutes exactly with the perspective
// divide: rolling inside the matrix ≡ rotating the projected 2D
// point. We exploit that to apply rotation AFTER the pan
// subtraction, with the identical composition (and identical code)
// as the 2D `world_to_pixel`:
//
//     pan → rotate → zoom
//
// This makes Pan X/Y mean the same thing in both render modes —
// a pre-rotation position in the fractal plane, the Apophysis
// convention — so toggling 2D/3D never shifts the view when pan and
// rotation are both set. (JWildfire's 3D path instead pans in
// screen-aligned post-projection coordinates, which is why JWF
// flames jump when perspective crosses zero. We diverge from JWF
// deliberately here; pitch/yaw/bank behavior is unaffected because
// the outermost roll composes after them either way.)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    return project_3d_full(p).pixel;
}

// Full 3D projection result: the pixel plus the camera-space position it
// was projected from. The camera_space here is ROLL-LESS (params.rotation
// is applied post-projection, see world_to_pixel_3d's doc above) — but the
// outermost roll is a screen-plane rotation that never touches camera-space
// z, so this single camera_space serves every per-sample depth consumer
// (depth-density compensation, DoF, far-density fade, fog, and solid
// rendering's depth buffer). Those blocks previously each rebuilt an
// equivalent matrix per splat; z is roll-invariant so the values are
// identical.
struct Projection3D {
    pixel: vec2<i32>,
    camera_space: vec3<f32>,
}

fn project_3d_full(p: vec3<f32>) -> Projection3D {
    // Same matrix world_to_pixel_3d always built via
    // project_3d_to_2d_apophysis (roll = 0.0 in the matrix; see the slot
    // mapping comments there).
    let camera_matrix = build_camera_matrix(
        0.0,                        // roll applied post-projection below
        -params.camera_rotation_x,  // pitch
        -params.camera_bank,        // bank
         params.camera_rotation_y,  // matrix roll slot ← our yaw
    );
    let camera_space = camera_transform(
        p,
        camera_matrix,
        vec3<f32>(params.camera_x, params.camera_y, params.camera_z)
    );
    let p2d = apply_perspective(camera_space, params.perspective_strength);

    // Pan, rotate, zoom — mirrors `world_to_pixel` (2D) exactly.
    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);

    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return Projection3D(vec2<i32>(i32(pixel.x), i32(pixel.y)), camera_space);
}

// Convert 2D fractal coords to pixel coords
fn world_to_pixel(p: vec2<f32>) -> vec2<i32> {
    // Apply view transform: pan, rotation, and zoom
    var transformed = p - vec2<f32>(params.pan_x, params.pan_y);

    // Apply rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    // Apply zoom
    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Apply post-symmetry to a 3D world-space sample. `k` indexes the
// symmetry copy: 0 = original (returned as-is), 1..K-1 = mirrored or
// rotated copies depending on `params.post_symmetry.kind`.
//
//   1 (XAxis): reflect across y = center_y → shift the copy by
//              (distance, 0) → rotate it around the center by
//              `rotation` radians. K = 2 (one mirror).
//   2 (YAxis): reflect across x = center_x → shift by (0, distance)
//              → rotate around center. K = 2.
//   3 (Point): rotate around (center_x, center_y) by k × 2π / order.
//              K = order.
//
// Z passes through unchanged — post-symmetry is a 2D operation
// applied to XY before the camera transform.
//
// Live behind `HAS_POST_SYMMETRY`: when the flame has no symmetry
// active, this function isn't referenced and the shader builder strips
// it from the compiled module.
fn post_symmetry_copy(p: vec3<f32>, k: u32) -> vec3<f32> {
    let cx = params.post_symmetry.center_x;
    let cy = params.post_symmetry.center_y;
    let kind = params.post_symmetry.kind;

    if (kind == 0u) {
        return p;
    }

    if (kind == 3u) {
        // Point mode: k=0 returns p unchanged (no rotation); k≥1
        // rotates around the center by k × 2π / order. Distance and
        // rotation aren't part of Point mode — only `order` and
        // `center` matter (matches JWildfire).
        if (k == 0u) {
            return p;
        }
        let order = max(params.post_symmetry.order, 1u);
        let theta = f32(k) * 6.28318530717958647692 / f32(order);
        let cs = cos(theta);
        let sn = sin(theta);
        let dx = p.x - cx;
        let dy = p.y - cy;
        return vec3<f32>(dx * cs - dy * sn + cx, dx * sn + dy * cs + cy, p.z);
    }

    // Axis modes (1 = XAxis math, 2 = YAxis math).
    //
    // We use the standard math-class convention: the named axis is
    // the *line of reflection*. XAxis reflects across the X axis
    // (horizontal line y = center_y), flipping Y; YAxis reflects
    // across the Y axis (vertical line x = center_x), flipping X.
    // JWildfire's `.flame` XML uses the opposite naming (their
    // `X_AXIS` is our YAxis); the swap lives in
    // `PostSymmetryType::xml_token`.
    //
    // Distance and rotation produce OPPOSITE effects on the original
    // and the mirror — the original moves +distance / rotates +θ,
    // the mirror moves −distance / rotates −θ. The mirror line stays
    // in place (still y = cy for XAxis, x = cx for YAxis), so the
    // symmetry across the named axis is preserved no matter how
    // distance and rotation are adjusted. Matches JWildfire's
    // "the symmetry is preserved along the axis" behavior.
    //
    // The trick: apply distance pan and rotation to `p` first, then
    // for k=1 reflect at the end. Algebraically, reflecting after
    // applying (+d, +θ) is equivalent to applying (−d, −θ) and then
    // reflecting `p`, so the mirror sees the opposite-direction
    // perturbation for free.
    var x = p.x;
    var y = p.y;

    // Pan along the flip direction (perpendicular to the mirror line).
    // JWildfire stores `distance` as the *total separation* between
    // the original and the mirror; the per-copy pan is `distance / 2`.
    // See JWildfire's `PostAxisSymmetryWFFunc.init` —
    // `_halve_dist = pAmount / 2.0` — used as `+halve_dist` for the
    // original and `-halve_dist` for the mirror, giving total
    // separation `distance`. Pre-fix we panned by the full `distance`,
    // doubling the visible spread vs JWildfire at the same numeric
    // input. We split by 2 here so on-disk values round-trip 1:1
    // with JWildfire (no import/export multiplier needed).
    let half_dist = params.post_symmetry.distance * 0.5;
    if (kind == 1u) {
        // XAxis flips Y → distance pans along Y.
        y = y + half_dist;
    } else {
        // YAxis flips X → distance pans along X.
        x = x + half_dist;
    }

    // Rotation around the center. JWildfire's PostAxisSymmetryWFFunc
    // uses a clockwise rotation matrix (`[cos sin; -sin cos]`); we
    // use the standard math-convention counterclockwise
    // (`[cos -sin; sin cos]`). Negate the angle here so the on-disk
    // `rotation_deg` value round-trips with JWildfire at the same
    // sign — pre-fix, JWildfire's `-15°` looked like our `+15°`.
    // No `/2` factor on rotation (unlike distance) — JWildfire's init
    // does `a = rotation_deg × π/180`, just degrees-to-radians.
    let a = -params.post_symmetry.rotation;
    let cs = cos(a);
    let sn = sin(a);
    let dx = x - cx;
    let dy = y - cy;
    x = dx * cs - dy * sn + cx;
    y = dx * sn + dy * cs + cy;

    // Reflect for k=1. This inverts the prior pan and rotation for
    // the mirror copy in one step (see the algebra note above).
    if (k == 1u) {
        if (kind == 1u) {
            y = 2.0 * cy - y;
        } else {
            x = 2.0 * cx - x;
        }
    }

    return vec3<f32>(x, y, p.z);
}

// atan2 with IEEE behaviour at the origin, for variation code that can
// reach atan2(0, 0).
//
// Metal compiles shaders with fast-math (wgpu never clears
// `fastMathEnabled`, which defaults to true), and its fast atan2
// returns pi/4 at the origin. Probed on an M2: that is the ONLY input
// where it diverges from IEEE — everything else agrees to 1 ulp. pi/4
// is a plausible finite value, so no bad-value guard downstream can
// tell it from a real angle; it silently relocates the point. It cost
// npolar 73% of apo-misc7's lit pixels, because npolar at its default
// parity reaches (0, 0) on every call.
//
// The branch reproduces IEEE exactly, which is NOT "return 0": the
// result depends on the signs of the two zeros.
//
//   atan2(+0, +0) = +0     atan2(+0, -0) = +pi
//   atan2(-0, +0) = -0     atan2(-0, -0) = -pi
//
// Collapsing all four to 0 changes the render on Vulkan, where atan2
// is already IEEE — measured, it moved the image. Getting this right
// is what makes the guard a bit-exact no-op on platforms that never
// needed it. The sign bit is read via bitcast because `x < 0.0` is
// false for -0.0, and because integer ops are immune to fast-math.
//
// Not substituted globally: one comparison per call costs ~5% across
// the full variation set, and probing all 646 variations found only
// npolar, ho and log_db reaching the origin at default parameters.
// Call this where (0, 0) is reachable; plain atan2 is fine elsewhere.
fn ff_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}

// Main compute shader template
// This template generates variants via conditional compilation:
//   - 2D mode (vec2 points) vs 3D mode (vec3 points)
//   - Simple (no path tracking) vs Full (with path tracking)
//   - Standard vs Xaos (chaos-weighted transform selection)
//
// Conditional markers:
//    ... 
//   
//    ... 
//
// Uses hard-coded constants (compiled at shader build time):
//   NUM_TRANSFORMS, COLOR_MODE, HAS_POST_AFFINE
// These enable dead code elimination and loop unrolling optimizations.

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])


    var current = vec2<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );





    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;  // For palette mode

    // Fuse (burn-in) countdown: plot only when 0. Starts at the
    // configured burn-in and is RESET by the bad-value respawn below
    // (JWF re-fuses re-randomized points the same way), so a respawned
    // point re-converges onto the attractor before contributing.
    var fuse = params.burn_in;







    // Per-thread state initialization for stateful variations that need
    // values beyond zero-fill (var<private> thread_state is already zeroed
    // by WGSL spec; this block runs the wgsl_state_init fragments declared
    // by active variations). Emitted by shader builder; empty for flames
    // with no custom-init variations.


    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Save old position for speed calculation
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);

        // Standard: uses hard-coded NUM_TRANSFORMS for loop unrolling
        let xform_idx = select_transform_const(rand_val);

        let xform = transforms[xform_idx];

        // Opacity check (stochastic transparency)
        // Note: We still apply the transform even when opacity=0 - opacity affects
        // visibility only, not IFS dynamics. Transform must update position for correct chaos game.
        var should_plot = rng_nextf(&rng) < xform.opacity;

        // doHide flag (JWildfire's pVarTP.doHide), reset each iteration. The
        // cut_* family of CanHide variations set it via the `hide` pointer
        // threaded into apply_variations; a true value suppresses this
        // iteration's splat (the chaos game still advances). See the
        // CanHide feature + the gate after the transform chain below.
        var should_hide = false;



        // Analytic-blur mean-splat accumulator: the selected transform's
        // analytic-blur variation (if any) writes its weighted offset
        // `w·offset` here; otherwise it stays zero. Reset each iteration. The
        // plot routes the deterministic mean to this transform's blur buffer.
        // Only declared (and threaded into apply_variations) when the feature
        // is active — a non-blur build is byte-identical to one without it.
        // See docs/projects/analytic-blur-buffer.md.





        // Apply NORMAL transform: affine + variations + post-affine.
        // The 4-way call shape (HAS_DC × HAS_RGB) keeps each combination
        // as its own variant rather than an inline conditional, because
        // the template processor only does whole-block substitution.
        let affine_p = apply_affine(xform, current);


        current = apply_variations(xform, xform_idx, affine_p, &rng, &should_hide);


        if (HAS_POST_AFFINE) {
            if (xform.post_enabled > 0.5) {
                current = apply_post_affine(xform, current);
            }
        }



        // A Normal- or Linked-phase CanHide (cut_*) variation removed this
        // point: the cut_* family returns the origin (0,0) when it hides, and
        // because they are Replace variations that becomes `current`. Letting
        // it feed forward makes the chaos game keep re-entering the origin and
        // pile extremely dense splats at the origin's images. Revert to the
        // pre-iteration point (so a hiding cut transform is a no-op for the
        // chaos game) and skip this iteration's splat. Final-phase hides are
        // applied later and only suppress the splat — they never reach here,
        // so `current` is untouched (Finals don't feed forward by design).
        if (should_hide) {
            current = old_pos;
            continue;
        }

        // flam3/JWF-style bad-value recovery, in two tiers:
        //
        // X/Y divergence → full respawn + re-fuse, like JWF's
        // validateState() → preFuseIter() (x, y ∈ [-1, 1], z = 0).
        // The magnitude check (rather than NaN compares, which WGSL
        // compilers may assume away) catches growth before it reaches
        // f32 infinity and poisons everything downstream.

        let z_railed_respawn = false;

        let bad_value = z_railed_respawn ||
                        !(abs(current.x) <= 1e32) ||
                        !(abs(current.y) <= 1e32);
        if (bad_value) {

            current = vec2<f32>(
                rng_nextf(&rng) * 2.0 - 1.0,
                rng_nextf(&rng) * 2.0 - 1.0
            );


            fuse = params.burn_in;
            continue;
        }

        // After Linked chain: current = P_linked (feeds forward as
        // next iteration's input). Speed and color flow use P_linked.
        let speed = length(current - old_pos);


        // Original Step 1 (palette mode), or speed-based color (speed mode).
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (COLOR_MODE == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }


        // Note: COLOR_MODE == 2 (PathMap) uses the full shader with path tracking




        // Skip burn-in / re-fuse iterations (the respawn above resets
        // the countdown so recovering points don't plot mid-flight)
        if (fuse > 0u) {
            fuse = fuse - 1u;
        } else {

            // No attachments: skip the chain — plot the post-Linked
            // (== post-Normal) point directly.
            let final_pos = current;



            // doHide gate: a CanHide (cut_*) variation anywhere in this
            // iteration's transform chain marks the point as cut — skip the
            // splat (all post-symmetry copies included) while the chaos game
            // continues from final_pos.
            if (should_hide) {
                should_plot = false;
            }


            // Post-symmetry — gated entirely by HAS_POST_SYMMETRY.
            // When false, sym_count is 1u and the loop runs exactly
            // once with sym_k=0 (passes final_pos through unchanged),
            // letting the compiler fully strip it. When true,
            // sym_count is 1 (None at runtime — shouldn't happen given
            // the compile-time gate, but defensive), 2 (axis modes),
            // or `order` (Point mode), and each iteration plots one
            // mirrored/rotated copy of the same sample.

            let sym_count = 1u;


            // Pre-compute the iteration's base color OUTSIDE the
            // symmetry loop. It depends only on color_index / color /
            // (none for path-map) — none of which change between the
            // K symmetric copies. Hoisting the palette texture sample
            // alone gives a (K-1)/K speedup for palette mode at high
            // Point-symmetry orders. Default of white covers the
            // path-map COLOR_MODE branch (and any unhandled mode);
            // fog inside the loop reads from this base into a local
            // copy so its per-copy depth modulation doesn't bleed.
            var base_final_color: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);
            if (COLOR_MODE == 0u) {
                let palette_srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                base_final_color = srgb_to_linear(palette_srgb);
            } else if (COLOR_MODE == 1u) {
                base_final_color = color;
            }




            let plot_src = final_pos;

            for (var sym_k: u32 = 0u; sym_k < sym_count; sym_k = sym_k + 1u) {

                let plot_pos = plot_src;


            // Per-sample histogram weight (1.0 = neutral). Multi-emit
            // seeds it with the emission's weight; 3D depth-density
            // compensation below multiplies on top.

            var density_weight = 1.0;



            // Convert to pixel coordinates

            let pixel = world_to_pixel(plot_pos);




            // Check bounds and opacity (only plot if both pass)
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height) && should_plot) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Per-copy local copy of the hoisted base color. We
                // need a `var` so fog (below) can modulate it without
                // affecting the next symmetric copy's plot.
                var final_color: vec3<f32> = base_final_color;






                // Sample-emit path (multi-tile render). Stream one Sample
                // per plotted point to a buffer; a host-driven accumulate
                // pass scatters samples into per-tile histograms. Used when
                // the full-resolution histogram exceeds the storage-buffer
                // binding size limit. See
                // docs/projects/unified-render-pipeline.md.
                //
                // Allocate a slot via atomic counter and write the sample.
                // Same plot-time coords + final_color the direct path uses
                // — keeps feature parity (DOF, fog, opacity, path map, etc.)
                // identical between the two output strategies.
                let sample_idx = atomicAdd(&sample_counter.count, 1u);
                if (sample_idx < arrayLength(&samples)) {
                    samples[sample_idx] = Sample(
                        f32(pixel.x),
                        f32(pixel.y),
                        clamp(final_color.r, 0.0, 1.0),
                        clamp(final_color.g, 0.0, 1.0),
                        clamp(final_color.b, 0.0, 1.0),

                        density_weight, 0.0, 0.0

                    );
                }

            }
            }  // end for (sym_k = 0..sym_count) — post-symmetry loop

        }


    }
}
