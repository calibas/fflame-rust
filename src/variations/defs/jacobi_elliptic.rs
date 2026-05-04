//! Jacobi elliptic family: `jac_sn`, `jac_cn`, `jac_dn`
//!
//! Three Apophysis variations using Jacobi sn/cn/dn elliptic
//! functions. Each takes a single user param `k` (the elliptic
//! modulus) and computes `Denom = w / (ε + sn²(x)·sn²(y)·k + cn²(y))`
//! over a different combination of NumX/NumY built from sn/cn/dn.
//!
//! Each variation inlines its own copy of the Jacobi sn/cn/dn helper
//! (~25 lines, ported from the IROIRO++ library, CC share-alike).
//! Helpers are named with the variation prefix (`jac_sn_je`, etc.) so
//! they don't collide when multiple variations are active in the
//! same shader.
//!
//!   - `jac_sn`: NumX = sn(x)·dn(y);          NumY = cn(x)·dn(x)·cn(y)·sn(y)
//!   - `jac_cn`: NumX = cn(x)·cn(y);          NumY = -dn(x)·sn(x)·dn(y)·sn(y)
//!   - `jac_dn`: NumX = dn(x)·cn(y)·dn(y);    NumY = -cn(x)·sn(x)·sn(y)·k
//!
//! All three factor cleanly through the outer multiplier.
//!
//! Sources: `output/jwildfire-vars/output/jac_{sn,cn,dn}.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// Each Jacobi helper:
//   inputs: uu (the angle), emmc (the complementary modulus, 1-k or k)
//   outputs: (sn, cn, dn)
// 8-iteration AGM-style descent; precision ~1e-7.

// =============================================================================
// jac_sn
// =============================================================================
pub static JAC_SN: VariationDef = VariationDef {
    name: "jac_sn",
    display_name: "Jac SN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("jac_sn_k", "K", unlimited_float, 0.5, -1.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn jac_sn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_sn(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_sn_je(p.x, k);
    let jy = jac_sn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = snx * dny;
    let num_y = cnx * dnx * cny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec2<f32>(denom * num_x, denom * num_y);
}
"#,
    wgsl_3d: Some(r#"
fn jac_sn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_sn(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_sn_je(p.x, k);
    let jy = jac_sn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = snx * dny;
    let num_y = cnx * dnx * cny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec3<f32>(denom * num_x, denom * num_y, p.z);
}
"#),
};

// =============================================================================
// jac_cn
// =============================================================================
pub static JAC_CN: VariationDef = VariationDef {
    name: "jac_cn",
    display_name: "Jac CN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("jac_cn_k", "K", unlimited_float, 0.5, -1.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn jac_cn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_cn(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_cn_je(p.x, k);
    let jy = jac_cn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = cnx * cny;
    let num_y = -dnx * snx * dny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec2<f32>(denom * num_x, denom * num_y);
}
"#,
    wgsl_3d: Some(r#"
fn jac_cn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_cn(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_cn_je(p.x, k);
    let jy = jac_cn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = cnx * cny;
    let num_y = -dnx * snx * dny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec3<f32>(denom * num_x, denom * num_y, p.z);
}
"#),
};

// =============================================================================
// jac_dn
// =============================================================================
pub static JAC_DN: VariationDef = VariationDef {
    name: "jac_dn",
    display_name: "Jac DN",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("jac_dn_k", "K", unlimited_float, 0.5, -1.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn jac_dn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_dn(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_dn_je(p.x, k);
    let jy = jac_dn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = dnx * cny * dny;
    let num_y = -cnx * snx * sny * k;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec2<f32>(denom * num_x, denom * num_y);
}
"#,
    wgsl_3d: Some(r#"
fn jac_dn_je(uu: f32, emmc: f32) -> vec3<f32> {
    let CA = 0.0003;
    var emc = emmc;
    var u = uu;
    var sn: f32 = 0.0;
    var cn: f32 = 1.0;
    var dn: f32 = 1.0;
    if (abs(emc) > 1e-30) {
        var bo: i32 = 0;
        var d: f32 = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        var a: f32 = 1.0;
        dn = 1.0;
        var em: array<f32, 8>;
        var en: array<f32, 8>;
        var c: f32 = 0.5 * (a + sqrt(emc));
        var l: i32 = 0;
        var done: bool = false;
        for (var i: i32 = 0; i < 8; i = i + 1) {
            if (!done) {
                l = i;
                em[i] = a;
                emc = sqrt(emc);
                en[i] = emc;
                c = 0.5 * (a + emc);
                if (abs(a - emc) <= CA * a) {
                    done = true;
                } else {
                    emc = a * emc;
                    a = c;
                }
            }
        }
        u = c * u;
        sn = sin(u);
        cn = cos(u);
        if (abs(sn) > 1e-30) {
            a = cn / sn;
            c = a * c;
            for (var ii: i32 = 7; ii >= 0; ii = ii - 1) {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = select(a, -a, sn < 0.0);
            cn = c * sn;
        }
        if (bo != 0) {
            let aa = dn;
            dn = cn;
            cn = aa;
            sn = sn / d;
        }
    } else {
        cn = 1.0 / cosh(u);
        dn = cn;
        sn = tanh(u);
    }
    return vec3<f32>(sn, cn, dn);
}

fn variation_jac_dn(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let k = get_param(xform_id, variation_id, 0u);
    let jx = jac_dn_je(p.x, k);
    let jy = jac_dn_je(p.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = dnx * cny * dny;
    let num_y = -cnx * snx * sny * k;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1e-9 + denom_inner);
    return vec3<f32>(denom * num_x, denom * num_y, p.z);
}
"#),
};
