/// 3D version of `pointgrid_wf` — same algorithm extended to a 3D `xcount ×
/// ycount × zcount` grid spanning `[xmin, xmax] × [ymin, ymax] × [zmin,
/// zmax]`. Each axis gets its own hashed jitter offset.
///
/// # Authors
/// - Andreas Maschke

// Point Grid 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xmin: #{
        value: -3.0, min: -100.0, max: 100.0, free: true,
        caption: "X Min",
        hint: "Left edge of the grid region on the X axis.",
    },
    xmax: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "X Max",
        hint: "Right edge of the grid region on the X axis.",
    },
    xcount: #{
        value: 10, min: 1, max: 1000,
        caption: "X Count",
        hint: "Number of grid cells along X.",
    },
    ymin: #{
        value: -3.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Min",
        hint: "Bottom edge of the grid region on the Y axis.",
    },
    ymax: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Max",
        hint: "Top edge of the grid region on the Y axis.",
    },
    ycount: #{
        value: 10, min: 1, max: 1000,
        caption: "Y Count",
        hint: "Number of grid cells along Y.",
    },
    zmin: #{
        value: -1.0, min: -100.0, max: 100.0, free: true,
        caption: "Z Min",
        hint: "Floor edge of the grid region on the Z axis.",
    },
    zmax: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Z Max",
        hint: "Ceiling edge of the grid region on the Z axis.",
    },
    zcount: #{
        value: 10, min: 1, max: 1000,
        caption: "Z Count",
        hint: "Number of grid cells along Z.",
    },
    distortion: #{
        value: 2.3, min: -10.0, max: 10.0, free: true,
        caption: "Distortion",
        hint: "Per-cell jitter amplitude. 0 = no jitter (pure grid); larger = more scatter around each cell center.",
    },
    seed: #{
        value: 1234, min: -100000, max: 100000,
        caption: "Seed",
        hint: "Hash seed for the per-cell jitter — changing it reshuffles the jitter pattern.",
    },
};

const INFO = #{ title: "Point Grid 3D WF", name: "rhai_pointgrid3d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let xc = max(1.0, to_float(p.xcount));
    let yc = max(1.0, to_float(p.ycount));
    let zc = max(1.0, to_float(p.zcount));
    #{ dx: (p.xmax - p.xmin) / xc, dy: (p.ymax - p.ymin) / yc, dz: (p.zmax - p.zmin) / zc }
}

fn pg_disc_noise(x, y) {
    let n = x + y * 57;
    n = (n << u32(13)) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    to_float(h) * (1.0 / 2147483647.0)
}

fn variation(z, p) {
    let xmin = p.xmin;
    let xcount = max(to_int(to_float(p.xcount)), 1);
    let ymin = p.ymin;
    let ycount = max(to_int(to_float(p.ycount)), 1);
    let distortion = p.distortion;
    let seed = to_int(to_float(p.seed));
    let dx = p.dx;
    let dy = p.dy;
    let x_idx = clamp(to_int(rand() * to_float(xcount)), 0, xcount - 1);
    let y_idx = clamp(to_int(rand() * to_float(ycount)), 0, ycount - 1);
    let x = xmin + dx * to_float(x_idx);
    let y = ymin + dy * to_float(y_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, y_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        x = x + distx;
        y = y + disty;
    }
    complex(x, y)
}

fn variation3(v, p) {
    let xmin = p.xmin;
    let xcount = max(to_int(to_float(p.xcount)), 1);
    let ymin = p.ymin;
    let ycount = max(to_int(to_float(p.ycount)), 1);
    let zmin = p.zmin;
    let zcount = max(to_int(to_float(p.zcount)), 1);
    let distortion = p.distortion;
    let seed = to_int(to_float(p.seed));
    let dx = p.dx;
    let dy = p.dy;
    let dz = p.dz;
    let x_idx = clamp(to_int(rand() * to_float(xcount)), 0, xcount - 1);
    let y_idx = clamp(to_int(rand() * to_float(ycount)), 0, ycount - 1);
    let z_idx = clamp(to_int(rand() * to_float(zcount)), 0, zcount - 1);
    let x = xmin + dx * to_float(x_idx);
    let y = ymin + dy * to_float(y_idx);
    let z = zmin + dz * to_float(z_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, z_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        let distz = (0.5 - pg_disc_noise(seed + 4761 + z_idx, y_idx)) * distortion;
        x = x + distx;
        y = y + disty;
        z = z + distz;
    }
    vec3(x, y, z)
}
