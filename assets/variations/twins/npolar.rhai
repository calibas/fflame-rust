/// N-power polar warp with parity-based branch selection — even-parity
/// (`|parity|` even) routes through a log-polar mid-step before the angular
/// power; odd-parity stays in cartesian. A random `floor(rand · |n|)`
/// selects one of `|n|` angular branches.

// N-Polar, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    parity: #{
        value: 0, min: -100, max: 100,
        caption: "Parity",
        hint: "Parity selector. `|parity| mod 2` chooses the branch (even = log-polar mid-step, odd = cartesian); the value itself also flips the sign of the even-branch's radial magnitude.",
    },
    n: #{
        value: 1, min: -100, max: 100,
        caption: "N",
        hint: "Angular branch count. 0 is forced to 1 internally; negative values use `|n|` branches.",
    },
};

const INFO = #{ title: "N-Polar", name: "rhai_npolar", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let parity_i = to_int(to_float(p.parity));
    let n_i = to_int(to_float(p.n));
    let nnz_i = (if n_i == 0 { 1 } else { n_i });
    let isodd_i = abs(parity_i) - (abs(parity_i) / 2) * 2;
    let absn_i = abs(nnz_i);
    #{ nnz: to_float(nnz_i), isodd: to_float(isodd_i), absn: to_float(absn_i), cn: 1.0 / (2.0 * to_float(nnz_i)) }
}

fn variation(z, p, ctx) {
    let parity = to_float(p.parity);
    let nnz = p.nnz;
    let isodd = p.isodd;
    let absn = p.absn;
    let cn = p.cn;
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let vvar = w * inv_pi;
    let vvar_2 = vvar * 0.5;
    let x = 0.0;
    let y = 0.0;
    if (isodd != 0.0) {
        x = z.re;
        y = z.im;
    } else {
        x = vvar * atan2(z.re, z.im);
        y = vvar_2 * ln(max(1.0e-30, z.re * z.re + z.im * z.im));
    }
    let angle = (atan2(y, x) + two_pi * to_float(floor(rand() * absn))) / nnz;
    let radial = pow(max(1.0e-30, x * x + y * y), cn);
    let parity_factor = (if isodd == 0.0 { parity } else { 1.0 });
    let r = w * radial * parity_factor;
    let cosa = cos(angle) * r;
    let sina = sin(angle) * r;
    let fx = 0.0;
    let fy = 0.0;
    if (isodd != 0.0) {
        fx = cosa;
        fy = sina;
    } else {
        fx = vvar_2 * ln(max(1.0e-30, cosa * cosa + sina * sina));
        fy = vvar * atan2(cosa, sina);
    }
    complex(fx * inv_w, fy * inv_w)
}
