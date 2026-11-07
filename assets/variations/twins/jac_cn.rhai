/// Jacobi elliptic cn-driven warp — same denominator as `jac_sn`, with
/// numerators `(cn(x,k)·cn(y,1−k), −dn(x,k)·sn(x,k)·dn(y,1−k)·sn(y,1−k))`.
/// Produces cn-flavored elliptic patterns.
///
/// # Authors
/// - IROIRO++

// Jac CN, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    jac_cn_k: #{
        value: 0.5, min: -1.0, max: 1.0, free: true,
        caption: "K",
        hint: "Elliptic modulus k. Ranges over [−1, 1]; 0 reduces to circular trig, ±1 reduces to hyperbolic.",
    },
};

const INFO = #{ title: "Jac CN", name: "rhai_jac_cn", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn jac_cn_je(uu, emmc) {
    let CA = 0.0003;
    let emc = emmc;
    let u = uu;
    let sn = 0.0;
    let cn = 1.0;
    let dn = 1.0;
    if (abs(emc) > 1.0e-30) {
        let bo = 0;
        let d = 1.0;
        if (emc < 0.0) {
            bo = 1;
            d = 1.0 - emc;
            emc = -emc / d;
            d = sqrt(d);
            u = d * u;
        }
        let a = 1.0;
        dn = 1.0;
        let em = array(8, 0.0);
        let en = array(8, 0.0);
        let c = 0.5 * (a + sqrt(emc));
        let l = 0;
        let done = false;
        for i in 0..8 {
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
        if (abs(sn) > 1.0e-30) {
            a = cn / sn;
            c = a * c;
            let ii = 7;
            while ii >= 0 {
                if (ii <= l) {
                    let b = em[ii];
                    a = c * a;
                    c = dn * c;
                    dn = (en[ii] + a) / (b + a);
                    a = c / b;
                }
                ii = ii - 1;
            }
            a = 1.0 / sqrt(c * c + 1.0);
            sn = (if sn < 0.0 { -a } else { a });
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
    vec3(sn, cn, dn)
}

fn variation(w, p) {
    let k = p.jac_cn_k;
    let jx = jac_cn_je(w.re, k);
    let jy = jac_cn_je(w.im, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = cnx * cny;
    let num_y = -dnx * snx * dny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1.0e-9 + denom_inner);
    complex(denom * num_x, denom * num_y)
}

fn variation3(v, p) {
    let k = p.jac_cn_k;
    let jx = jac_cn_je(v.x, k);
    let jy = jac_cn_je(v.y, 1.0 - k);
    let snx = jx.x;
    let cnx = jx.y;
    let dnx = jx.z;
    let sny = jy.x;
    let cny = jy.y;
    let dny = jy.z;
    let num_x = cnx * cny;
    let num_y = -dnx * snx * dny * sny;
    let denom_inner = snx * snx * sny * sny * k + cny * cny;
    let denom = 1.0 / (1.0e-9 + denom_inner);
    vec3(denom * num_x, denom * num_y, v.z)
}
