// Solid-rendering deferred shade pass (Phase 1).
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
    // Grid resolution per axis; 0 = volume absent → every volume feature
    // (gradient normals, volumetric AO, shadow march, occlusion repair)
    // compiles to the Phase 1 behavior.
    volume_dim: u32,
    volume_extent: f32,
    // Normalizer for raw voxel counts: rho_norm = count · this. Host
    // bakes it from the accumulated splat total so rho_norm ≈ 1 marks
    // "solid" density regardless of how long the render has run.
    vol_density_scale: f32,
    // Volume shadow-march strength (0 = off).
    shadow_strength: f32,

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
// Phase 2 world-space density volume (volume_dim³ u32 counts). Bound to
// a 4-byte dummy when volume_dim == 0 — never indexed then.
@group(0) @binding(5) var<storage, read> vol: array<u32>;

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

// Nearest-voxel normalized density; 0 outside the grid.
fn vol_density_nearest(w: vec3<f32>) -> f32 {
    let ve = sp.volume_extent;
    if (abs(w.x) >= ve || abs(w.y) >= ve || abs(w.z) >= ve) {
        return 0.0;
    }
    let vd = sp.volume_dim;
    let vx = min(u32((w.x / ve * 0.5 + 0.5) * f32(vd)), vd - 1u);
    let vy = min(u32((w.y / ve * 0.5 + 0.5) * f32(vd)), vd - 1u);
    let vz = min(u32((w.z / ve * 0.5 + 0.5) * f32(vd)), vd - 1u);
    return f32(vol[(vz * vd + vy) * vd + vx]) * sp.vol_density_scale;
}

// Trilinear normalized density (for gradients and AO taps); 0 outside.
fn vol_density(w: vec3<f32>) -> f32 {
    let ve = sp.volume_extent;
    if (abs(w.x) >= ve || abs(w.y) >= ve || abs(w.z) >= ve) {
        return 0.0;
    }
    let vd = i32(sp.volume_dim);
    let g = (w / ve * 0.5 + vec3<f32>(0.5)) * f32(sp.volume_dim) - vec3<f32>(0.5);
    let gf = floor(g);
    let f = g - gf;
    let i0 = vec3<i32>(gf);
    var sum = 0.0;
    for (var c = 0; c < 8; c = c + 1) {
        let o = vec3<i32>(c & 1, (c >> 1) & 1, (c >> 2) & 1);
        let i = clamp(i0 + o, vec3<i32>(0), vec3<i32>(vd - 1));
        let wgt = mix(vec3<f32>(1.0) - f, f, vec3<f32>(o));
        sum = sum + f32(vol[(u32(i.z) * sp.volume_dim + u32(i.y)) * sp.volume_dim + u32(i.x)])
            * wgt.x * wgt.y * wgt.z;
    }
    return sum * sp.vol_density_scale;
}

// Camera depth where the view ray through pixel (px, py) first meets
// solid volume density; 3e38 when it never does. reconstruct() is linear
// in depth, so the camera ray is o_c + d·r_c and the parameter d IS the
// camera depth — no reparameterization needed after the world transform.
fn vol_ray_depth(px: f32, py: f32) -> f32 {
    let o_c = reconstruct(px, py, 0.0);
    let r_c = reconstruct(px, py, 1.0) - o_c;
    let o_w = cam_to_world(o_c);
    let r_w = dir_to_world(r_c);           // NOT unit length: param = depth
    // Slab-intersect the cube [-ve, ve]³ in the depth parameter.
    let ve = sp.volume_extent;
    var d0 = -3.0e38;
    var d1 = 3.0e38;
    for (var a = 0; a < 3; a = a + 1) {
        let ro = o_w[a];
        let rd = r_w[a];
        if (abs(rd) < 1e-12) {
            if (abs(ro) >= ve) {
                return 3.0e38;
            }
            continue;
        }
        let t0 = (-ve - ro) / rd;
        let t1 = (ve - ro) / rd;
        d0 = max(d0, min(t0, t1));
        d1 = min(d1, max(t0, t1));
    }
    if (d1 <= d0) {
        return 3.0e38;
    }
    // Fixed-step march, ~1.25 voxels per step measured in world space
    // (r_w is depth-parameterized, so convert). Integrated density trips
    // the surface at 1 voxel-equivalent of solid density — sparse shells
    // a little under solid still trigger within a few steps.
    let voxel = 2.0 * ve / f32(sp.volume_dim);
    let wlen = max(length(r_w), 1e-9);
    let step_d = 1.25 * voxel / wlen;
    var acc = 0.0;
    var d = d0 + step_d * 0.5;
    for (var i = 0; i < 160; i = i + 1) {
        if (d >= d1) {
            break;
        }
        acc = acc + vol_density_nearest(o_w + r_w * d) * 1.25;
        if (acc >= 1.0) {
            return d;
        }
        d = d + step_d;
    }
    return 3.0e38;
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
    var albedo = accum.rgb;
    var alpha_out = accum.a;
    var fill_normal = vec3<f32>(0.0);
    var filled = false;
    var use_vol_normal = false;

    // Phase 2 occlusion repair: ask the volume where the view ray FIRST
    // meets solid density. A pixel whose nearest sample sits well behind
    // that is a LEAK — its only samples belong to back structure showing
    // through a sparsely-covered front surface (per-pixel depth can't fix
    // those: there's nothing nearer to keep). Holes get the same
    // authority: the volume knows a surface crosses this pixel even when
    // zero samples landed on it.
    var d_vol = 3.0e38;
    var leak = false;
    var vol_margin = 0.0;
    if (sp.volume_dim > 0u) {
        let voxel = 2.0 * sp.volume_extent / f32(sp.volume_dim);
        vol_margin = max(6.0 * max(sp.surface_thickness, 0.001), 3.0 * voxel);
        d_vol = vol_ray_depth(f32(px), f32(py));
        leak = d < 1.0e37 && d_vol < d - vol_margin;
    }

    if (d > 1.0e37 || leak) {
        // Missing or wrong surface here. Ring fill: synthesize from
        // neighbors that agree they're one continuous surface — the chaos
        // game leaves pinholes wherever the IFS measure is thin, and those
        // read as see-through geometry. With volume authority the search
        // runs even at gap_fill 0, accepts only neighbors near the
        // volume's surface depth, and needs less consensus (the volume
        // already vouched a surface is here).
        let vol_backed = d_vol < 1.0e37;
        var rings = i32(sp.gap_fill);
        if (vol_backed) {
            rings = max(rings, 2);
        }
        // Leak pixels must be rebuilt from the FRONT surface — neighbors
        // at or behind the leaked depth would just re-import the leak.
        let d_reject = select(3.0e38, d - vol_margin, leak);
        if (sp.use_normal_tex != 0u && rings > 0) {
            let win = max(sp.surface_thickness, 0.005) * 6.0;
            let need = select(5.0, 3.0, vol_backed);
            for (var g = 1; g <= rings; g = g + 1) {
                var cnt = 0.0;
                var dmin = 3.0e38;
                var dmax = -3.0e38;
                var dsum = 0.0;
                var nsum = vec3<f32>(0.0);
                var asum = vec4<f32>(0.0);
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
                    if (nt.w > 1.0e37 || nt.w >= d_reject) {
                        continue;
                    }
                    // Volume-anchored: only members of the surface the
                    // volume found (rejects back-structure neighbors that
                    // leaked through nearby pixels too).
                    if (vol_backed && abs(nt.w - d_vol) > vol_margin * 2.0) {
                        continue;
                    }
                    cnt = cnt + 1.0;
                    dmin = min(dmin, nt.w);
                    dmax = max(dmax, nt.w);
                    dsum = dsum + nt.w;
                    nsum = nsum + nt.xyz;
                    asum = asum + textureLoad(accum_tex, vec2<i32>(lx + ox, ly + oy), 0);
                }
                // Fill only when enough of the ring is surface AND it's
                // ONE surface (tight depth spread) — silhouette gaps
                // between different surfaces stay open.
                if (cnt >= need && (dmax - dmin) < win) {
                    d = dsum / cnt;
                    let nl = length(nsum);
                    if (nl > 1e-9) {
                        fill_normal = nsum / nl;
                    } else {
                        fill_normal = vec3<f32>(0.0, 0.0, 1.0);
                    }
                    albedo = asum.rgb / cnt;
                    alpha_out = asum.a / cnt;
                    filled = true;
                    break;
                }
            }
        }
        if (!filled && leak) {
            // No usable front-surface ring, but the volume placed a
            // surface in front of the leaked samples: keep this pixel's
            // color, move its geometry to the volume surface and shade
            // with the density gradient — occluded correctly, color
            // approximate.
            d = d_vol;
            filled = true;
            use_vol_normal = true;
        }
        if (!filled) {
            // Genuinely empty (and the volume agrees, or is absent) —
            // emissive pass-through.
            textureStore(shade_out, vec2<i32>(lx, ly), accum);
            return;
        }
    }

    let pos = reconstruct(f32(px), f32(py), d);

    var n: vec3<f32>;
    if (use_vol_normal) {
        n = vec3<f32>(0.0, 0.0, 1.0);   // replaced by the gradient below
    } else if (filled) {
        n = fill_normal;
    } else if (sp.use_normal_tex != 0u) {
        // Pre-computed + à-trous-smoothed normal (full-image texture,
        // global coordinates).
        n = textureLoad(normal_tex, vec2<i32>(px, py), 0).xyz;
        if (length(n) < 1e-6) {
            textureStore(shade_out, vec2<i32>(lx, ly), accum);
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
        textureStore(shade_out, vec2<i32>(lx, ly), accum);
        return;
    }
    n = ni / nlen;
    }

    // Phase 2 gradient normals: where the volume sees a strong density
    // edge, -∇ρ is a world-space surface normal — stable under camera
    // motion and free of the screen-space estimator's silhouette and
    // Monte-Carlo artifacts. Blend by edge confidence (relative density
    // change per voxel) so flat/foggy interiors keep the screen normal.
    if (sp.volume_dim > 0u) {
        let w = cam_to_world(pos);
        let h = 2.0 * sp.volume_extent / f32(sp.volume_dim);
        let grad = vec3<f32>(
            vol_density(w + vec3<f32>(h, 0.0, 0.0)) - vol_density(w - vec3<f32>(h, 0.0, 0.0)),
            vol_density(w + vec3<f32>(0.0, h, 0.0)) - vol_density(w - vec3<f32>(0.0, h, 0.0)),
            vol_density(w + vec3<f32>(0.0, 0.0, h)) - vol_density(w - vec3<f32>(0.0, 0.0, h)),
        ) * 0.5;
        let gl = length(grad);
        if (gl > 1e-6) {
            var nv = dir_to_cam(-grad / gl);
            // Face the viewer (matches the screen-space estimator's
            // convention; keeps grazing surfaces lit sanely).
            if (nv.z < 0.0) {
                nv = -nv;
            }
            let conf = clamp(gl / max(vol_density(w), 0.25), 0.0, 1.0);
            let wv = select(conf, 1.0, use_vol_normal);
            n = normalize(mix(n, nv, wv));
        }
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

    // Phase 2 volumetric AO: hemisphere density taps in WORLD space —
    // matter above the surface occludes regardless of whether any pixel
    // sampled it (screen-space AO can only see what the depth buffer
    // caught). Composes multiplicatively with SSAO under the same
    // strength control.
    if (sp.ssao_strength > 0.0 && sp.volume_dim > 0u) {
        let w = cam_to_world(pos);
        let nw = dir_to_world(n);
        var t1 = cross(nw, vec3<f32>(0.0, 0.0, 1.0));
        if (dot(t1, t1) < 1e-6) {
            t1 = cross(nw, vec3<f32>(1.0, 0.0, 0.0));
        }
        t1 = normalize(t1);
        let t2 = cross(nw, t1);
        // Tap at the SSAO radius, at least a couple of voxels out so the
        // surface's own shell doesn't self-occlude.
        let r = max(sp.ssao_radius, 4.0 * sp.volume_extent / f32(sp.volume_dim));
        var occ = clamp(vol_density(w + nw * r), 0.0, 1.0);
        occ = occ + clamp(vol_density(w + normalize(nw + t1 * 1.2) * r), 0.0, 1.0);
        occ = occ + clamp(vol_density(w + normalize(nw - t1 * 1.2) * r), 0.0, 1.0);
        occ = occ + clamp(vol_density(w + normalize(nw + t2 * 1.2) * r), 0.0, 1.0);
        occ = occ + clamp(vol_density(w + normalize(nw - t2 * 1.2) * r), 0.0, 1.0);
        ao = ao * (1.0 - sp.ssao_strength * clamp(occ / 5.0, 0.0, 1.0));
    }

    // Blinn-Phong with camera-space directional lights.
    let v = normalize(-pos);
    var lit = albedo * (sp.ambient * ao);
    // Phase 2 shadow march setup (per-pixel invariants, hoisted).
    let do_shadow = sp.shadow_strength > 0.0 && sp.volume_dim > 0u;
    var sh_origin = vec3<f32>(0.0);
    var sh_voxel = 0.0;
    if (do_shadow) {
        sh_voxel = 2.0 * sp.volume_extent / f32(sp.volume_dim);
        // Start clear of the surface's own density shell.
        sh_origin = cam_to_world(pos) + dir_to_world(n) * sh_voxel * 1.5;
    }
    for (var li = 0; li < 4; li = li + 1) {
        let intensity = sp.lights[li].dir_intensity.w;
        if (intensity <= 0.0) {
            continue;
        }
        let l = sp.lights[li].dir_intensity.xyz;
        let lcol = sp.lights[li].color.rgb * intensity;
        let ndotl = max(dot(n, l), 0.0);
        // Phase 2 shadow march: integrate density toward the light;
        // transmittance attenuates this light's diffuse + specular.
        var shadow = 1.0;
        if (do_shadow && ndotl > 0.0) {
            let lw = dir_to_world(l);
            var dens = 0.0;
            var t = sh_voxel * 3.0;
            for (var s = 0; s < 32; s = s + 1) {
                let swp = sh_origin + lw * t;
                if (abs(swp.x) >= sp.volume_extent || abs(swp.y) >= sp.volume_extent
                    || abs(swp.z) >= sp.volume_extent) {
                    break;
                }
                dens = dens + vol_density_nearest(swp) * 2.0;
                if (dens > 8.0) {
                    break;      // already opaque — stop integrating
                }
                t = t + sh_voxel * 2.0;
            }
            shadow = mix(1.0, exp(-dens), sp.shadow_strength);
        }
        lit = lit + albedo * lcol * (sp.diffuse * ndotl * ao * shadow);
        if (sp.specular > 0.0 && ndotl > 0.0) {
            let h = normalize(l + v);
            let spec = pow(max(dot(n, h), 0.0), max(sp.shininess, 1.0));
            lit = lit + lcol * (sp.specular * spec * shadow);
        }
    }

    let rgb = mix(albedo, lit, clamp(sp.shading_strength, 0.0, 1.0));
    textureStore(shade_out, vec2<i32>(lx, ly), vec4<f32>(rgb, alpha_out));
}
