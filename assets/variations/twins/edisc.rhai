// E-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "E-Disc", name: "rhai_edisc" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let tmp = z.re * z.re + z.im * z.im + 1.0;
    let tmp2 = 2.0 * z.re;
    let r1 = sqrt(max(tmp + tmp2, 0.0));
    let r2 = sqrt(max(tmp - tmp2, 0.0));
    let xmax = (r1 + r2) * 0.5;
    let safe_xmax = max(xmax, 1.0);
    let a1 = ln(safe_xmax + sqrt(max(safe_xmax - 1.0, 0.0)));
    let a2 = -acos(clamp(z.re / max(safe_xmax, 1.0e-30), -1.0, 1.0));
    let wlocal = w * 0.08643743168737844;
    let sn = sin(a1);
    let cs = cos(a1);
    let snh = sinh(a2);
    let csh = cosh(a2);
    if (z.im > 0.0) {
        sn = -sn;
    }
    complex(wlocal * csh * cs * inv_w, wlocal * snh * sn * inv_w)
}
