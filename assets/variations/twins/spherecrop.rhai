/// 3D version of `circlecrop` — tests whether the input lies inside a
/// sphere of radius `radius` centered at `(x, y, z)`. Inside, the input
/// passes through scaled by weight. Outside, behavior depends on `zero`: 1
/// = hide (collapse to origin), 0 = scatter onto the sphere surface (with
/// `scatter_area` randomization).
///
/// # Authors
/// - Xyrus02

// Sphere Crop, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Sphere radius.",
    },
    x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X center of the sphere.",
    },
    y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y center of the sphere.",
    },
    z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z",
        hint: "Z center of the sphere.",
    },
    scatter_area: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Scatter Area",
        hint: "Random scatter band along the sphere surface. 0 = snap to surface; ±1 = scatter across full half-radius.",
    },
    zero: #{
        value: true,
        caption: "Zero",
        hint: "When on, points outside the sphere collapse to the origin. When off, they scatter onto the surface.",
    },
};

const INFO = #{ title: "Sphere Crop", name: "rhai_spherecrop", category: "full_3d", phase: "any" };

fn prepare(p) {
    #{ ca: clamp(p.scatter_area, -1.0, 1.0) }
}

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    let cr = p.radius;
    let x0 = p.x;
    let y0 = p.y;
    let zero = to_int(to_float(p.zero));
    let ca = p.ca;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let dx = z.re - x0;
    let dy = z.im - y0;
    let rad = sqrt(dx * dx + dy * dy);
    let ang = atan2(dy, dx);
    let rdc = cr + rand() * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;
    if (cr0 && esc) {
        hidden = true;
        return #{ z: complex(0.0, 0.0), hide: hidden };
    }
    if (!cr0 && esc) {
        return #{ z: complex((w * rdc * cos(ang) + x0) * inv_w, (w * rdc * sin(ang) + y0) * inv_w), hide: hidden };
    }
    #{ z: complex((w * dx + x0) * inv_w, (w * dy + y0) * inv_w), hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let cr = p.radius;
    let x0 = p.x;
    let y0 = p.y;
    let z0 = p.z;
    let zero = to_int(to_float(p.zero));
    let ca = p.ca;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let dx = v.x - x0;
    let dy = v.y - y0;
    let dz = v.z - z0;
    let rad = sqrt(dx * dx + dy * dy + dz * dz);
    let safe_rad = (if rad < 1.0e-30 { 1.0e-30 } else { rad });
    let ang = atan2(dy, dx);
    let az = acos(clamp(dz / safe_rad, -1.0, 1.0));
    let rdc = cr + rand() * 0.5 * ca;
    let esc = rad > cr;
    let cr0 = zero == 1;
    let s = sin(ang);
    let c = cos(ang);
    let saz = sin(az);
    let caz = cos(az);
    if (cr0 && esc) {
        hidden = true;
        return #{ z: vec3(0.0, 0.0, 0.0), hide: hidden };
    }
    if (!cr0 && esc) {
        return #{ z: vec3((w * rdc * c * saz + x0) * inv_w, (w * rdc * s * saz + y0) * inv_w, (w * rdc * caz + z0) * inv_w), hide: hidden };
    }
    #{ z: vec3((w * dx + x0) * inv_w, (w * dy + y0) * inv_w, (w * dz + z0) * inv_w), hide: hidden }
}
