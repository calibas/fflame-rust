// Solid-rendering deferred shade pass (splat-native lighting).
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

    // Effective camera matrix rows (world→camera: cam = E·(w − cam_pos),
    // E orthonormal), so world = cam.x·row0 + cam.y·row1 + cam.z·row2 +
    // cam_pos. Host computes E exactly as project_3d_full does
    // (utilities.wgsl build_camera_matrix + camera_transform's transposed
    // application). Used by the shadow-map lookup (world-space
    // reconstruction of the shaded point).
    cam_row0: vec4<f32>,
    cam_row1: vec4<f32>,
    cam_row2: vec4<f32>,
    cam_pos: vec4<f32>,
    // Shadow-map strength (0 = off).
    shadow_strength: f32,
    // Interactive temporal smoothing (0 = off): final = mix(current,
    // previous frame's shade output, this). During progressive
    // accumulation the shading tracks genuinely drifting data —
    // per-frame raw it reads as patchy strobing; blended over ~7 frames
    // it's a calm drift. Exports and the first post-reset frames pass 0.
    temporal_ema: f32,
    _pad_sp0: f32,
    _pad_sp1: f32,

    // Light-space shadow maps (Stage 2): ortho fit (xyz center,
    // w radius) matching the splat's frozen fit exactly.
    shadow_fit: vec4<f32>,
    // Word offset of map 0 inside the histogram binding; maps are
    // shadow_res² words each, light i ↔ map i. shadow_count = 0
    // disables the lookup (exports without maps, shadows off).
    shadow_word_offset: u32,
    shadow_res: u32,
    shadow_count: u32,
    _pad_sm: u32,

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

// ── Camera helpers (shadow-map lookup) ──

fn cam_to_world(c: vec3<f32>) -> vec3<f32> {
    return c.x * sp.cam_row0.xyz + c.y * sp.cam_row1.xyz + c.z * sp.cam_row2.xyz
        + sp.cam_pos.xyz;
}

// Rotation-only variants (E is orthonormal: inverse = transpose).
fn dir_to_world(c: vec3<f32>) -> vec3<f32> {
    return c.x * sp.cam_row0.xyz + c.y * sp.cam_row1.xyz + c.z * sp.cam_row2.xyz;
}
// Light-space shadow-map factor for light `li` at world position
// `wpos` (lw = world-space direction TO the light): 1 = fully lit,
// 0 = fully occluded. 4-tap PCF against the splat-resolution ortho
// depth map in the histogram tail — shadows resolve at SPLAT
// resolution, which no affordable voxel grid can deliver. The basis
// derivation must match shadow_map_splat in core/header.wgsl exactly.
fn shadow_map_factor(wpos: vec3<f32>, lw: vec3<f32>, li: u32) -> f32 {
    let rel = wpos - sp.shadow_fit.xyz;
    var bu = cross(lw, vec3<f32>(0.0, 0.0, 1.0));
    if (dot(bu, bu) < 1e-6) {
        bu = cross(lw, vec3<f32>(1.0, 0.0, 0.0));
    }
    bu = normalize(bu);
    let bv = cross(lw, bu);
    let r = max(sp.shadow_fit.w, 1e-6);
    let res = f32(sp.shadow_res);
    let mu = (dot(rel, bu) / r * 0.5 + 0.5) * res;
    let mv = (dot(rel, bv) / r * 0.5 + 0.5) * res;
    if (mu < 0.0 || mu >= res || mv < 0.0 || mv >= res) {
        return 1.0;
    }
    let my_d = dot(rel, lw);
    // Bias ~2.5 texels of world size: below it, surface samples shadow
    // themselves (acne); far above it, shadows detach (peter-panning).
    let bias = 2.5 * (2.0 * r / res);
    var lit = 0.0;
    for (var t = 0u; t < 4u; t = t + 1u) {
        let ox = select(-0.75, 0.75, (t & 1u) != 0u);
        let oy = select(-0.75, 0.75, (t & 2u) != 0u);
        let tx = u32(clamp(mu + ox, 0.0, res - 1.0));
        let ty = u32(clamp(mv + oy, 0.0, res - 1.0));
        let enc = histogram[sp.shadow_word_offset + li * sp.shadow_res * sp.shadow_res + ty * sp.shadow_res + tx];
        if (enc == 0u) {
            lit = lit + 1.0;
            continue;
        }
        let ord = select(~enc, enc & 0x7FFFFFFFu, (enc & 0x80000000u) != 0u);
        let occ_d = bitcast<f32>(ord);
        lit = lit + select(0.0, 1.0, occ_d <= my_d + bias);
    }
    return lit * 0.25;
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

    // ── Depth-buffer shading (splat-native pipeline) ─────────────────
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

    // Blinn-Phong with camera-space directional lights, plus
    // splat-resolution shadow maps (Stage 2).
    let v = normalize(-pos);
    var lit = albedo * (sp.ambient * ao);
    let p1_shadows = sp.shadow_count > 0u && sp.shadow_strength > 0.0;
    var p1_wpos = vec3<f32>(0.0);
    if (p1_shadows) {
        p1_wpos = cam_to_world(pos);
    }
    for (var li = 0; li < 4; li = li + 1) {
        let intensity = sp.lights[li].dir_intensity.w;
        if (intensity <= 0.0) {
            continue;
        }
        let l = sp.lights[li].dir_intensity.xyz;
        let lcol = sp.lights[li].color.rgb * intensity;
        let ndotl = max(dot(n, l), 0.0);
        var shadow = 1.0;
        if (p1_shadows && ndotl > 0.0) {
            shadow = mix(1.0, shadow_map_factor(p1_wpos, dir_to_world(l), u32(li)), sp.shadow_strength);
        }
        lit = lit + albedo * lcol * (sp.diffuse * ndotl * ao * shadow);
        if (sp.specular > 0.0 && ndotl > 0.0) {
            let hh = normalize(l + v);
            let spec = pow(max(dot(n, hh), 0.0), max(sp.shininess, 1.0));
            lit = lit + lcol * (sp.specular * spec * shadow);
        }
    }

    let rgb = mix(albedo, lit, clamp(sp.shading_strength, 0.0, 1.0));
    shade_store(lx, ly, vec4<f32>(rgb, alpha_out));
}
