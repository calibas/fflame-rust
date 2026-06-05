// noise.wgsl — Simplex + Perlin noise helpers.
//
// **Implementation note**: JWildfire's `NoiseTools.simplexNoise3D` uses
// 1024-entry permutation + 1024-entry gradient lookup tables. Direct
// port to WGSL fails on GPU — module-scope `const array<u32, 1024>`
// gets emitted into the SPIR-V `Private` storage class, which the
// driver allocates per shader invocation (~20KB per thread × 32K
// threads = gigabytes of GPU memory → system lockup, observed
// empirically on Vulkan).
//
// We use the **table-free** procedural Simplex by Ian McEwan / Stefan
// Gustavson — the `webgl-noise` MIT-licensed implementation widely
// used in WebGL/WebGPU shaders. All "hash" lookups are computed by the
// `permute` integer-shuffle; gradients are derived arithmetically from
// the hash. Fixed cost per call, no global state, no driver-side
// per-thread allocations.
//
// Difference from JWildfire's output: not bit-identical (different
// gradient layout and a different final scale constant — 42.0 here vs
// 32.0 in NoiseTools), but produces statistically equivalent simplex
// noise in roughly [-1, 1]. The visual effect on `crackle`'s
// cell-center perturbation and on `dc_perlin`'s noise field is the
// same kind of organic warping — neither variation is sensitive to
// the exact noise values.
//
// Reference: https://github.com/stegu/webgl-noise
// License header preserved below.
//
//   Description : Array and textureless GLSL 2D/3D/4D simplex
//                 noise functions.
//        Author : Ian McEwan, Ashima Arts.
//    Maintainer : ijm
//       Lastmod : 20110822 (ijm)
//       License : Copyright (C) 2011 Ashima Arts. All rights reserved.
//                 Distributed under the MIT License. See LICENSE file.
//                 https://github.com/ashima/webgl-noise

fn noise_mod289_v3(x: vec3<f32>) -> vec3<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn noise_mod289_v4(x: vec4<f32>) -> vec4<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

// Integer hash via repeated `(x*34 + 10)*x mod 289`. Generates a
// well-distributed pseudorandom value per 3D lattice point with no
// table lookups.
fn noise_permute_v4(x: vec4<f32>) -> vec4<f32> {
    return noise_mod289_v4(((x * 34.0) + 10.0) * x);
}

// First-order Taylor approximation of 1/sqrt — accurate enough across
// the input range produced by simplex's radial falloff.
fn noise_taylor_inv_sqrt(r: vec4<f32>) -> vec4<f32> {
    return 1.79284291400159 - 0.85373472095314 * r;
}

// Simplex noise in 3D. Output is roughly in [-1, 1]; the 42.0 final
// scale is the empirically-tuned value for this gradient layout.
fn simplex_noise_3d(v: vec3<f32>) -> f32 {
    let c = vec2<f32>(1.0 / 6.0, 1.0 / 3.0);

    // First corner (skew, then unskew).
    let i = floor(v + dot(v, c.yyy));
    let x0 = v - i + dot(i, c.xxx);

    // Other corners — determined by the relative order of x0's components.
    let g = step(x0.yzx, x0.xyz);
    let l = 1.0 - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);

    let x1 = x0 - i1 + c.xxx;
    let x2 = x0 - i2 + c.yyy;
    let x3 = x0 - 0.5;  // -D.yyy where D.y = 0.5

    // Per-corner hashes via the permute integer shuffle.
    let i_mod = noise_mod289_v3(i);
    let p = noise_permute_v4(
        noise_permute_v4(
            noise_permute_v4(
                i_mod.z + vec4<f32>(0.0, i1.z, i2.z, 1.0)
            ) + i_mod.y + vec4<f32>(0.0, i1.y, i2.y, 1.0)
        ) + i_mod.x + vec4<f32>(0.0, i1.x, i2.x, 1.0)
    );

    // Map the hash onto 7×7 points over a square, then onto an
    // octahedron — yields ~98 different gradient directions, plenty for
    // a smooth noise function.
    let n_ = 0.142857142857;  // 1/7
    let ns = n_ * vec3<f32>(2.0, 0.5, 1.0) - vec3<f32>(0.0, 1.0, 0.0);

    let j = p - 49.0 * floor(p * ns.z * ns.z);

    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);

    let xc = x_ * ns.x + ns.yyyy;
    let yc = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(xc) - abs(yc);

    let b0 = vec4<f32>(xc.x, xc.y, yc.x, yc.y);
    let b1 = vec4<f32>(xc.z, xc.w, yc.z, yc.w);

    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4<f32>(0.0));

    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;

    var p0 = vec3<f32>(a0.x, a0.y, h.x);
    var p1 = vec3<f32>(a0.z, a0.w, h.y);
    var p2 = vec3<f32>(a1.x, a1.y, h.z);
    var p3 = vec3<f32>(a1.z, a1.w, h.w);

    let norm = noise_taylor_inv_sqrt(
        vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3))
    );
    p0 = p0 * norm.x;
    p1 = p1 * norm.y;
    p2 = p2 * norm.z;
    p3 = p3 * norm.w;

    // Radial falloff per corner — same `0.6 - r²` weighting as the
    // table-based version, rolled into a vec4.
    var m = max(
        0.6 - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)),
        vec4<f32>(0.0),
    );
    m = m * m;

    return 42.0 * dot(
        m * m,
        vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)),
    );
}

// Perlin's octave-summed noise — sum of simplex noise at successive
// frequencies, each octave's amplitude divided by `a_scale^octave` and
// its frequency multiplied by `f_scale^octave`. `octaves` is clamped to
// [1, 8] to keep the per-call cost bounded on the GPU.
fn perlin_noise_3d(v: vec3<f32>, a_scale: f32, f_scale: f32, octaves_raw: u32) -> f32 {
    let octaves = clamp(octaves_raw, 1u, 8u);
    var n: f32 = 0.0;
    var u: vec3<f32> = v;
    var a: f32 = 1.0;
    for (var i: u32 = 0u; i < octaves; i = i + 1u) {
        n = n + simplex_noise_3d(u) / a;
        a = a * a_scale;
        u = u * f_scale;
    }
    return n;
}
