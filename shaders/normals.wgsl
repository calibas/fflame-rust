// Solid-rendering normal estimation pass (Phase 1, à-trous pipeline).
//
// Extracts the shade pass's bilateral slope fit into its own full-image
// pass: for every pixel with depth, estimate the camera-space normal
// from the encoded nearest-depth field and write (normal.xyz, depth) to
// an Rgba32Float texture. Pixels without depth write (0,0,0, 3e38).
//
// The à-trous smoothing passes (atrous.wgsl) then refine this texture
// before the shade pass consumes it. Shares the ShadeParams uniform (and
// its buffer) with shade.wgsl — declared identically here.

struct ShadeLight {
    dir_intensity: vec4<f32>,
    color: vec4<f32>,
}

struct ShadeParams {
    width: u32,
    height: u32,
    zoom: f32,
    rotation: f32,
    pan_x: f32,
    pan_y: f32,
    perspective_strength: f32,
    shading_strength: f32,
    ambient: f32,
    diffuse: f32,
    specular: f32,
    shininess: f32,
    ssao_strength: f32,
    ssao_radius: f32,
    surface_thickness: f32,
    depth_word_offset: u32,
    tex_y0: u32,
    tex_height: u32,
    _pad0: u32,           // use_normal_tex (unused here)
    _pad1: u32,           // gap_fill (unused here)
    // Phase 2 camera + volume block (unused here; layout must match
    // shade.wgsl — same buffer): 4 camera vec4s + 4 volume scalars.
    _pad_cam0: vec4<f32>,
    _pad_cam1: vec4<f32>,
    _pad_cam2: vec4<f32>,
    _pad_cam3: vec4<f32>,
    _pad_vol: vec4<f32>,
    lights: array<ShadeLight, 4>,
}

@group(0) @binding(0) var<storage, read> histogram: array<u32>;
@group(0) @binding(1) var<uniform> sp: ShadeParams;
@group(0) @binding(2) var normals_out: texture_storage_2d<rgba32float, write>;

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

fn reconstruct(px: f32, py: f32, d: f32) -> vec3<f32> {
    let scale = f32(min(sp.width, sp.height)) * 0.25;
    let center = vec2<f32>(f32(sp.width), f32(sp.height)) * 0.5;
    var t = (vec2<f32>(px + 0.5, py + 0.5) - center) / scale;
    t = t / max(sp.zoom, 1e-6);
    let c = cos(-sp.rotation);
    let s = sin(-sp.rotation);
    t = vec2<f32>(t.x * c - t.y * s, t.x * s + t.y * c);
    t = t + vec2<f32>(sp.pan_x, sp.pan_y);
    let zr = 1.0 + sp.perspective_strength * d;
    return vec3<f32>(t * zr, -d);
}

@compute @workgroup_size(8, 8, 1)
fn normals_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= sp.width || gid.y >= sp.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    let d = depth_at(px, py);
    if (d > 1.0e37) {
        textureStore(normals_out, vec2<i32>(px, py), vec4<f32>(0.0, 0.0, 0.0, 3.0e38));
        return;
    }

    // Bilateral slope fit (see shade.wgsl's inline variant for the full
    // rationale): average in-window per-pixel depth slopes over a 9x9
    // neighborhood; the window rejects other surfaces at silhouettes.
    let win = max(sp.surface_thickness, 0.005) * 3.0;
    var sum_x = 0.0;
    var w_x = 0.0;
    var sum_y = 0.0;
    var w_y = 0.0;
    for (var oy = -4; oy <= 4; oy = oy + 1) {
        for (var ox = -4; ox <= 4; ox = ox + 1) {
            let nd = depth_at(px + ox, py + oy);
            if (nd > 1.0e37 || abs(nd - d) > win) {
                continue;
            }
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

    let pos = reconstruct(f32(px), f32(py), d);
    let tangent_x = reconstruct(f32(px + 1), f32(py), d + dzdx) - pos;
    let tangent_y = reconstruct(f32(px), f32(py + 1), d + dzdy) - pos;
    var n = cross(tangent_y, tangent_x);
    if (n.z < 0.0) {
        n = -n;
    }
    let nlen = length(n);
    if (nlen < 1e-12) {
        textureStore(normals_out, vec2<i32>(px, py), vec4<f32>(0.0, 0.0, 1.0, d));
        return;
    }
    textureStore(normals_out, vec2<i32>(px, py), vec4<f32>(n / nlen, d));
}
