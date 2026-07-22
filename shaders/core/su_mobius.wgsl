// SU(n) SL(2,C) Mobius groups — Roger Bagula. Chaos game over a base
// generator set + its inverses, each conjugated in-shader by the tunable
// triquasiconformal deformation C = dk(delta).s0.qf(theta+i*eta):
//   - SU(2) 6-group 'plugged two ways' (two_Programs_..6group_Plugged_SU2
//     hypertriquasiconformal): 6 base matrices s[1..6] + inverses = 12;
//     Bagula's qf = rotate(i*pi/4) — HYPERbolic (eta=45, theta=0).
//   - SU(3) reduced Gell-Mann (Three_Programs_su3_reduction..): u0[1..8] +
//     inverses = 16; qf = rotate(pi/4) — elliptic (theta=45, eta=0).
// Layout per group: [base_1..M, inv_1..M]; conjugating an inverse base =
// the inverse generator, so a single conjugation path covers both.
// Packed pairs: 2k=(A.re,A.im,B.re,B.im), 2k+1=(C.re,C.im,D.re,D.im).
const SU_MOBIUS_BASE: array<vec4<f32>, 56> = array<vec4<f32>, 56>(
    vec4<f32>(2.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(0.0000000, 1.0000000, 0.0000000, 0.0000000), vec4<f32>(2.0000000, 0.0000000, -0.0000000, -1.0000000),
    vec4<f32>(2.0000000, 0.0000000, 0.0000000, 1.0000000), vec4<f32>(0.0000000, 1.0000000, 0.0000000, 0.0000000),
    vec4<f32>(0.0000000, 0.0000000, 1.0000000, 0.0000000), vec4<f32>(-1.0000000, 0.0000000, 2.0000000, 0.0000000),
    vec4<f32>(0.0000000, 1.0000000, 0.0000000, 0.0000000), vec4<f32>(2.0000000, 0.0000000, -0.0000000, -1.0000000),
    vec4<f32>(0.0000000, 0.0000000, 0.0000000, 1.0000000), vec4<f32>(0.0000000, 1.0000000, 2.0000000, 0.0000000),
    vec4<f32>(0.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 2.0000000, 0.0000000),
    vec4<f32>(0.0000000, -1.0000000, 0.0000000, 0.0000000), vec4<f32>(-2.0000000, 0.0000000, 0.0000000, 1.0000000),
    vec4<f32>(0.0000000, 0.0000000, -0.0000000, -1.0000000), vec4<f32>(-0.0000000, -1.0000000, 2.0000000, 0.0000000),
    vec4<f32>(2.0000000, 0.0000000, -1.0000000, 0.0000000), vec4<f32>(1.0000000, 0.0000000, 0.0000000, 0.0000000),
    vec4<f32>(0.0000000, -1.0000000, 0.0000000, 0.0000000), vec4<f32>(-2.0000000, 0.0000000, 0.0000000, 1.0000000),
    vec4<f32>(2.0000000, 0.0000000, -0.0000000, -1.0000000), vec4<f32>(-0.0000000, -1.0000000, 0.0000000, 0.0000000),
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

// (offset, generator-count) per group id. Inverses of the first half are
// the second half, so avoid-reversal pairs local j with (j+count/2)%count.
fn su_group_range(group: u32) -> vec2<u32> {
    switch group {
        case 0u: { return vec2<u32>(0u, 12u); }   // SU(2) 6-group
        case 1u: { return vec2<u32>(12u, 16u); }  // SU(3) reduced
        default: { return vec2<u32>(12u, 16u); }
    }
}

const SU_S0_AB: vec4<f32> = vec4<f32>(0.7071068, 0.0, 0.0, -0.7071068);
const SU_S0_CD: vec4<f32> = vec4<f32>(0.0, -0.7071068, 0.7071068, 0.0);

struct SuMat { a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32> }
fn su_cmul(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.x * y.x - x.y * y.y, x.x * y.y + x.y * y.x);
}
fn su_cdiv(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let dn = dot(y, y) + 1e-30;
    return vec2<f32>(x.x * y.x + x.y * y.y, x.y * y.x - x.x * y.y) / dn;
}
fn su_matmul(P: SuMat, Q: SuMat) -> SuMat {
    return SuMat(su_cmul(P.a,Q.a)+su_cmul(P.b,Q.c), su_cmul(P.a,Q.b)+su_cmul(P.b,Q.d),
                 su_cmul(P.c,Q.a)+su_cmul(P.d,Q.c), su_cmul(P.c,Q.b)+su_cmul(P.d,Q.d));
}
fn su_matinv(P: SuMat) -> SuMat {
    let det = su_cmul(P.a,P.d) - su_cmul(P.b,P.c);
    return SuMat(su_cdiv(P.d,det), su_cdiv(-P.b,det), su_cdiv(-P.c,det), su_cdiv(P.a,det));
}
fn su_base(idx: u32) -> SuMat {
    let ab = SU_MOBIUS_BASE[2u*idx]; let cd = SU_MOBIUS_BASE[2u*idx+1u];
    return SuMat(ab.xy, ab.zw, cd.xy, cd.zw);
}
// Conjugator C = dk(delta).s0.qf(theta + i*eta). qf = rotate of a COMPLEX
// angle: elliptic theta (SU(3)'s '45') + hyperbolic eta (SU(2)'s 'hyper').
fn su_conjugator(theta: f32, eta: f32, delta: f32) -> SuMat {
    let ch = cosh(eta); let sh = sinh(eta);
    let ct = cos(theta); let st = sin(theta);
    let ca = vec2<f32>(ct * ch, -st * sh);   // cos(theta + i eta)
    let sa = vec2<f32>(st * ch,  ct * sh);   // sin(theta + i eta)
    let qf = SuMat(ca, vec2<f32>(-sa.x, -sa.y), sa, ca);
    let dk = SuMat(vec2<f32>(1.0, delta), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, -delta));
    let s0 = SuMat(SU_S0_AB.xy, SU_S0_AB.zw, SU_S0_CD.xy, SU_S0_CD.zw);
    return su_matmul(su_matmul(dk, s0), qf);
}
fn su_mobius_apply(idx: u32, z: vec2<f32>, cj: SuMat, cji: SuMat) -> vec2<f32> {
    let m = su_matmul(su_matmul(cj, su_base(idx)), cji);
    return su_cdiv(su_cmul(m.a, z) + m.b, su_cmul(m.c, z) + m.d);
}
