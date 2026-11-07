/// Pre-phase Z-axis rotation — applies a rotation by `w · π/2` around the Z
/// axis to the input before any normal-phase variations run. Useful as a
/// per-transform pre-rotation step driven by the variation weight.

// Pre Spin Z, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre Spin Z", name: "rhai_pre_spin_z", category: "rotation_3d", phase: "pre" };

fn variation(z, p, ctx) {
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    complex(sina * z.im + cosa * z.re, cosa * z.im - sina * z.re)
}
