// Gamma, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Gamma", name: "rhai_gamma" };

fn gm_lgamma_iter(x) {
    let log_2pi_half = 0.9189385332046727;
    let xx = x - 1.0;
    let t = xx + 7.5;
    let a = 0.99999999999980993;
    a = a + 676.5203681218851 / (xx + 1.0);
    a = a - 1259.1392167224028 / (xx + 2.0);
    a = a + 771.3234287776531 / (xx + 3.0);
    a = a - 176.6150291621406 / (xx + 4.0);
    a = a + 12.507343278686905 / (xx + 5.0);
    a = a - 0.13857109526572012 / (xx + 6.0);
    a = a + 9.9843695780195716e-6 / (xx + 7.0);
    a = a + 1.5056327351493116e-7 / (xx + 8.0);
    log_2pi_half + (xx + 0.5) * ln(t) - t + ln(max(abs(a), 1.0e-30))
}

fn gm_lgamma(x) {
    let pi = 3.14159265358979;
    if (x < 0.5) {
        let s = sin(pi * x);
        let safe_s = (if abs(s) < 1.0e-30 { 1.0e-30 } else { abs(s) });
        return ln(pi) - ln(safe_s) - gm_lgamma_iter(1.0 - x);
    }
    gm_lgamma_iter(x)
}

fn variation(z, p) {
    let r = sqrt(z.re * z.re + z.im * z.im);
    complex(gm_lgamma(r), atan2(z.im, z.re))
}
