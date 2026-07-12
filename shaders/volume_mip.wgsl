// Solid-rendering Phase 2: density-volume derivation passes.
//
// The shade pass must NOT consume the raw splat grid directly:
//   - at view-fit resolution each voxel holds few samples, so raw
//     normals/AO/shadows resolve per-voxel Poisson noise ("patchy"
//     lighting), and
//   - the gradient of a trilinearly-sampled field is constant inside
//     each cell and jumps at cell faces, so raw gradients facet into
//     voxel-shaped patches ("rectangular shadows").
//
// Each shade therefore derives two half-resolution fields first:
//   reduce_main : raw (dim³ u32 counts, atomic-written by the splat)
//                 → avg  (hd³ f32, 4³-window mean — the SMOOTH field
//                         for gradient normals, AO taps, shadow march)
//                 → vmax (hd³ f32, 4³-window max — seed for closing)
//   dilate_main : vmax → tmp  (max over (2r+1)³ half-res voxels)
//   erode_main  : tmp  → out  (min over (2r+1)³)
// dilate+erode = morphological CLOSING: holes in the splatted shell up
// to ~2r half-res voxels read as sealed for the occlusion / repair ray
// march, while the erosion cancels the dilation's silhouette inflation
// everywhere except inside holes. r = 0 skips both passes (the shade
// pass then reads vmax directly). This invents surface where the IFS
// measure has none — an artistic dial (`volume_closing`), not data
// recovery.
//
// All fields keep RAW-count scale; consumers multiply by
// vol_density_scale exactly as they did for the raw grid.

struct MipParams {
    dim: u32,       // raw grid resolution per axis
    half_dim: u32,  // derived grid resolution per axis (dim / 2)
    radius: u32,    // closing radius in half-res voxels (1 or 2)
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> mp: MipParams;
@group(0) @binding(2) var<storage, read_write> out_a: array<f32>;
@group(0) @binding(3) var<storage, read_write> out_b: array<f32>;
// f32-typed view of the same slot as `src` for the half-res→half-res
// passes (dilate/erode read the previous pass's f32 output).
@group(0) @binding(4) var<storage, read> src_f: array<f32>;

@compute @workgroup_size(4, 4, 4)
fn reduce_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let hd = mp.half_dim;
    if (gid.x >= hd || gid.y >= hd || gid.z >= hd) {
        return;
    }
    let base = vec3<i32>(gid) * 2;
    let d = i32(mp.dim);
    var sum = 0.0;
    var cnt = 0.0;
    var mx = 0.0;
    // 4³ window centered on the 2×2×2 block (offsets -1..2): a plain
    // 2× box keeps too much noise; the one-voxel overlap between
    // neighboring outputs acts as the smoothing kernel.
    for (var oz = -1; oz <= 2; oz = oz + 1) {
        for (var oy = -1; oy <= 2; oy = oy + 1) {
            for (var ox = -1; ox <= 2; ox = ox + 1) {
                let p = base + vec3<i32>(ox, oy, oz);
                if (p.x < 0 || p.y < 0 || p.z < 0 || p.x >= d || p.y >= d || p.z >= d) {
                    continue;
                }
                let v = f32(src[(u32(p.z) * mp.dim + u32(p.y)) * mp.dim + u32(p.x)]);
                sum = sum + v;
                cnt = cnt + 1.0;
                mx = max(mx, v);
            }
        }
    }
    let idx = (gid.z * hd + gid.y) * hd + gid.x;
    out_a[idx] = sum / max(cnt, 1.0);
    out_b[idx] = mx;
}

@compute @workgroup_size(4, 4, 4)
fn dilate_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let hd = mp.half_dim;
    if (gid.x >= hd || gid.y >= hd || gid.z >= hd) {
        return;
    }
    let r = i32(mp.radius);
    let hdi = i32(hd);
    var mx = 0.0;
    for (var oz = -r; oz <= r; oz = oz + 1) {
        for (var oy = -r; oy <= r; oy = oy + 1) {
            for (var ox = -r; ox <= r; ox = ox + 1) {
                let p = vec3<i32>(gid) + vec3<i32>(ox, oy, oz);
                if (p.x < 0 || p.y < 0 || p.z < 0 || p.x >= hdi || p.y >= hdi || p.z >= hdi) {
                    continue;
                }
                mx = max(mx, src_f[(u32(p.z) * hd + u32(p.y)) * hd + u32(p.x)]);
            }
        }
    }
    out_a[(gid.z * hd + gid.y) * hd + gid.x] = mx;
}

@compute @workgroup_size(4, 4, 4)
fn erode_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let hd = mp.half_dim;
    if (gid.x >= hd || gid.y >= hd || gid.z >= hd) {
        return;
    }
    let r = i32(mp.radius);
    let hdi = i32(hd);
    var mn = 3.0e38;
    for (var oz = -r; oz <= r; oz = oz + 1) {
        for (var oy = -r; oy <= r; oy = oy + 1) {
            for (var ox = -r; ox <= r; ox = ox + 1) {
                let p = vec3<i32>(gid) + vec3<i32>(ox, oy, oz);
                if (p.x < 0 || p.y < 0 || p.z < 0 || p.x >= hdi || p.y >= hdi || p.z >= hdi) {
                    // Outside the grid is empty space: eroding against it
                    // keeps the cube boundary from reading as solid.
                    mn = 0.0;
                    continue;
                }
                mn = min(mn, src_f[(u32(p.z) * hd + u32(p.y)) * hd + u32(p.x)]);
            }
        }
    }
    out_a[(gid.z * hd + gid.y) * hd + gid.x] = mn;
}
