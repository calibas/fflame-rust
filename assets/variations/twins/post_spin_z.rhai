/// Post-phase Z-axis rotation — same rotation as `pre_spin_z` (`w · π/2`
/// around Z), applied after all normal-phase variations.

// Post Spin Z, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Post Spin Z", name: "rhai_post_spin_z", category: "rotation_3d", phase: "post" };

fn variation(z, p, ctx) {
    let pi_2 = 1.5707963267948966;
    let w = ctx.weight;
    let angle = w * pi_2;
    let sina = sin(angle);
    let cosa = cos(angle);
    complex(sina * z.im + cosa * z.re, cosa * z.im - sina * z.re)
}
