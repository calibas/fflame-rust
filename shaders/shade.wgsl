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
    _pad1: u32,

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

    let d = depth_at(px, py);
    if (d > 1.0e37) {
        // No surface here — emissive pass-through.
        textureStore(shade_out, vec2<i32>(lx, ly), accum);
        return;
    }

    let pos = reconstruct(f32(px), f32(py), d);

    var n: vec3<f32>;
    if (sp.use_normal_tex != 0u) {
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
    let albedo = accum.rgb;
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
            let h = normalize(l + v);
            let spec = pow(max(dot(n, h), 0.0), max(sp.shininess, 1.0));
            lit = lit + lcol * (sp.specular * spec);
        }
    }

    let rgb = mix(albedo, lit, clamp(sp.shading_strength, 0.0, 1.0));
    textureStore(shade_out, vec2<i32>(lx, ly), vec4<f32>(rgb, accum.a));
}
