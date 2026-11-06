/// Yin-yang pattern — inside the unit disc, points get reflected onto the
/// iconic two-droplet curve at the given radius. Optional `dual_t` randomly
/// picks between two rotations per iteration (yin and yang each get their
/// own twist). Outside the disc, points either pass through or get
/// discarded.
///
/// # Authors
/// - DarkBeam

// Yin Yang, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Radius",
        hint: "Radius of the two yin/yang droplet centers, 0-1.",
    },
    ang1: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Ang1",
        hint: "Rotation angle for the first half, in half-turns (multiples of π).",
    },
    ang2: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Ang2",
        hint: "Rotation angle for the second half, in half-turns. Only used when `dual_t` is on.",
    },
    dual_t: #{
        value: true,
        caption: "Dual T",
        hint: "When on, randomly picks between the two rotations each iteration so yin and yang each get their own twist. When off, always uses `ang1`.",
    },
    outside: #{
        value: false,
        caption: "Outside",
        hint: "When on, points outside the unit circle pass through unchanged. When off, they're discarded.",
    },
};

const INFO = #{ title: "Yin Yang", name: "rhai_yin_yang", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let ang1 = p.ang1;
    let ang2 = p.ang2;
    #{ sina: sin(pi * ang1), cosa: cos(pi * ang1), sinb: sin(pi * ang2), cosb: cos(pi * ang2) }
}

fn variation(z, p) {
    let radius = p.radius;
    let dual_t = to_float(p.dual_t);
    let outside = to_float(p.outside);
    let sina = p.sina;
    let cosa = p.cosa;
    let sinb = p.sinb;
    let cosb = p.cosb;
    let r2 = z.re * z.re + z.im * z.im;
    if (r2 < 1.0) {
        let inv = 1.0;
        let rr = radius;
        let nx = 0.0;
        let ny = 0.0;
        if (dual_t > 0.5 && rand() > 0.5) {
            inv = -1.0;
            rr = 1.0 - radius;
            nx = z.re * cosb - z.im * sinb;
            ny = z.re * sinb + z.im * cosb;
        } else {
            nx = z.re * cosa - z.im * sina;
            ny = z.re * sina + z.im * cosa;
        }
        if (ny > 0.0) {
            let t = sqrt(max(0.0, 1.0 - ny * ny));
            let k = nx / max(1.0e-30, t);
            let t1 = (t - 0.5) * 2.0;
            let alfa = (1.0 - k) * 0.5;
            let beta = 1.0 - alfa;
            let dx = alfa * (rr - 1.0);
            let k1 = alfa * rr + beta;
            return complex((t1 * k1 + dx) * inv, sqrt(max(0.0, 1.0 - t1 * t1)) * k1 * inv);
        } else {
            return complex((nx * (1.0 - rr) + rr) * inv, (ny * (1.0 - rr)) * inv);
        }
    } else if (outside > 0.5) {
        return z;
    }
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let radius = p.radius;
    let dual_t = to_float(p.dual_t);
    let outside = to_float(p.outside);
    let sina = p.sina;
    let cosa = p.cosa;
    let sinb = p.sinb;
    let cosb = p.cosb;
    let r2 = v.x * v.x + v.y * v.y;
    if (r2 < 1.0) {
        let inv = 1.0;
        let rr = radius;
        let nx = 0.0;
        let ny = 0.0;
        if (dual_t > 0.5 && rand() > 0.5) {
            inv = -1.0;
            rr = 1.0 - radius;
            nx = v.x * cosb - v.y * sinb;
            ny = v.x * sinb + v.y * cosb;
        } else {
            nx = v.x * cosa - v.y * sina;
            ny = v.x * sina + v.y * cosa;
        }
        if (ny > 0.0) {
            let t = sqrt(max(0.0, 1.0 - ny * ny));
            let k = nx / max(1.0e-30, t);
            let t1 = (t - 0.5) * 2.0;
            let alfa = (1.0 - k) * 0.5;
            let beta = 1.0 - alfa;
            let dx = alfa * (rr - 1.0);
            let k1 = alfa * rr + beta;
            return vec3((t1 * k1 + dx) * inv, sqrt(max(0.0, 1.0 - t1 * t1)) * k1 * inv, v.z);
        } else {
            return vec3((nx * (1.0 - rr) + rr) * inv, (ny * (1.0 - rr)) * inv, v.z);
        }
    } else if (outside > 0.5) {
        return v;
    }
    vec3(0.0, 0.0, v.z)
}
