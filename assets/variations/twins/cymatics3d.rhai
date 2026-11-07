/// 3D standing-wave cymatics attractor (the chladni idea in three
/// dimensions).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Cymatics 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m: #{
        value: 3.0, min: 1.0, max: 20.0,
        caption: "Mode M",
        hint: "First mode number of the resonator. Integer values give clean standing-wave structure; fractional values morph between figures.",
    },
    n: #{
        value: 5.0, min: 1.0, max: 20.0,
        caption: "Mode N",
        hint: "Second mode number.",
    },
    l: #{
        value: 2.0, min: 1.0, max: 20.0,
        caption: "Mode L",
        hint: "Third mode number. The three modes are cycled across x/y/z by the three field terms, so all three matter in every axis.",
    },
    a: #{
        value: 1.0, min: -2.0, max: 2.0,
        caption: "Mix A",
        hint: "Amplitude of the first term, s(m,x)·s(n,y)·s(l,z).",
    },
    b: #{
        value: 0.7, min: -2.0, max: 2.0,
        caption: "Mix B",
        hint: "Amplitude of the second (cyclically permuted) term. The interference between terms is what bends the nodal surfaces — with b = c = 0 you get a plain rectangular cell grid. In Heightfield mode this is the second term of the 2D wave.",
    },
    c: #{
        value: 0.5, min: -2.0, max: 2.0,
        caption: "Mix C",
        hint: "Amplitude of the third cyclic term. Unused in Heightfield mode.",
    },
    size: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Size",
        hint: "Spatial scale: one period of the field spans 2·size units in each axis.",
    },
    mode: #{
        value: 0, choices: ["Nodal Surface", "Heightfield"],
        caption: "Mode",
        hint: "Nodal Surface: Newton-project points onto the 3D nodal surfaces of the resonator (the 3D sand-on-a-plate analogue; pairs well with solid rendering). Heightfield: displace z toward the 2D plate field — a standing water-wave surface; x/y pass through.",
    },
    walls: #{
        value: 0, choices: ["Antinode (Free)", "Node (Fixed)"],
        caption: "Walls",
        hint: "Boundary condition of the wave basis. Antinode (cosine): coordinate planes are antinodes — the nodal set is purely curved surfaces, like a real free-edged plate. Node (sine): the origin planes and their period grid are themselves nodes — adds flat planes through the pattern.",
    },
    height: #{
        value: 0.5, min: -2.0, max: 2.0,
        caption: "Height",
        hint: "Heightfield mode only: amplitude of the z displacement.",
    },
    slice_z: #{
        value: 0.25, min: -2.0, max: 2.0,
        caption: "Slice Z",
        hint: "Slice position. 2D render mode: z of the planar slice through the 3D nodal set. 3D Nodal Surface mode with Slice Thickness > 0: center of the z-slab the attractor is confined to. With Node (sine) walls avoid multiples of size — those planes are themselves nodal and the slice degenerates.",
    },
    slice_thickness: #{
        value: 0.0, min: 0.0, max: 2.0,
        caption: "Slice Thickness",
        hint: "Slab thickness around Slice Z. 2D render mode: 0 cuts a razor-thin plane; > 0 samples each point's slice z randomly within the slab — an X-ray-style projection through it. 3D Nodal Surface mode: 0 disables slicing (full volume); > 0 confines the attractor to the slab.",
    },
    steps: #{
        value: 3, min: 1, max: 6,
        caption: "Steps",
        hint: "Newton iterations toward the nodal surface per call (Nodal Surface mode). 1 is soft; 3+ lands points crisply on the surface.",
    },
    strength: #{
        value: 0.9, min: 0.0, max: 1.0,
        caption: "Strength",
        hint: "Nodal Surface: blend between the untouched input point (0) and the fully projected point (1). Heightfield: blend between the input z and the wave surface — 1 lands points exactly on the surface.",
    },
    jitter: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Jitter",
        hint: "Isotropic random offset added after projection — grain thickness for the surfaces. 0 keeps them razor thin.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Distance", "Amplitude", "Depth"],
        caption: "Color Mode",
        hint: "Direct-color source, applied through the transform's Direct Color slider. Distance: palette position 1 on the nodal set fading to 0 away from it. Amplitude: signed field value — anti-phase regions of the resonator get opposite palette ends (in Heightfield mode this colors by wave height). Depth: colors by the output point's z — reveals depth structure of the surfaces (in 2D, the sampled slice z).",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 4.0,
        caption: "Color Scale",
        hint: "Contrast for the direct-color modes: Distance falloff sharpness, Amplitude saturation.",
    },
};

const INFO = #{ title: "Cymatics 3D", name: "rhai_cymatics3d", category: "advanced_2d" };

fn cymatics3d_field(q, m, n, l, a, b, c, size, cos_basis) {
    let pi = 3.14159265359;
    let s = q / size * pi;
    let mx_s = sin(m * s.x);
    let mx_c = cos(m * s.x);
    let nx_s = sin(n * s.x);
    let nx_c = cos(n * s.x);
    let lx_s = sin(l * s.x);
    let lx_c = cos(l * s.x);
    let my_s = sin(m * s.y);
    let my_c = cos(m * s.y);
    let ny_s = sin(n * s.y);
    let ny_c = cos(n * s.y);
    let ly_s = sin(l * s.y);
    let ly_c = cos(l * s.y);
    let mz_s = sin(m * s.z);
    let mz_c = cos(m * s.z);
    let nz_s = sin(n * s.z);
    let nz_c = cos(n * s.z);
    let lz_s = sin(l * s.z);
    let lz_c = cos(l * s.z);
    let smx = (if cos_basis { mx_c } else { mx_s });
    let dmx = (if cos_basis { -mx_s } else { mx_c });
    let snx = (if cos_basis { nx_c } else { nx_s });
    let dnx = (if cos_basis { -nx_s } else { nx_c });
    let slx = (if cos_basis { lx_c } else { lx_s });
    let dlx = (if cos_basis { -lx_s } else { lx_c });
    let smy = (if cos_basis { my_c } else { my_s });
    let dmy = (if cos_basis { -my_s } else { my_c });
    let sny = (if cos_basis { ny_c } else { ny_s });
    let dny = (if cos_basis { -ny_s } else { ny_c });
    let sly = (if cos_basis { ly_c } else { ly_s });
    let dly = (if cos_basis { -ly_s } else { ly_c });
    let smz = (if cos_basis { mz_c } else { mz_s });
    let dmz = (if cos_basis { -mz_s } else { mz_c });
    let snz = (if cos_basis { nz_c } else { nz_s });
    let dnz = (if cos_basis { -nz_s } else { nz_c });
    let slz = (if cos_basis { lz_c } else { lz_s });
    let dlz = (if cos_basis { -lz_s } else { lz_c });
    let k = pi / size;
    let f = a * smx * sny * slz + b * snx * sly * smz + c * slx * smy * snz;
    vec4(f, k * (a * m * dmx * sny * slz + b * n * dnx * sly * smz + c * l * dlx * smy * snz), k * (a * n * smx * dny * slz + b * l * snx * dly * smz + c * m * slx * dmy * snz), k * (a * l * smx * sny * dlz + b * m * snx * sly * dmz + c * n * slx * smy * dnz))
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let m = p.m;
    let n = p.n;
    let l = p.l;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let size = max(1.0e-6, p.size);
    let mode = u32(to_float(p.mode));
    let cos_basis = to_float(p.walls) < 0.5;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    if (mode == u32(1)) {
        return #{ z: z, color: color };
    }
    let cell = size / max(1.0e-3, min(min(m, n), l));
    let max_step = 0.5 * cell;
    let z_eval = slice_z + (rand() - 0.5) * slice_thickness;
    let q = z;
    for i in 0..steps {
        let fd = cymatics3d_field(vec3(q.re, q.im, z_eval), m, n, l, a, b, c, size, cos_basis);
        let g = complex(fd.y, fd.z);
        let step = fd.x * g / (g.norm_sqr() + 1.0e-6);
        let sl = step.abs();
        if (sl > max_step) {
            step = step * (max_step / sl);
        }
        q = q - step;
    }
    if (dc_mode == u32(1)) {
        let fd = cymatics3d_field(vec3(z.re, z.im, z_eval), m, n, l, a, b, c, size, cos_basis);
        let dist = abs(fd.x) / (complex(fd.y, fd.z).abs() + 1.0e-6);
        color = exp(-6.0 * dc_scale * dist / cell);
    } else if (dc_mode == u32(2)) {
        let fd = cymatics3d_field(vec3(z.re, z.im, z_eval), m, n, l, a, b, c, size, cos_basis);
        let f_norm = abs(a) + abs(b) + abs(c) + 1.0e-6;
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
    } else if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * z_eval / size);
    }
    let out = mix(z, q, strength);
    if (jitter > 0.0) {
        out = out + complex(rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let m = p.m;
    let n = p.n;
    let l = p.l;
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let size = max(1.0e-6, p.size);
    let mode = u32(to_float(p.mode));
    let cos_basis = to_float(p.walls) < 0.5;
    let height = p.height;
    let slice_z = p.slice_z;
    let slice_thickness = p.slice_thickness;
    let steps = to_int(to_float(p.steps));
    let strength = p.strength;
    let jitter = p.jitter;
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let pi = 3.14159265359;
    let cell = size / max(1.0e-3, min(min(m, n), l));
    let out = vec3(0.0, 0.0, 0.0);
    if (mode == u32(1)) {
        let s = complex(v.x, v.y) / size * pi;
        let mx_s = sin(m * s.re);
        let mx_c = cos(m * s.re);
        let nx_s = sin(n * s.re);
        let nx_c = cos(n * s.re);
        let my_s = sin(m * s.im);
        let my_c = cos(m * s.im);
        let ny_s = sin(n * s.im);
        let ny_c = cos(n * s.im);
        let smx = (if cos_basis { mx_c } else { mx_s });
        let dmx = (if cos_basis { -mx_s } else { mx_c });
        let snx = (if cos_basis { nx_c } else { nx_s });
        let dnx = (if cos_basis { -nx_s } else { nx_c });
        let smy = (if cos_basis { my_c } else { my_s });
        let dmy = (if cos_basis { -my_s } else { my_c });
        let sny = (if cos_basis { ny_c } else { ny_s });
        let dny = (if cos_basis { -ny_s } else { ny_c });
        let f2d = a * smx * sny + b * snx * smy;
        let t1 = complex(v.x, v.y);
        out = vec3(t1.re, t1.im, mix(v.z, height * f2d, strength));
        if (dc_mode == u32(1)) {
            let k = pi / size;
            let g = complex(k * (a * m * dmx * sny + b * n * dnx * smy), k * (a * n * smx * dny + b * m * snx * dmy));
            let dist = abs(f2d) / (g.abs() + 1.0e-6);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let f_norm = abs(a) + abs(b) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * f2d / f_norm);
        }
    } else {
        let half_t = 0.5 * slice_thickness;
        let max_step = 0.5 * cell;
        let q = v;
        for i in 0..steps {
            let fd = cymatics3d_field(q, m, n, l, a, b, c, size, cos_basis);
            let g = vec3(fd.y, fd.z, fd.w);
            let step = fd.x * g / (dot(g, g) + 1.0e-6);
            let sl = length(step);
            if (sl > max_step) {
                step = step * (max_step / sl);
            }
            q = q - step;
            if (half_t > 0.0) {
                q.z = clamp(q.z, slice_z - half_t, slice_z + half_t);
            }
        }
        if (dc_mode == u32(1)) {
            let fd = cymatics3d_field(v, m, n, l, a, b, c, size, cos_basis);
            let dist = abs(fd.x) / (length(vec3(fd.y, fd.z, fd.w)) + 1.0e-6);
            color = exp(-6.0 * dc_scale * dist / cell);
        } else if (dc_mode == u32(2)) {
            let fd = cymatics3d_field(v, m, n, l, a, b, c, size, cos_basis);
            let f_norm = abs(a) + abs(b) + abs(c) + 1.0e-6;
            color = 0.5 + 0.5 * tanh(2.0 * dc_scale * fd.x / f_norm);
        }
        out = mix(v, q, strength);
    }
    if (dc_mode == u32(3)) {
        color = 0.5 + 0.5 * tanh(2.0 * dc_scale * out.z / size);
    }
    if (jitter > 0.0) {
        out = out + vec3(rand() - 0.5, rand() - 0.5, rand() - 0.5) * (2.0 * jitter);
    }
    #{ z: out, color: color }
}
