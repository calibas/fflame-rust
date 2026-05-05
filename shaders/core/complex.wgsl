// Complex arithmetic + 2x2 complex matrix helpers for variations that
// operate over the Riemann sphere (Möbius transformations, Kleinian
// groups, complex trig/exp variants).
//
// Convention: a Complex is a vec2<f32> with .x = real, .y = imaginary.
// Functions are namespaced with `c` prefix (cmul, cdiv, csqrt, ...).
// 2×2 complex matrices use the `CMat2` struct with entries (a, b, c, d)
// representing [[a, b], [c, d]].
//
// f32 precision applies (no f64 in WGSL). Branch cuts follow standard
// principal-value conventions: csqrt returns the root with non-negative
// real part. Edge cases (cdiv by ~0, csqrt of 0) clamped via select to
// avoid NaN/Inf propagation; near-singular outputs are clipped naturally
// by the histogram.
//
// Seeded by arthomnix/fractal_viewer (MIT) for cmul/cdiv/csquare and
// DonKarlssonSan's GLSL gist as a textbook reference for csqrt.
// Implementation written from scratch; ~90 LoC.

fn cadd(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a + b;
}

fn csub(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return a - b;
}

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn cmul_real(a: vec2<f32>, s: f32) -> vec2<f32> {
    return a * s;
}

fn cconj(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x, -a.y);
}

fn cmag2(a: vec2<f32>) -> f32 {
    // |a|² = ar² + ai² (cheaper than sqrt when only the squared
    // magnitude is needed, e.g., for cdiv).
    return a.x * a.x + a.y * a.y;
}

fn cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    // a / b = a · conj(b) / |b|²
    let denom = cmag2(b);
    let safe_denom = select(denom, 1e-30, denom < 1e-30);
    return cmul(a, cconj(b)) / safe_denom;
}

fn csquare(a: vec2<f32>) -> vec2<f32> {
    // (ar + ai·i)² = (ar² - ai²) + 2·ar·ai·i
    return vec2<f32>(a.x * a.x - a.y * a.y, 2.0 * a.x * a.y);
}

fn csqrt(a: vec2<f32>) -> vec2<f32> {
    // Principal branch: result has non-negative real part. Standard
    // formula via |a| split:
    //   r = sqrt((|a| + ar) / 2)
    //   i = sign(ai) · sqrt((|a| - ar) / 2)
    // Edge case: a = 0 returns (0, 0).
    let mag = sqrt(cmag2(a));
    let real_part = sqrt(max(0.5 * (mag + a.x), 0.0));
    let imag_mag = sqrt(max(0.5 * (mag - a.x), 0.0));
    let imag_part = select(-imag_mag, imag_mag, a.y >= 0.0);
    return vec2<f32>(real_part, imag_part);
}

// 2×2 complex matrix:  [[a, b], [c, d]]
struct CMat2 {
    a: vec2<f32>,
    b: vec2<f32>,
    c: vec2<f32>,
    d: vec2<f32>,
}

fn cmat2_make(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>) -> CMat2 {
    var m: CMat2;
    m.a = a;
    m.b = b;
    m.c = c;
    m.d = d;
    return m;
}

// Möbius transformation:  f(z) = (a·z + b) / (c·z + d)
fn cmat2_apply(m: CMat2, z: vec2<f32>) -> vec2<f32> {
    let num = cadd(cmul(m.a, z), m.b);
    let den = cadd(cmul(m.c, z), m.d);
    return cdiv(num, den);
}

// Inverse for an SL(2,ℂ) matrix (determinant = 1):  [[d, -b], [-c, a]].
// Kleinian generator matrices are normalized to det = 1 by construction
// (Indra's Pearls Ch. 4), so this shortcut suffices for klein_group and
// related ports. For general 2×2 complex matrices the full formula
// would divide by det — out of scope here.
fn cmat2_inverse_sl2(m: CMat2) -> CMat2 {
    return cmat2_make(m.d, vec2<f32>(-m.b.x, -m.b.y), vec2<f32>(-m.c.x, -m.c.y), m.a);
}
