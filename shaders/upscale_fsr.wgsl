// upscale_fsr.wgsl -- AMD FidelityFX Super Resolution 1.0 (FSR 1) for the
// viewport's upscale (src/renderer/upscale.rs, docs/projects/quality-governor.md):
// EASU, the edge-adaptive spatial upsampling, then RCAS, the robust
// contrast-adaptive sharpening, as two passes.
//
// A line-for-line port of the 32-bit float paths of ffx_fsr1.h
// v1.20210629 (`FsrEasuF`, `FsrEasuTapF`, `FsrEasuSetF`, `FsrRcasF`) and
// the ffx_a.h helpers they call, with the original names, so it can be
// read against the source. What differs, and why:
//   - EASU reads its twelve taps with `textureLoad`, clamped to the edge,
//     rather than through four `textureGather`s and a clamping sampler:
//     the same texels, and no sampler (SHADERS.md's WebGPU rule).
//   - The alpha, which FSR does not carry, is bilinear from the four
//     nearest in EASU and passed through RCAS
//     (`FSR_RCAS_PASSTHROUGH_ALPHA`).
//   - RCAS's two limiters divide by zero on a ring of pure black or pure
//     white: 0 * inf, NaN, which HLSL's max() passes over for its other
//     operand. WGSL leaves a NaN's max() to the platform, and Metal's
//     fast-math does not keep NaNs at all, so `rcas_lobe` reproduces
//     HLSL's result there explicitly.
//   - `FSR_RCAS_DENOISE` is on: flames are grainy, and sharpening grain
//     makes more of it.
//   - The input is the tone-mapped 8-bit picture, already in perceptual
//     space as FSR expects, so `FsrRcasInputF` is the identity.
//
// Reference: https://github.com/GPUOpen-Effects/FidelityFX-FSR
// (ffx-fsr/ffx_fsr1.h, ffx-fsr/ffx_a.h). License headers preserved below.
//
// -----------------------------------------------------------------------------
// FidelityFX Super Resolution Sample
//
// Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files(the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and / or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions :
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
// -----------------------------------------------------------------------------
// MIT LICENSE
// ===========
// Copyright (c) 2014 Michal Drobot (for concepts used in "FLOAT APPROXIMATIONS").
// -----------
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
// -----------
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
// Software.
// -----------
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
// WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
// -----------------------------------------------------------------------------

// The upscale's uniform (renderer/upscale.rs), as both passes read it.
struct UpscaleParams {
    // Source pixels per destination pixel, each axis: FsrEasuCon's con0.xy.
    ratio: vec2<f32>,
    kind: u32,
    // RCAS's sharpness, linear: FsrRcasCon's con.x, exp2(-stops).
    sharpness: f32,
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> p: UpscaleParams;

// ---- ffx_a.h --------------------------------------------------------------

fn AMin3F1(x: f32, y: f32, z: f32) -> f32 { return min(x, min(y, z)); }
fn AMax3F1(x: f32, y: f32, z: f32) -> f32 { return max(x, max(y, z)); }
fn AMin3F3(x: vec3<f32>, y: vec3<f32>, z: vec3<f32>) -> vec3<f32> { return min(x, min(y, z)); }
fn AMax3F3(x: vec3<f32>, y: vec3<f32>, z: vec3<f32>) -> vec3<f32> { return max(x, max(y, z)); }
fn ASatF1(x: f32) -> f32 { return clamp(x, 0.0, 1.0); }
fn APrxLoRcpF1(a: f32) -> f32 { return bitcast<f32>(0x7ef07ebbu - bitcast<u32>(a)); }
fn APrxMedRcpF1(a: f32) -> f32 {
    let b = bitcast<f32>(0x7ef19fffu - bitcast<u32>(a));
    return b * (-b * a + 2.0);
}
fn APrxLoRsqF1(a: f32) -> f32 { return bitcast<f32>(0x5f347d74u - (bitcast<u32>(a) >> 1u)); }

// ---- EASU -----------------------------------------------------------------

fn fsr_tap(q: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(src_tex, clamp(q, vec2<i32>(0), size - vec2<i32>(1)), 0);
}

// Filtering for a given tap for the scalar.
fn FsrEasuTapF(
    aC: ptr<function, vec3<f32>>, // Accumulated color, with negative lobe.
    aW: ptr<function, f32>, // Accumulated weight.
    off: vec2<f32>, // Pixel offset from resolve position to tap.
    dir: vec2<f32>, // Gradient direction.
    len: vec2<f32>, // Length.
    lob: f32, // Negative lobe strength.
    clp: f32, // Clipping point.
    c: vec3<f32>, // Tap color.
) {
    // Rotate offset by direction.
    var v: vec2<f32>;
    v.x = (off.x * (dir.x)) + (off.y * dir.y);
    v.y = (off.x * (-dir.y)) + (off.y * dir.x);
    // Anisotropy.
    v *= len;
    // Compute distance^2.
    var d2 = v.x * v.x + v.y * v.y;
    // Limit to the window as at corner, 2 taps can easily be outside.
    d2 = min(d2, clp);
    // Approximation of lancos2 without sin() or rcp(), or sqrt() to get x.
    //  (25/16 * (2/5 * x^2 - 1)^2 - (25/16 - 1)) * (1/4 * x^2 - 1)^2
    //  |_______________________________________|   |_______________|
    //                   base                             window
    // The general form of the 'base' is,
    //  (a*(b*x^2-1)^2-(a-1))
    // Where 'a=1/(2*b-b^2)' and 'b' moves around the negative lobe.
    var wB = (2.0 / 5.0) * d2 + (-1.0);
    var wA = lob * d2 + (-1.0);
    wB *= wB;
    wA *= wA;
    wB = (25.0 / 16.0) * wB + (-(25.0 / 16.0 - 1.0));
    let w = wB * wA;
    // Do weighted average.
    *aC += c * w;
    *aW += w;
}

// Accumulate direction and length.
fn FsrEasuSetF(
    dir: ptr<function, vec2<f32>>,
    len: ptr<function, f32>,
    pp: vec2<f32>,
    biS: bool, biT: bool, biU: bool, biV: bool,
    lA: f32, lB: f32, lC: f32, lD: f32, lE: f32,
) {
    // Compute bilinear weight, branches factor out as predicates are compiler time immediates.
    //  s t
    //  u v
    var w = 0.0;
    if (biS) { w = (1.0 - pp.x) * (1.0 - pp.y); }
    if (biT) { w = pp.x * (1.0 - pp.y); }
    if (biU) { w = (1.0 - pp.x) * pp.y; }
    if (biV) { w = pp.x * pp.y; }
    // Direction is the '+' diff.
    //    a
    //  b c d
    //    e
    // Then takes magnitude from abs average of both sides of 'c'.
    // Length converts gradient reversal to 0, smoothly to non-reversal at 1, shaped, then adding horz and vert terms.
    let dc = lD - lC;
    let cb = lC - lB;
    var lenX = max(abs(dc), abs(cb));
    lenX = APrxLoRcpF1(lenX);
    let dirX = lD - lB;
    (*dir).x += dirX * w;
    lenX = ASatF1(abs(dirX) * lenX);
    lenX *= lenX;
    *len += lenX * w;
    // Repeat for the y axis.
    let ec = lE - lC;
    let ca = lC - lA;
    var lenY = max(abs(ec), abs(ca));
    lenY = APrxLoRcpF1(lenY);
    let dirY = lE - lA;
    (*dir).y += dirY * w;
    lenY = ASatF1(abs(dirY) * lenY);
    lenY *= lenY;
    *len += lenY * w;
}

// Simplest multi-channel approximate luma possible (luma times 2, in 2 FMA/MAD).
fn fsr_luma2(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

@compute @workgroup_size(8, 8, 1)
fn easu_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = vec2<i32>(textureDimensions(dst_tex));
    let ip = vec2<i32>(i32(gid.x), i32(gid.y));
    if (ip.x >= dims.x || ip.y >= dims.y) {
        return;
    }
    let size = vec2<i32>(textureDimensions(src_tex));
    // Get position of 'f' (FsrEasuCon's con0: the ratio, and half a
    // destination pixel less half a source pixel).
    var pp = vec2<f32>(ip) * p.ratio + (0.5 * p.ratio - vec2<f32>(0.5));
    let fp = floor(pp);
    pp -= fp;
    let f0 = vec2<i32>(fp);
    // 12-tap kernel.
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let b = fsr_tap(f0 + vec2<i32>(0, -1), size);
    let c = fsr_tap(f0 + vec2<i32>(1, -1), size);
    let e = fsr_tap(f0 + vec2<i32>(-1, 0), size);
    let f = fsr_tap(f0, size);
    let g = fsr_tap(f0 + vec2<i32>(1, 0), size);
    let h = fsr_tap(f0 + vec2<i32>(2, 0), size);
    let i = fsr_tap(f0 + vec2<i32>(-1, 1), size);
    let j = fsr_tap(f0 + vec2<i32>(0, 1), size);
    let k = fsr_tap(f0 + vec2<i32>(1, 1), size);
    let l = fsr_tap(f0 + vec2<i32>(2, 1), size);
    let n = fsr_tap(f0 + vec2<i32>(0, 2), size);
    let o = fsr_tap(f0 + vec2<i32>(1, 2), size);
    let bL = fsr_luma2(b.rgb);
    let cL = fsr_luma2(c.rgb);
    let iL = fsr_luma2(i.rgb);
    let jL = fsr_luma2(j.rgb);
    let fL = fsr_luma2(f.rgb);
    let eL = fsr_luma2(e.rgb);
    let kL = fsr_luma2(k.rgb);
    let lL = fsr_luma2(l.rgb);
    let hL = fsr_luma2(h.rgb);
    let gL = fsr_luma2(g.rgb);
    let oL = fsr_luma2(o.rgb);
    let nL = fsr_luma2(n.rgb);
    // Accumulate for bilinear interpolation.
    var dir = vec2<f32>(0.0);
    var len = 0.0;
    FsrEasuSetF(&dir, &len, pp, true, false, false, false, bL, eL, fL, gL, jL);
    FsrEasuSetF(&dir, &len, pp, false, true, false, false, cL, fL, gL, hL, kL);
    FsrEasuSetF(&dir, &len, pp, false, false, true, false, fL, iL, jL, kL, nL);
    FsrEasuSetF(&dir, &len, pp, false, false, false, true, gL, jL, kL, lL, oL);
    // Normalize with approximation, and cleanup close to zero.
    let dir2 = dir * dir;
    var dirR = dir2.x + dir2.y;
    let zro = dirR < (1.0 / 32768.0);
    dirR = APrxLoRsqF1(dirR);
    dirR = select(dirR, 1.0, zro);
    dir.x = select(dir.x, 1.0, zro);
    dir *= vec2<f32>(dirR);
    // Transform from {0 to 2} to {0 to 1} range, and shape with square.
    len = len * 0.5;
    len *= len;
    // Stretch kernel {1.0 vert|horz, to sqrt(2.0) on diagonal}.
    let stretch = (dir.x * dir.x + dir.y * dir.y) * APrxLoRcpF1(max(abs(dir.x), abs(dir.y)));
    // Anisotropic length after rotation,
    //  x := 1.0 lerp to 'stretch' on edges
    //  y := 1.0 lerp to 2x on edges
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 + (-0.5) * len);
    // Based on the amount of 'edge',
    // the window shifts from +/-{sqrt(2.0) to slightly beyond 2.0}.
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    // Set distance^2 clipping point to the end of the adjustable window.
    let clp = APrxLoRcpF1(lob);
    // Accumulation mixed with min/max of 4 nearest.
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let min4 = min(AMin3F3(f.rgb, g.rgb, j.rgb), k.rgb);
    let max4 = max(AMax3F3(f.rgb, g.rgb, j.rgb), k.rgb);
    // Accumulation.
    var aC = vec3<f32>(0.0);
    var aW = 0.0;
    FsrEasuTapF(&aC, &aW, vec2<f32>(0.0, -1.0) - pp, dir, len2, lob, clp, b.rgb); // b
    FsrEasuTapF(&aC, &aW, vec2<f32>(1.0, -1.0) - pp, dir, len2, lob, clp, c.rgb); // c
    FsrEasuTapF(&aC, &aW, vec2<f32>(-1.0, 1.0) - pp, dir, len2, lob, clp, i.rgb); // i
    FsrEasuTapF(&aC, &aW, vec2<f32>(0.0, 1.0) - pp, dir, len2, lob, clp, j.rgb); // j
    FsrEasuTapF(&aC, &aW, vec2<f32>(0.0, 0.0) - pp, dir, len2, lob, clp, f.rgb); // f
    FsrEasuTapF(&aC, &aW, vec2<f32>(-1.0, 0.0) - pp, dir, len2, lob, clp, e.rgb); // e
    FsrEasuTapF(&aC, &aW, vec2<f32>(1.0, 1.0) - pp, dir, len2, lob, clp, k.rgb); // k
    FsrEasuTapF(&aC, &aW, vec2<f32>(2.0, 1.0) - pp, dir, len2, lob, clp, l.rgb); // l
    FsrEasuTapF(&aC, &aW, vec2<f32>(2.0, 0.0) - pp, dir, len2, lob, clp, h.rgb); // h
    FsrEasuTapF(&aC, &aW, vec2<f32>(1.0, 0.0) - pp, dir, len2, lob, clp, g.rgb); // g
    FsrEasuTapF(&aC, &aW, vec2<f32>(1.0, 2.0) - pp, dir, len2, lob, clp, o.rgb); // o
    FsrEasuTapF(&aC, &aW, vec2<f32>(0.0, 2.0) - pp, dir, len2, lob, clp, n.rgb); // n
    // Normalize and dering.
    let pix = min(max4, max(min4, aC * vec3<f32>(1.0 / aW)));
    // The alpha, bilinear from the four nearest.
    let alpha = mix(mix(f.a, g.a, pp.x), mix(j.a, k.a, pp.x), pp.y);
    textureStore(dst_tex, ip, vec4<f32>(pix, alpha));
}

// ---- RCAS -----------------------------------------------------------------

// This is set at the limit of providing unnatural results for sharpening.
const FSR_RCAS_LIMIT: f32 = 0.25 - (1.0 / 16.0);

// One channel's lobe: max(-hitMin, hitMax), the limiters FsrRcasF computes
// with "high precision RCPs". Where the ring is all 0 hitMin divides by
// zero (0 * inf), and where it is all 1 hitMax does; HLSL's max() passes
// the NaN over for its other operand, which is what each early return is.
fn rcas_lobe(mn4: f32, mx4: f32, e: f32) -> f32 {
    // Immediate constants for peak range.
    let peakC = vec2<f32>(1.0, -1.0 * 4.0);
    let hitMax = (peakC.x - max(mx4, e)) * (1.0 / (4.0 * mn4 + peakC.y));
    if (mx4 <= 0.0) {
        return hitMax;
    }
    let hitMin = min(mn4, e) * (1.0 / (4.0 * mx4));
    if (mn4 >= 1.0) {
        return -hitMin;
    }
    return max(-hitMin, hitMax);
}

@compute @workgroup_size(8, 8, 1)
fn rcas_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = vec2<i32>(textureDimensions(dst_tex));
    let sp = vec2<i32>(i32(gid.x), i32(gid.y));
    if (sp.x >= dims.x || sp.y >= dims.y) {
        return;
    }
    let size = vec2<i32>(textureDimensions(src_tex));
    // Algorithm uses minimal 3x3 pixel neighborhood.
    //    b
    //  d e f
    //    h
    let b = fsr_tap(sp + vec2<i32>(0, -1), size).rgb;
    let d = fsr_tap(sp + vec2<i32>(-1, 0), size).rgb;
    let ee = fsr_tap(sp, size);
    let e = ee.rgb;
    let f = fsr_tap(sp + vec2<i32>(1, 0), size).rgb;
    let h = fsr_tap(sp + vec2<i32>(0, 1), size).rgb;
    // Luma times 2.
    let bL = fsr_luma2(b);
    let dL = fsr_luma2(d);
    let eL = fsr_luma2(e);
    let fL = fsr_luma2(f);
    let hL = fsr_luma2(h);
    // Noise detection.
    var nz = 0.25 * bL + 0.25 * dL + 0.25 * fL + 0.25 * hL - eL;
    nz = ASatF1(abs(nz) * APrxMedRcpF1(AMax3F1(AMax3F1(bL, dL, eL), fL, hL) - AMin3F1(AMin3F1(bL, dL, eL), fL, hL)));
    nz = -0.5 * nz + 1.0;
    // Min and max of ring.
    let mn4 = min(AMin3F3(b, d, f), h);
    let mx4 = max(AMax3F3(b, d, f), h);
    let lobeR = rcas_lobe(mn4.r, mx4.r, e.r);
    let lobeG = rcas_lobe(mn4.g, mx4.g, e.g);
    let lobeB = rcas_lobe(mn4.b, mx4.b, e.b);
    var lobe = max(-FSR_RCAS_LIMIT, min(AMax3F1(lobeR, lobeG, lobeB), 0.0)) * p.sharpness;
    // Apply noise removal.
    lobe *= nz;
    // Resolve, which needs the medium precision rcp approximation to avoid visible tonality changes.
    let rcpL = APrxMedRcpF1(4.0 * lobe + 1.0);
    let pix = (lobe * b + lobe * d + lobe * h + lobe * f + e) * rcpL;
    textureStore(dst_tex, sp, vec4<f32>(clamp(pix, vec3<f32>(0.0), vec3<f32>(1.0)), ee.a));
}
