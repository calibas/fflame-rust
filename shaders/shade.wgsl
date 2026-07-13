// Solid-rendering deferred shade pass (Phase 1 lighting + Phase 2
// density-volume consumption).
//
// Runs between accumulate and tonemap (via tonemap_pass_with_input, like
// density effects): reads the accumulator (rgb = density-weighted mean
// flame color = "albedo", a = density) plus the per-pixel nearest-depth
// region the SOLID splat path maintains inside the histogram buffer, and
// writes a shaded Rgba32Float image (alpha passed through untouched so
// the tonemap's log-density math is unaffected).
//
// Per pixel:
//   1. decode nearest depth (inverted ordered-float encoding; 0 = no
//      sample → emissive pass-through),
//   2. reconstruct the camera-space position (inverting the Apophysis
//      projection: zr = 1 + persp·d, camera xy = projected xy · zr),
//   3. estimate a camera-space normal from depth differences (picking
//      the smaller of forward/backward differences per axis to avoid
//      silhouette smearing),
//   4. SSAO: spiral depth taps, occluded when a neighbor is closer than
//      the center by more than a normal-dependent bias,
//   5. Blinn-Phong: ambient + up to 4 directional lights (directions
//      precomputed host-side in camera space) with diffuse + specular,
//   6. final rgb = mix(albedo, lit, shading_strength).
//
// All reads are textureLoad / raw buffer indexing — no samplers, no
// FLOAT32_FILTERABLE dependency, WASM-clean.

struct ShadeLight {
    // xyz = normalized camera-space direction TO the light, w = intensity
    // (0 when the light is disabled — host bakes enabled into intensity).
    dir_intensity: vec4<f32>,
    // rgb = light color (linear), w unused.
    color: vec4<f32>,
}

struct ShadeParams {
    // FULL image dimensions — reconstruction and depth indexing always
    // work in full-image coordinates, even when shading a strip.
    width: u32,
    height: u32,
    // View-transform inverse inputs (must match utilities.wgsl
    // world_to_pixel_3d / project_3d_full).
    zoom: f32,
    rotation: f32,
    pan_x: f32,
    pan_y: f32,
    perspective_strength: f32,
    // Master emissive↔lit blend (0 = pass-through; the host skips the
    // whole pass at 0).
    shading_strength: f32,

    ambient: f32,
    diffuse: f32,
    specular: f32,
    shininess: f32,

    ssao_strength: f32,   // 0 = off
    ssao_radius: f32,     // world units at the surface
    // Depth-noise scale for the bilateral smooth: the nearest-depth field
    // carries Monte-Carlo variance on the order of the surface shell, so
    // neighbors within ~2 shells of the center are averaged before
    // normals are taken from the smoothed field.
    surface_thickness: f32,
    // Word offset of the depth region inside the bound buffer:
    // interactive = W*H*4 (depth region inside the histogram binding);
    // export = 0 (a dedicated full-image depth buffer).
    depth_word_offset: u32,

    // Region rendering (strip-tiled exporters): the bound input/output
    // textures cover full-width rows [tex_y0, tex_y0 + tex_height).
    // Interactive: tex_y0 = 0, tex_height = height.
    tex_y0: u32,
    tex_height: u32,
    // 1 = read normals from the pre-smoothed (normals + à-trous) texture
    // at binding 4; 0 = estimate inline (strip-tiled exports, where the
    // full-image normal textures don't exist). The texture is full-image
    // sized and indexed with GLOBAL coordinates.
    use_normal_tex: u32,
    // Surface closing (0 = off): fill pixels with NO sample when a ring
    // of neighbors within this radius agree they are one surface (valid
    // depths, tight spread) — closes the see-through pinholes a sparse
    // chaos game leaves in solid surfaces. Requires use_normal_tex.
    gap_fill: u32,

    // ── Phase 2 density volume ──
    // Effective camera matrix rows (world→camera: cam = E·(w − cam_pos),
    // E orthonormal), so world = cam.x·row0 + cam.y·row1 + cam.z·row2 +
    // cam_pos. Host computes E exactly as project_3d_full does
    // (utilities.wgsl build_camera_matrix + camera_transform's transposed
    // application). Only read when volume_dim > 0.
    cam_row0: vec4<f32>,
    cam_row1: vec4<f32>,
    cam_row2: vec4<f32>,
    cam_pos: vec4<f32>,
    // World-space center of the grid cube (view-fit mode tracks the
    // world point at screen center; manual mode is the origin).
    vol_center: vec4<f32>,
    // Grid resolution per axis; 0 = volume absent → every volume feature
    // (gradient normals, volumetric AO, shadow march, occlusion repair)
    // compiles to the Phase 1 behavior.
    volume_dim: u32,
    volume_extent: f32,
    // Normalizer for raw voxel counts: rho_norm = count · this. Host
    // bakes it from the accumulated splat total so rho_norm ≈ 1 marks
    // "solid" density regardless of how long the render has run.
    vol_density_scale: f32,
    // Volume shadow-march strength (0 = off; host pre-multiplies by
    // vol_trust).
    shadow_strength: f32,
    // How well the grid resolves the current view, 0-1 (host-computed
    // from voxels-across-the-visible-width). Every volume feature scales
    // with it, so a too-coarse grid (manual extent on a zoomed-in scene)
    // degrades gracefully to the Phase 1 look instead of stamping
    // voxel-scale artifacts on the image.
    vol_trust: f32,
    // Interactive temporal smoothing (0 = off): final = mix(current,
    // previous frame's shade output, this). During progressive
    // accumulation the volume-derived shading tracks genuinely drifting
    // data — per-frame raw it reads as patchy strobing; blended over
    // ~7 frames it's a calm drift. Exports and the first post-reset
    // frames pass 0.
    temporal_ema: f32,
    // Volume-primary march: opacity dial (FractalConfig::solid_strength;
    // 1 = hard surface, lower = translucent emission/absorption).
    solid_strength: f32,
    // Scene-mean per-pixel density (host: Σaccum.a / pixels) — the
    // tonemap alpha assigned to base-coat-only pixels, and the detail
    // layer's confidence scale.
    base_alpha: f32,

    lights: array<ShadeLight, 4>,
}

@group(0) @binding(0) var accum_tex: texture_2d<f32>;
// The full histogram buffer; only the depth region (offset W*H*4 words)
// is read here. Non-atomic read-only view of the same buffer the splat
// pass writes with atomics — the shade pass runs after the batch's
// compute+accumulate, so there are no concurrent writers.
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var<uniform> sp: ShadeParams;
@group(0) @binding(3) var shade_out: texture_storage_2d<rgba32float, write>;
// Pre-smoothed (normal.xyz, depth) from normals.wgsl + atrous.wgsl.
// Bound to a 1x1 dummy when use_normal_tex == 0.
@group(0) @binding(4) var normal_tex: texture_2d<f32>;
// Previous frame's shade output (ping-pong partner of shade_out).
// 1×1 dummy when temporal_ema == 0 — never read then.
@group(0) @binding(7) var prev_shade: texture_2d<f32>;

// Phase 2 density fields, derived from the raw splat grid each shade by
// volume_mip.wgsl (half resolution, raw-count scale, f32). The raw grid
// is NOT sampled here: at view-fit resolution it carries per-voxel
// Poisson noise (patchy lighting) and cell-faceted gradients
// (rectangular shading blocks). Bound to 4-byte dummies when
// volume_dim == 0 — never indexed then.
// Smoothed (4³-window mean): gradient normals, AO taps, shadow march.
@group(0) @binding(5) var<storage, read> vol_avg: array<f32>;
// Morphologically closed max field: the volume-primary march's opacity
// (σ) source — holes in the splatted shell up to ~2×volume_closing
// half-res voxels read as sealed.
@group(0) @binding(6) var<storage, read> vol_closed: array<f32>;
// Half-res mean color (rgb in [0,1], w = mean count): the march's
// base-coat color. 4-byte dummy when volume_dim == 0.
@group(0) @binding(8) var<storage, read> vol_color: array<vec4<f32>>;

// ── Phase 2 volume helpers (all callers gate on sp.volume_dim > 0) ──

fn cam_to_world(c: vec3<f32>) -> vec3<f32> {
    return c.x * sp.cam_row0.xyz + c.y * sp.cam_row1.xyz + c.z * sp.cam_row2.xyz
        + sp.cam_pos.xyz;
}

// Rotation-only variants (E is orthonormal: inverse = transpose).
fn dir_to_world(c: vec3<f32>) -> vec3<f32> {
    return c.x * sp.cam_row0.xyz + c.y * sp.cam_row1.xyz + c.z * sp.cam_row2.xyz;
}
fn dir_to_cam(w: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(sp.cam_row0.xyz, w), dot(sp.cam_row1.xyz, w), dot(sp.cam_row2.xyz, w));
}

// Derived-field grid resolution per axis.
fn vol_hd() -> u32 {
    return max(sp.volume_dim / 2u, 1u);
}

// Trilinear normalized density from the CLOSED field — the
// volume-primary march's opacity (σ) source.
fn vol_closed_density(w: vec3<f32>) -> f32 {
    let ve = sp.volume_extent;
    let r = w - sp.vol_center.xyz;
    if (abs(r.x) >= ve || abs(r.y) >= ve || abs(r.z) >= ve) {
        return 0.0;
    }
    let hd = vol_hd();
    let vd = i32(hd);
    let g = (r / ve * 0.5 + vec3<f32>(0.5)) * f32(hd) - vec3<f32>(0.5);
    let gf = floor(g);
    let f = g - gf;
    let i0 = vec3<i32>(gf);
    var sum = 0.0;
    for (var c = 0; c < 8; c = c + 1) {
        let o = vec3<i32>(c & 1, (c >> 1) & 1, (c >> 2) & 1);
        let i = clamp(i0 + o, vec3<i32>(0), vec3<i32>(vd - 1));
        let wgt = mix(vec3<f32>(1.0) - f, f, vec3<f32>(o));
        sum = sum + vol_closed[(u32(i.z) * hd + u32(i.y)) * hd + u32(i.x)]
            * wgt.x * wgt.y * wgt.z;
    }
    return sum * sp.vol_density_scale;
}

// Trilinear base-coat color (rgb in [0,1]); zero outside the grid.
fn vol_color_at(w: vec3<f32>) -> vec3<f32> {
    let ve = sp.volume_extent;
    let r = w - sp.vol_center.xyz;
    if (abs(r.x) >= ve || abs(r.y) >= ve || abs(r.z) >= ve) {
        return vec3<f32>(0.0);
    }
    let hd = vol_hd();
    let vd = i32(hd);
    let g = (r / ve * 0.5 + vec3<f32>(0.5)) * f32(hd) - vec3<f32>(0.5);
    let gf = floor(g);
    let f = g - gf;
    let i0 = vec3<i32>(gf);
    var csum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var c = 0; c < 8; c = c + 1) {
        let o = vec3<i32>(c & 1, (c >> 1) & 1, (c >> 2) & 1);
        let i = clamp(i0 + o, vec3<i32>(0), vec3<i32>(vd - 1));
        let wgt0 = mix(vec3<f32>(1.0) - f, f, vec3<f32>(o));
        let v = vol_color[(u32(i.z) * hd + u32(i.y)) * hd + u32(i.x)];
        // Weight corners by their density too, so empty voxels don't
        // drag the color toward black at shell boundaries.
        let wgt = wgt0.x * wgt0.y * wgt0.z * (v.w + 1e-3);
        csum = csum + v.rgb * wgt;
        wsum = wsum + wgt;
    }
    return csum / max(wsum, 1e-6);
}

// Trilinear normalized density from the SMOOTHED field (gradient
// normals, AO taps, shadow march); 0 outside.
fn vol_density(w: vec3<f32>) -> f32 {
    let ve = sp.volume_extent;
    let r = w - sp.vol_center.xyz;
    if (abs(r.x) >= ve || abs(r.y) >= ve || abs(r.z) >= ve) {
        return 0.0;
    }
    let hd = vol_hd();
    let vd = i32(hd);
    let g = (r / ve * 0.5 + vec3<f32>(0.5)) * f32(hd) - vec3<f32>(0.5);
    let gf = floor(g);
    let f = g - gf;
    let i0 = vec3<i32>(gf);
    var sum = 0.0;
    for (var c = 0; c < 8; c = c + 1) {
        let o = vec3<i32>(c & 1, (c >> 1) & 1, (c >> 2) & 1);
        let i = clamp(i0 + o, vec3<i32>(0), vec3<i32>(vd - 1));
        let wgt = mix(vec3<f32>(1.0) - f, f, vec3<f32>(o));
        sum = sum + vol_avg[(u32(i.z) * hd + u32(i.y)) * hd + u32(i.x)]
            * wgt.x * wgt.y * wgt.z;
    }
    return sum * sp.vol_density_scale;
}

// Per-pixel hash in [0, 1) — decorrelates march sampling between
// neighboring pixels so voxel-scale stepping reads as fine noise
// instead of bands.
fn shade_hash(px: i32, py: i32) -> f32 {
    var h = u32(px) * 374761393u + u32(py) * 668265263u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 0xFFFFu) / 65536.0;
}

// Decode the splat path's inverted ordered-float depth encoding.
// Returns the out-of-band sentinel 3e38 ("no sample") for empty pixels —
// NOT a sign check: legitimate depths are negative whenever geometry sits
// behind the camera plane (orthographic renders don't clip it, and any
// object straddling z = 0 has a negative-depth front surface).
fn depth_at(px: i32, py: i32) -> f32 {
    if (px < 0 || py < 0 || px >= i32(sp.width) || py >= i32(sp.height)) {
        return 3.0e38;
    }
    let idx = sp.depth_word_offset + u32(py) * sp.width + u32(px);
    let enc = histogram[idx];
    if (enc == 0u) {
        return 3.0e38;
    }
    let ord = ~enc;
    let bits = select(~ord, ord ^ 0x80000000u, (ord & 0x80000000u) != 0u);
    return bitcast<f32>(bits);
}

// Camera-space position for a pixel at depth d — the inverse of
// project_3d_full's pixel mapping. Camera looks down -z; the point sits
// at camera z = -d.
fn reconstruct(px: f32, py: f32, d: f32) -> vec3<f32> {
    let scale = f32(min(sp.width, sp.height)) * 0.25;
    let center = vec2<f32>(f32(sp.width), f32(sp.height)) * 0.5;
    var t = (vec2<f32>(px + 0.5, py + 0.5) - center) / scale;
    t = t / max(sp.zoom, 1e-6);
    let c = cos(-sp.rotation);
    let s = sin(-sp.rotation);
    t = vec2<f32>(t.x * c - t.y * s, t.x * s + t.y * c);
    t = t + vec2<f32>(sp.pan_x, sp.pan_y);
    // Perspective divide inversion: projected = camera_xy / zr with
    // zr = 1 - persp·camera_z = 1 + persp·d.
    let zr = 1.0 + sp.perspective_strength * d;
    return vec3<f32>(t * zr, -d);
}

// Volume-primary march: front-to-back emission/absorption through the
// CLOSED density field with base-coat colors from the color field. This
// IS the solid renderer's geometry and occlusion — "is there a surface
// here" is an integral, never a threshold, so the hole/repair/banding
// failure modes of reconciling a sparse depth buffer with a volume by
// rules simply don't exist. The histogram's per-pixel detail is layered
// on top by the caller (base coat from voxels, detail from iterations).
struct MarchResult {
    rgb: vec3<f32>,      // transmittance-weighted mean base color
    coverage: f32,       // 1 − final transmittance
    d_surf: f32,         // depth at opacity 0.5 (or weighted mean)
    // Transmittance-weighted mean UNCLAMPED normalized density along
    // the contributing path (1 ≈ solid). Restores the flame's
    // log-density dynamic range in the tonemap: base-coat pixels
    // through dust get proportionally lower alpha than the dense core
    // instead of a flat scene-mean brightness.
    density: f32,
}

fn volume_march(px: f32, py: f32, jitter: f32) -> MarchResult {
    var res: MarchResult;
    res.rgb = vec3<f32>(0.0);
    res.coverage = 0.0;
    res.d_surf = 3.0e38;
    res.density = 0.0;
    let o_c = reconstruct(px, py, 0.0);
    let r_c = reconstruct(px, py, 1.0) - o_c;
    let o_w = cam_to_world(o_c);
    let r_w = dir_to_world(r_c);           // NOT unit length: param = depth
    let ve = sp.volume_extent;
    let o_rel = o_w - sp.vol_center.xyz;
    var d0 = -3.0e38;
    var d1 = 3.0e38;
    for (var a = 0; a < 3; a = a + 1) {
        let ro = o_rel[a];
        let rd = r_w[a];
        if (abs(rd) < 1e-12) {
            if (abs(ro) >= ve) {
                return res;
            }
            continue;
        }
        let t0 = (-ve - ro) / rd;
        let t1 = (ve - ro) / rd;
        d0 = max(d0, min(t0, t1));
        d1 = min(d1, max(t0, t1));
    }
    if (d1 <= d0) {
        return res;
    }
    let voxel = 2.0 * ve / f32(vol_hd());
    let wlen = max(length(r_w), 1e-9);
    let step_d = 0.75 * voxel / wlen;
    // Opacity per voxel-length of solid density: solid_strength 1 makes
    // a solid voxel saturate within ~a step (hard surface); lower values
    // are translucent emission/absorption. Density is OPACITY-CLAMPED —
    // solid is solid; a 1000× filament is not 1000× solid.
    let sigma_k = mix(0.25, 4.0, clamp(sp.solid_strength, 0.0, 1.0)) * 0.75;
    var t_ = 1.0;
    var csum = vec3<f32>(0.0);
    var wsum = 0.0;
    var dsum = 0.0;
    var densum = 0.0;
    var d = d0 + step_d * (0.25 + 0.5 * jitter);
    for (var i = 0; i < 192; i = i + 1) {
        if (d >= d1) {
            break;
        }
        let w = o_w + r_w * d;
        let sig_raw = vol_closed_density(w);
        let sig = min(sig_raw, 1.5);
        if (sig > 0.002) {
            let a_step = 1.0 - exp(-sig * sigma_k);
            let contrib = t_ * a_step;
            csum = csum + vol_color_at(w) * contrib;
            wsum = wsum + contrib;
            dsum = dsum + d * contrib;
            densum = densum + min(sig_raw, 8.0) * contrib;
            t_ = t_ * (1.0 - a_step);
            if (res.d_surf > 1.0e37 && t_ < 0.5) {
                res.d_surf = d;
            }
            if (t_ < 0.01) {
                break;
            }
        }
        d = d + step_d;
    }
    res.coverage = 1.0 - t_;
    if (wsum > 1e-6) {
        res.rgb = csum / wsum;
        res.density = densum / wsum;
        if (res.d_surf > 1.0e37) {
            res.d_surf = dsum / wsum;
        }
    }
    return res;
}

// Final write with optional temporal blend against the previous
// frame's shade output.
fn shade_store(lx: i32, ly: i32, v: vec4<f32>) {
    var out = v;
    if (sp.temporal_ema > 0.0) {
        let prev = textureLoad(prev_shade, vec2<i32>(lx, ly), 0);
        out = mix(v, prev, sp.temporal_ema);
    }
    textureStore(shade_out, vec2<i32>(lx, ly), out);
}

@compute @workgroup_size(8, 8, 1)
fn shade_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= sp.width || gid.y >= sp.tex_height) {
        return;
    }
    // Local (texture) vs global (image) coordinates: albedo reads and the
    // output write are texture-local; depth, reconstruction, and SSAO use
    // full-image coordinates so strips shade identically to a one-shot.
    let lx = i32(gid.x);
    let ly = i32(gid.y);
    let px = lx;
    let py = i32(gid.y + sp.tex_y0);
    let accum = textureLoad(accum_tex, vec2<i32>(lx, ly), 0);

    var d = depth_at(px, py);

    // ── Volume-primary path (Phase 3) ────────────────────────────────
    // The march IS the solid renderer: geometry and occlusion come from
    // the closed density field (base coat — voxels fill ~1000× faster
    // than pixels, so the surface is complete and hole-free long before
    // per-pixel coverage converges), and the pixel's own chaos-game data
    // is layered on top as the DETAIL pass wherever it exists at (or in
    // front of) the base surface. "Is there a surface here" is an
    // integral, never a threshold — the hole / repair-seam / coverage-
    // transition failure modes of reconciling a sparse depth buffer with
    // volume rules by thresholds do not exist on this path.
    if (sp.volume_dim > 0u && sp.vol_trust > 0.0) {
        let mr = volume_march(f32(px), f32(py), shade_hash(px, py));
        let cov = mr.coverage * sp.vol_trust;
        if (cov < 0.005) {
            // Ray misses the volume — emissive passthrough (which may
            // itself be empty = background).
            shade_store(lx, ly, accum);
            return;
        }
        // Detail confidence: this pixel's accumulated density vs the
        // scene mean — a continuous base → detail hand-over as coverage
        // accumulates. Gated to samples at or in front of the base
        // surface so occluded back-structure never shows through.
        let voxel = 2.0 * sp.volume_extent / f32(vol_hd());
        let vol_margin = max(6.0 * max(sp.surface_thickness, 0.001), 4.0 * voxel);
        var detail_w = 0.0;
        if (d < 1.0e37 && d < mr.d_surf + vol_margin * 2.0) {
            detail_w = accum.a / (accum.a + 0.35 * max(sp.base_alpha, 1e-6));
        }
        let albedo2 = mix(mr.rgb, accum.rgb, detail_w);
        // Base-coat alpha scales with the ray's mean density so the
        // tonemap keeps the flame's dynamic range (dust dim, core
        // bright) while occlusion stays solid.
        let alpha2 = mix(sp.base_alpha * cov * clamp(mr.density, 0.02, 4.0), accum.a, detail_w);
        let dgeo = mix(mr.d_surf, min(d, mr.d_surf + vol_margin), detail_w);
        let pos = reconstruct(f32(px), f32(py), dgeo);

        // Normal: ∇ρ of the smooth field (world-space, camera-stable);
        // the screen-space normal folds in for the detail layer when the
        // full-image normal chain exists.
        let wpos = cam_to_world(pos);
        let h = 2.0 * sp.volume_extent / f32(vol_hd());
        let grad = vec3<f32>(
            vol_density(wpos + vec3<f32>(h, 0.0, 0.0)) - vol_density(wpos - vec3<f32>(h, 0.0, 0.0)),
            vol_density(wpos + vec3<f32>(0.0, h, 0.0)) - vol_density(wpos - vec3<f32>(0.0, h, 0.0)),
            vol_density(wpos + vec3<f32>(0.0, 0.0, h)) - vol_density(wpos - vec3<f32>(0.0, 0.0, h)),
        ) * 0.5;
        let gl = length(grad);
        var n = vec3<f32>(0.0, 0.0, 1.0);
        // Gradient confidence: where the field is surface-like, shade
        // lit; where it's dust/filaments the gradient is weak and the
        // pixel falls back toward pure emissive — visually correct
        // there (see 3d-volume-surface-planning.md).
        var lit_conf = 0.0;
        if (gl > 1e-6) {
            n = dir_to_cam(-grad / gl);
            if (n.z < 0.0) {
                n = -n;
            }
            lit_conf = clamp(gl * 2.0 / max(vol_density(wpos), 0.25), 0.0, 1.0);
        }
        if (sp.use_normal_tex != 0u && detail_w > 0.0) {
            let nt = textureLoad(normal_tex, vec2<i32>(px, py), 0);
            if (nt.w < 1.0e37 && length(nt.xyz) > 1e-6) {
                n = normalize(mix(n, nt.xyz, detail_w * 0.5));
                lit_conf = max(lit_conf, detail_w * 0.5);
            }
        }

        // Volumetric AO: hemisphere density taps, lifted off the shell
        // and per-pixel jittered (surface-scale features otherwise
        // darken voxel-shaped footprints).
        var ao = 1.0;
        if (sp.ssao_strength > 0.0) {
            let w0 = wpos + dir_to_world(n) * voxel * 2.0;
            let nw = dir_to_world(n);
            let ja = shade_hash(px, py) * 6.2831853;
            var t1 = cross(nw, vec3<f32>(0.0, 0.0, 1.0));
            if (dot(t1, t1) < 1e-6) {
                t1 = cross(nw, vec3<f32>(1.0, 0.0, 0.0));
            }
            t1 = normalize(t1);
            let t2 = cross(nw, t1);
            let jc = cos(ja);
            let js = sin(ja);
            let r1 = t1 * jc + t2 * js;
            let r2 = t2 * jc - t1 * js;
            let rr = max(sp.ssao_radius, 4.0 * voxel);
            var occ = clamp(vol_density(w0 + nw * rr), 0.0, 1.0);
            occ = occ + clamp(vol_density(w0 + normalize(nw + r1 * 1.2) * rr), 0.0, 1.0);
            occ = occ + clamp(vol_density(w0 + normalize(nw - r1 * 1.2) * rr), 0.0, 1.0);
            occ = occ + clamp(vol_density(w0 + normalize(nw + r2 * 1.2) * rr), 0.0, 1.0);
            occ = occ + clamp(vol_density(w0 + normalize(nw - r2 * 1.2) * rr), 0.0, 1.0);
            ao = 1.0 - sp.ssao_strength * sp.vol_trust * clamp(occ / 5.0, 0.0, 1.0);
        }

        // Blinn-Phong; each light's diffuse + specular attenuated by an
        // opacity-clamped shadow march through the smooth field.
        let v = normalize(-pos);
        var lit = albedo2 * (sp.ambient * ao);
        let do_shadow = sp.shadow_strength > 0.0;
        let sh_origin = wpos + dir_to_world(n) * voxel * 1.5;
        let sh_jitter = shade_hash(px, py);
        for (var li = 0; li < 4; li = li + 1) {
            let intensity = sp.lights[li].dir_intensity.w;
            if (intensity <= 0.0) {
                continue;
            }
            let l = sp.lights[li].dir_intensity.xyz;
            let lcol = sp.lights[li].color.rgb * intensity;
            let ndotl = max(dot(n, l), 0.0);
            var shadow = 1.0;
            if (do_shadow && ndotl > 0.0) {
                let lw = dir_to_world(l);
                var dens = 0.0;
                var t = voxel * (2.0 + 2.0 * sh_jitter);
                for (var st = 0; st < 32; st = st + 1) {
                    let swp = sh_origin + lw * t;
                    let srel = swp - sp.vol_center.xyz;
                    if (abs(srel.x) >= sp.volume_extent || abs(srel.y) >= sp.volume_extent
                        || abs(srel.z) >= sp.volume_extent) {
                        break;
                    }
                    let ramp = smoothstep(voxel * 2.0, voxel * 4.0, t);
                    dens = dens + min(vol_density(swp), 1.0) * 2.0 * ramp;
                    if (dens > 8.0) {
                        break;
                    }
                    t = t + voxel * 2.0;
                }
                shadow = mix(1.0, exp(-dens), sp.shadow_strength);
            }
            lit = lit + albedo2 * lcol * (sp.diffuse * ndotl * ao * shadow);
            if (sp.specular > 0.0 && ndotl > 0.0) {
                let hh = normalize(l + v);
                let spec = pow(max(dot(n, hh), 0.0), max(sp.shininess, 1.0));
                lit = lit + lcol * (sp.specular * spec * shadow);
            }
        }
        let lm = clamp(sp.shading_strength, 0.0, 1.0) * lit_conf;
        let rgb = mix(albedo2, lit, lm);
        shade_store(lx, ly, vec4<f32>(rgb, alpha2));
        return;
    }

    // ── Phase 1 path (volume off): depth-buffer shading ──────────────
    var albedo = accum.rgb;
    var alpha_out = accum.a;
    var fill_normal = vec3<f32>(0.0);
    var has_fill_normal = false;
    var filled = false;

    if (d > 1.0e37) {
        // No sample here. Surface closing: if a ring of neighbors agree
        // they're one continuous surface, synthesize this pixel from
        // them (density-weighted so sparse neighbors grow in smoothly) —
        // the chaos game leaves pinholes wherever the IFS measure is
        // thin, and those read as see-through geometry.
        if (sp.use_normal_tex != 0u && sp.gap_fill > 0u) {
            let win = max(sp.surface_thickness, 0.005) * 6.0;
            for (var g = 1; g <= i32(sp.gap_fill); g = g + 1) {
                var cnt = 0.0;
                var dmin = 3.0e38;
                var dmax = -3.0e38;
                var wsum = 0.0;
                var dsum = 0.0;
                var nsum = vec3<f32>(0.0);
                var asum_rgb = vec3<f32>(0.0);
                var asum_a = 0.0;
                for (var k = 0; k < 8; k = k + 1) {
                    var ox = 0; var oy = 0;
                    switch (k) {
                        case 0: { ox = g; oy = 0; }
                        case 1: { ox = -g; oy = 0; }
                        case 2: { ox = 0; oy = g; }
                        case 3: { ox = 0; oy = -g; }
                        case 4: { ox = g; oy = g; }
                        case 5: { ox = -g; oy = g; }
                        case 6: { ox = g; oy = -g; }
                        default: { ox = -g; oy = -g; }
                    }
                    let sx = px + ox;
                    let sy = py + oy;
                    if (sx < 0 || sy < 0 || sx >= i32(sp.width) || sy >= i32(sp.height)) {
                        continue;
                    }
                    let nt = textureLoad(normal_tex, vec2<i32>(sx, sy), 0);
                    if (nt.w > 1.0e37) {
                        continue;
                    }
                    let acc_n = textureLoad(accum_tex, vec2<i32>(lx + ox, ly + oy), 0);
                    let wn = acc_n.a;
                    cnt = cnt + 1.0;
                    dmin = min(dmin, nt.w);
                    dmax = max(dmax, nt.w);
                    wsum = wsum + wn;
                    dsum = dsum + nt.w * wn;
                    nsum = nsum + nt.xyz * wn;
                    asum_rgb = asum_rgb + acc_n.rgb * wn;
                    asum_a = asum_a + acc_n.a;
                }
                // Fill only when most of the ring is surface AND it's
                // ONE surface (tight depth spread) — silhouette gaps
                // between different surfaces stay open.
                if (cnt >= 5.0 && (dmax - dmin) < win && wsum > 1e-6) {
                    d = dsum / wsum;
                    let nl = length(nsum);
                    if (nl > 1e-9) {
                        fill_normal = nsum / nl;
                        has_fill_normal = true;
                    }
                    albedo = asum_rgb / wsum;
                    alpha_out = asum_a / cnt;
                    filled = true;
                    break;
                }
            }
        }
        if (!filled) {
            // Genuinely empty — emissive pass-through.
            shade_store(lx, ly, accum);
            return;
        }
    }

    let pos = reconstruct(f32(px), f32(py), d);

    var n: vec3<f32>;
    if (filled) {
        n = select(vec3<f32>(0.0, 0.0, 1.0), fill_normal, has_fill_normal);
    } else if (sp.use_normal_tex != 0u) {
        // Pre-computed + à-trous-smoothed normal (full-image texture,
        // global coordinates).
        n = textureLoad(normal_tex, vec2<i32>(px, py), 0).xyz;
        if (length(n) < 1e-6) {
            shade_store(lx, ly, accum);
            return;
        }
    } else {
    // Inline bilateral slope fit: 9x9 window of neighbors whose depth
    // sits within a few surface-shells of the center. The raw
    // nearest-depth field carries Monte-Carlo noise comparable to the
    // shell; the wide in-window average kills it without bleeding across
    // silhouette edges. Used by the strip-tiled export path (the
    // full-image normal textures don't exist there); the interactive and
    // single-shot paths use normals.wgsl + atrous.wgsl instead.
    let win = max(sp.surface_thickness, 0.005) * 3.0;
    var tangent_x: vec3<f32>;
    var tangent_y: vec3<f32>;
    {
        var sum_x = 0.0; var w_x = 0.0;
        var sum_y = 0.0; var w_y = 0.0;
        for (var oy = -4; oy <= 4; oy = oy + 1) {
            for (var ox = -4; ox <= 4; ox = ox + 1) {
                let nd = depth_at(px + ox, py + oy);
                if (nd > 1.0e37 || abs(nd - d) > win) {
                    continue;
                }
                // Accumulate weighted x/y-slope estimates: each in-window
                // neighbor contributes its per-pixel depth slope.
                if (ox != 0) {
                    sum_x = sum_x + (nd - d) / f32(ox);
                    w_x = w_x + 1.0;
                }
                if (oy != 0) {
                    sum_y = sum_y + (nd - d) / f32(oy);
                    w_y = w_y + 1.0;
                }
            }
        }
        let dzdx = select(0.0, sum_x / w_x, w_x > 0.0);
        let dzdy = select(0.0, sum_y / w_y, w_y > 0.0);
        // Tangents from the smoothed slopes, one pixel apart in screen
        // space, reconstructed into camera space.
        tangent_x = reconstruct(f32(px + 1), f32(py), d + dzdx) - pos;
        tangent_y = reconstruct(f32(px), f32(py + 1), d + dzdy) - pos;
    }
    var ni = cross(tangent_y, tangent_x);
    // Face the camera (camera at origin, looking down -z: normals should
    // have positive z toward the viewer).
    if (ni.z < 0.0) {
        ni = -ni;
    }
    let nlen = length(ni);
    if (nlen < 1e-12) {
        shade_store(lx, ly, accum);
        return;
    }
    n = ni / nlen;
    }

    // SSAO: 8 spiral taps. A tap occludes when its surface point is
    // closer to the camera than the center's tangent plane allows
    // (bias grows with slope), with a range falloff so distant
    // foreground objects don't darken the background.
    var ao = 1.0;
    if (sp.ssao_strength > 0.0) {
        let scale = f32(min(sp.width, sp.height)) * 0.25;
        let zr = 1.0 + sp.perspective_strength * d;
        let radius_px = max(sp.ssao_radius * scale * sp.zoom / max(zr, 1e-3), 1.5);
        var occl = 0.0;
        var taps = 0.0;
        for (var i = 0; i < 8; i = i + 1) {
            let ang = f32(i) * 2.39996323;          // golden angle spiral
            let r = radius_px * sqrt((f32(i) + 0.5) / 8.0);
            let sx = px + i32(round(cos(ang) * r));
            let sy = py + i32(round(sin(ang) * r));
            let sd = depth_at(sx, sy);
            if (sd > 1.0e37) {
                continue;
            }
            taps = taps + 1.0;
            let diff = d - sd;                       // >0: neighbor nearer
            let bias = 0.01 + 0.02 * sp.ssao_radius;
            if (diff > bias) {
                // Range check: fade occlusion from far-foreground objects.
                occl = occl + clamp(1.0 - (diff - bias) / (4.0 * sp.ssao_radius + 1e-6), 0.0, 1.0);
            }
        }
        if (taps > 0.0) {
            ao = 1.0 - sp.ssao_strength * (occl / taps);
        }
    }

    // Blinn-Phong with camera-space directional lights.
    let v = normalize(-pos);
    var lit = albedo * (sp.ambient * ao);
    for (var li = 0; li < 4; li = li + 1) {
        let intensity = sp.lights[li].dir_intensity.w;
        if (intensity <= 0.0) {
            continue;
        }
        let l = sp.lights[li].dir_intensity.xyz;
        let lcol = sp.lights[li].color.rgb * intensity;
        let ndotl = max(dot(n, l), 0.0);
        lit = lit + albedo * lcol * (sp.diffuse * ndotl * ao);
        if (sp.specular > 0.0 && ndotl > 0.0) {
            let hh = normalize(l + v);
            let spec = pow(max(dot(n, hh), 0.0), max(sp.shininess, 1.0));
            lit = lit + lcol * (sp.specular * spec);
        }
    }

    let rgb = mix(albedo, lit, clamp(sp.shading_strength, 0.0, 1.0));
    shade_store(lx, ly, vec4<f32>(rgb, alpha_out));
}
