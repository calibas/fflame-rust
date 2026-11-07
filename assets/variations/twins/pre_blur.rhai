/// Adds gaussian random offset to the input point before the rest of
/// the variations run. Softens sharp features by jittering each
/// iteration's starting position.

// Pre-Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-Blur", name: "rhai_pre_blur", category: "advanced_2d", phase: "pre" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let rnd_g = w * (rand() + rand() + rand() + rand() + rand() + rand() - 3.0);
    let rnd_a = rand() * 6.28318530717959;
    complex(z.re + rnd_g * cos(rnd_a), z.im + rnd_g * sin(rnd_a))
}
