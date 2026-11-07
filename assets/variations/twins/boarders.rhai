/// Cell-grid border warp — each input point is decomposed into a cell
/// index and an offset within the cell. A coin flip per iteration either
/// shrinks the offset toward the cell center or pushes it toward the
/// nearest cell border.
///
/// # Authors
/// - Apophysis Plugin Pack

// Boarders, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Boarders", name: "rhai_boarders", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let round_x = round_even(z.re);
    let round_y = round_even(z.im);
    let off_x = z.re - round_x;
    let off_y = z.im - round_y;
    if (rand() >= 0.75) {
        return complex(off_x * 0.5 + round_x, off_y * 0.5 + round_y);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return complex(off_x * 0.5 + round_x + 0.25, off_y * 0.5 + round_y + 0.25 * off_y / off_x);
        }
        return complex(off_x * 0.5 + round_x - 0.25, off_y * 0.5 + round_y - 0.25 * off_y / off_x);
    }
    if (off_y >= 0.0) {
        return complex(off_x * 0.5 + round_x + off_x / off_y * 0.25, off_y * 0.5 + round_y + 0.25);
    }
    complex(off_x * 0.5 + round_x - off_x / off_y * 0.25, off_y * 0.5 + round_y - 0.25)
}

fn variation3(v, p) {
    let round_x = round_even(v.x);
    let round_y = round_even(v.y);
    let off_x = v.x - round_x;
    let off_y = v.y - round_y;
    if (rand() >= 0.75) {
        return vec3(off_x * 0.5 + round_x, off_y * 0.5 + round_y, v.z);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec3(off_x * 0.5 + round_x + 0.25, off_y * 0.5 + round_y + 0.25 * off_y / off_x, v.z);
        }
        return vec3(off_x * 0.5 + round_x - 0.25, off_y * 0.5 + round_y - 0.25 * off_y / off_x, v.z);
    }
    if (off_y >= 0.0) {
        return vec3(off_x * 0.5 + round_x + off_x / off_y * 0.25, off_y * 0.5 + round_y + 0.25, v.z);
    }
    vec3(off_x * 0.5 + round_x - off_x / off_y * 0.25, off_y * 0.5 + round_y - 0.25, v.z)
}
