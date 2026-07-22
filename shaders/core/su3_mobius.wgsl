// SU(3)-reduced SL(2,C) Mobius group — Roger Bagula, Three_Programs_su3_
// reduction_McMullen_Moebius_Nylander_SU3_reduced_SL2C2_triquasiconformal45
// _16Limit_4.nb. The eight Gell-Mann matrices reduce 3x3 -> 2x2 as
// u0[i] = tt.lambda[i].t (tt=[[1,0,1],[a,b,c]]); this file bakes the eight
// u0 (entries 0..7) and their inverses (8..15) as base matrices, and the
// variation conjugates each in-shader by C = dk(delta).s0.qf(theta) — the
// tunable triquasiconformal deformation (Bagula's is theta=pi/4, delta=1).
// The group is {C u0_i C^-1} + inverses = 16 SL(2,C) Mobius generators;
// (C M C^-1)^-1 = C M^-1 C^-1, so conjugating the baked inverses gives the
// inverse generators. Packed pairs: 2k=(A.re,A.im,B.re,B.im),
// 2k+1=(C.re,C.im,D.re,D.im).
const SU3_U0: array<vec4<f32>, 32> = array<vec4<f32>, 32>(
    vec4<f32>(0.0000000, 0.0000000, 0.0000000, 1.0000000), vec4<f32>(0.0000000, 1.0000000, 0.0000000, 2.0000000),
    vec4<f32>(2.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(1.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 2.0000000, 0.0000000),
    vec4<f32>(2.0000000, 0.0000000, 2.0000000, 1.0000000), vec4<f32>(2.0000000, 1.0000000, 2.0000000, 2.0000000),
    vec4<f32>(0.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(0.0000000, 0.0000000, 0.0000000, 1.0000000), vec4<f32>(0.0000000, 1.0000000, -2.0000000, 2.0000000),
    vec4<f32>(0.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 2.0000000, 0.0000000),
    vec4<f32>(-0.5773503, 0.0000000, -0.5773503, -1.1547005), vec4<f32>(-0.5773503, -1.1547005, -0.0000000, -2.3094011),
    vec4<f32>(0.0000000, 2.0000000, -0.0000000, -1.0000000), vec4<f32>(-0.0000000, -1.0000000, 0.0000000, 0.0000000),
    vec4<f32>(0.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 2.0000000, 0.0000000),
    vec4<f32>(2.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 1.0000000, 0.0000000),
    vec4<f32>(2.0000000, 2.0000000, -2.0000000, -1.0000000), vec4<f32>(-2.0000000, -1.0000000, 2.0000000, 0.0000000),
    vec4<f32>(0.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(-2.0000000, 2.0000000, -0.0000000, -1.0000000), vec4<f32>(-0.0000000, -1.0000000, 0.0000000, 0.0000000),
    vec4<f32>(2.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(-0.0000000, -2.3094011, 0.5773503, 1.1547005), vec4<f32>(0.5773503, 1.1547005, -0.5773503, 0.0000000),
);

// s0 = [[1,-i],[-i,1]]/sqrt2 (fixed part of the conjugator).
const SU3_S0_AB: vec4<f32> = vec4<f32>(0.7071068, 0.0, 0.0, -0.7071068);
const SU3_S0_CD: vec4<f32> = vec4<f32>(0.0, -0.7071068, 0.7071068, 0.0);

struct Su3Mat { a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32> }

fn su3_cmul(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.x * y.x - x.y * y.y, x.x * y.y + x.y * y.x);
}
fn su3_cdiv(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let dn = dot(y, y) + 1e-30;
    return vec2<f32>(x.x * y.x + x.y * y.y, x.y * y.x - x.x * y.y) / dn;
}
fn su3_matmul(P: Su3Mat, Q: Su3Mat) -> Su3Mat {
    return Su3Mat(
        su3_cmul(P.a, Q.a) + su3_cmul(P.b, Q.c),
        su3_cmul(P.a, Q.b) + su3_cmul(P.b, Q.d),
        su3_cmul(P.c, Q.a) + su3_cmul(P.d, Q.c),
        su3_cmul(P.c, Q.b) + su3_cmul(P.d, Q.d));
}
fn su3_matinv(P: Su3Mat) -> Su3Mat {
    let det = su3_cmul(P.a, P.d) - su3_cmul(P.b, P.c);
    return Su3Mat(su3_cdiv(P.d, det), su3_cdiv(-P.b, det), su3_cdiv(-P.c, det), su3_cdiv(P.a, det));
}
fn su3_base(k: u32) -> Su3Mat {
    let ab = SU3_U0[2u * k];
    let cd = SU3_U0[2u * k + 1u];
    return Su3Mat(ab.xy, ab.zw, cd.xy, cd.zw);
}
// Conjugator C = dk(delta) . s0 . qf(theta), theta in radians.
fn su3_conjugator(theta: f32, delta: f32) -> Su3Mat {
    let re0 = vec2<f32>(0.0, 0.0);
    let dk = Su3Mat(vec2<f32>(1.0, delta), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, -delta));
    let s0 = Su3Mat(SU3_S0_AB.xy, SU3_S0_AB.zw, SU3_S0_CD.xy, SU3_S0_CD.zw);
    let ct = cos(theta); let st = sin(theta);
    let qf = Su3Mat(vec2<f32>(ct, 0.0), vec2<f32>(-st, 0.0), vec2<f32>(st, 0.0), vec2<f32>(ct, 0.0));
    return su3_matmul(su3_matmul(dk, s0), qf);
}
// Apply generator k under conjugator (C, Ci) to complex z.
fn su3_mobius_apply(k: u32, z: vec2<f32>, cj: Su3Mat, cji: Su3Mat) -> vec2<f32> {
    let m = su3_matmul(su3_matmul(cj, su3_base(k)), cji);
    let num = su3_cmul(m.a, z) + m.b;
    let den = su3_cmul(m.c, z) + m.d;
    return su3_cdiv(num, den);
}
