/// Axis-aligned 3D crop with optional jitter — constrains points to a 3D
/// box. Outside the box, points either collapse to zero (`zero` on) or
/// scatter back toward the nearest edge with `scatter_area` randomness.
///
/// # Authors
/// - Xyrus02

// Crop 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    left: #{
        value: -1.0, min: -10.0, max: 10.0, free: true,
        caption: "Left",
        hint: "Left bound of the cropping box (X axis).",
    },
    top: #{
        value: -1.0, min: -10.0, max: 10.0, free: true,
        caption: "Top",
        hint: "Top bound (Y axis).",
    },
    floor: #{
        value: -1.0, min: -10.0, max: 10.0, free: true,
        caption: "Floor",
        hint: "Floor bound (Z axis).",
    },
    right: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Right",
        hint: "Right bound (X axis).",
    },
    bottom: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Bottom",
        hint: "Bottom bound (Y axis).",
    },
    ceiling: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Ceiling",
        hint: "Ceiling bound (Z axis).",
    },
    scatter_area: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Scatter Area",
        hint: "Width of the scatter band along each box edge. 0 = snap to edge; 1 = scatter across the full box width.",
    },
    zero: #{
        value: false,
        caption: "Zero",
        hint: "When on, out-of-box points collapse to origin. When off, they scatter back toward the nearest edge.",
    },
};

const INFO = #{ title: "Crop 3D", name: "rhai_crop3d", category: "full_3d", phase: "any", replace: true };

fn prepare(p) {
    let left = p.left;
    let top = p.top;
    let floor = p.floor;
    let right = p.right;
    let bottom = p.bottom;
    let ceiling = p.ceiling;
    #{ xmin: min(left, right), xmax: max(left, right), ymin: min(top, bottom), ymax: max(top, bottom), zmin: min(floor, ceiling), zmax: max(floor, ceiling) }
}

fn variation(z, p) {
    let scatter = p.scatter_area;
    let zero = to_float(p.zero);
    let xmin = p.xmin;
    let xmax = p.xmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let w_range = (xmax - xmin) * 0.5 * scatter;
    let h_range = (ymax - ymin) * 0.5 * scatter;
    let x = z.re;
    let y = z.im;
    let outside = (x < xmin) || (x > xmax) || (y < ymin) || (y > ymax);
    if (outside && zero != 0.0) {
        return complex(0.0, 0.0);
    }
    if (x < xmin) {
        x = xmin + rand() * w_range;
    } else if (x > xmax) {
        x = xmax - rand() * w_range;
    }
    if (y < ymin) {
        y = ymin + rand() * h_range;
    } else if (y > ymax) {
        y = ymax - rand() * h_range;
    }
    complex(x, y)
}

fn variation3(v, p) {
    let scatter = p.scatter_area;
    let zero = to_float(p.zero);
    let xmin = p.xmin;
    let xmax = p.xmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let zmin = p.zmin;
    let zmax = p.zmax;
    let w_range = (xmax - xmin) * 0.5 * scatter;
    let h_range = (ymax - ymin) * 0.5 * scatter;
    let l_range = (zmax - zmin) * 0.5 * scatter;
    let x = v.x;
    let y = v.y;
    let z = v.z;
    let outside = (x < xmin) || (x > xmax) || (y < ymin) || (y > ymax) || (z < zmin) || (z > zmax);
    if (outside && zero != 0.0) {
        return vec3(0.0, 0.0, 0.0);
    }
    if (x < xmin) {
        x = xmin + rand() * w_range;
    } else if (x > xmax) {
        x = xmax - rand() * w_range;
    }
    if (y < ymin) {
        y = ymin + rand() * h_range;
    } else if (y > ymax) {
        y = ymax - rand() * h_range;
    }
    if (z < zmin) {
        z = zmin + rand() * l_range;
    } else if (z > zmax) {
        z = zmax - rand() * l_range;
    }
    vec3(x, y, z)
}
