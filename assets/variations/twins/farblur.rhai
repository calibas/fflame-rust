/// Far-distance random blur — emits a spherical-coordinate random offset
/// whose magnitude scales with the squared distance of the running
/// accumulator from a configurable origin, so points further from the
/// origin receive proportionally more blur. The scale factor is modulated
/// by a rolling buffer of 4 random samples (`r[0..3]`), refreshed one slot
/// per iteration; their sum minus 2 gives a slowly-varying signed
/// multiplier. In 2D the missing Z accumulator collapses the `dz` term to a
/// constant `(0 - z_origin)²`, so `z_origin` becomes a constant additive
/// offset on the radial magnitude.
///
/// # Authors
/// - zephyrtronium

// Far Blur, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Scale",
        hint: "X-axis blur scale. Multiplies the X component of the random spherical offset.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Scale",
        hint: "Y-axis blur scale.",
    },
    z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Scale",
        hint: "Z-axis blur scale (3D only).",
    },
    x_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X Origin",
        hint: "Origin X coordinate. The accumulator's distance from this point (squared) modulates the blur magnitude — larger distance → more blur.",
    },
    y_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Origin",
        hint: "Origin Y coordinate.",
    },
    z_origin: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Origin",
        hint: "Origin Z coordinate. In 2D mode this becomes a constant offset (the 2D accumulator has no Z, so `dz` reduces to `-z_origin`).",
    },
};

const INFO = #{ title: "Far Blur", name: "rhai_farblur", category: "full_3d" };

fn init(p) {
    #{ st: [0.0, 0.0, 0.0, 0.0, 0.0] }
}

fn variation(z, p, ctx) {
    let two_pi = 6.28318530717959;
    let x_param = p.x;
    let y_param = p.y;
    let x_origin = p.x_origin;
    let y_origin = p.y_origin;
    let z_origin = p.z_origin;
    let r0 = this.st[0];
    let r1 = this.st[1];
    let r2 = this.st[2];
    let r3 = this.st[3];
    let n_idx = this.st[4];
    let dx = ctx.accum.re - x_origin;
    let dy = ctx.accum.im - y_origin;
    let dz = -z_origin;
    let r = (dx * dx + dy * dy + dz * dz) * (r0 + r1 + r2 + r3 - 2.0);
    let new_sample = rand();
    let n = u32(n_idx) & u32(3);
    this.st[to_int(n)] = new_sample;
    this.st[4] = to_float((n + u32(1)) & u32(3));
    let u = rand() * two_pi;
    let v = rand() * two_pi;
    let sv = sin(v);
    complex(x_param * r * sv * cos(u), y_param * r * sv * sin(u))
}

fn variation3(q, p, ctx) {
    let two_pi = 6.28318530717959;
    let x_param = p.x;
    let y_param = p.y;
    let z_param = p.z;
    let x_origin = p.x_origin;
    let y_origin = p.y_origin;
    let z_origin = p.z_origin;
    let r0 = this.st[0];
    let r1 = this.st[1];
    let r2 = this.st[2];
    let r3 = this.st[3];
    let n_idx = this.st[4];
    let dx = ctx.accum.x - x_origin;
    let dy = ctx.accum.y - y_origin;
    let dz = ctx.accum.z - z_origin;
    let r = (dx * dx + dy * dy + dz * dz) * (r0 + r1 + r2 + r3 - 2.0);
    let new_sample = rand();
    let n = u32(n_idx) & u32(3);
    this.st[to_int(n)] = new_sample;
    this.st[4] = to_float((n + u32(1)) & u32(3));
    let u = rand() * two_pi;
    let v = rand() * two_pi;
    let sv = sin(v);
    let cv = cos(v);
    vec3(x_param * r * sv * cos(u), y_param * r * sv * sin(u), z_param * r * cv)
}
