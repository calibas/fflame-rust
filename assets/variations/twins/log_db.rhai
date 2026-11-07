/// Complex log with random period jitter on the imaginary part. Like the
/// basic Log variation but the angle output gets shifted by a random
/// multiple of π (configured via `fix_period`), producing repeating-strip
/// patterns.
///
/// # Authors
/// - DarkBeam

// Log DB, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    base: #{
        value: 1.0, min: 1.0e-6, max: 100.0, free: true, exponential: false,
        caption: "Base",
        hint: "Logarithm base. Larger values compress the output, smaller values stretch it. Mirrors the basic Log variation's `base`.",
    },
    fix_period: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Fix Period",
        hint: "How much random vertical shift gets added each iteration. 0 = no shift; higher = more striping.",
    },
};

const INFO = #{ title: "Log DB", name: "rhai_log_db", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let base = p.base;
    let fix_period = p.fix_period;
    #{ denom: (if base <= 1.0e-20 { 0.5 } else { 0.5 / ln(max(1.0e-20, 2.71828182845905 * base)) }), fixpe: (if fix_period <= 1.0e-20 { 3.14159265358979 } else { 3.14159265358979 * fix_period }) }
}

fn variation(z, p) {
    let denom = p.denom;
    let fixpe = p.fixpe;
    let fix_atan_period = 0.0;
    for i in 0..7 {
        let adp = to_int(rand_u32() % u32(10)) - 5;
        if (adp >= 3 || adp <= -3) {
            adp = 0;
        }
        fix_atan_period = fix_atan_period + to_float(adp);
    }
    fix_atan_period = fix_atan_period * fixpe;
    let r2 = max(1.1754944e-38, z.re * z.re + z.im * z.im);
    complex(denom * ln(r2), atan2(z.re, z.im) + fix_atan_period)
}
