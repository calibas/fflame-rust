"""The twins' inverses, written by hand (docs/archive/projects/rhai-planners.md, B1).

The Flame Attractor walks a flame backwards and needs each variation's
exact inverse. Twenty-six built-ins carry theirs in Rust
(`variations::inverse`); a twin carries its own in Rhai, `fn inverse(z, p,
branch)`, so an edited twin keeps one while it still undoes the twin's
forward map -- checked when it is used (`variations::user_inverse::check`).
**A built-in with none in Rust takes its twin's** (`user_inverse::of`), so
adding an entry here gives a built-in the Flame Attractor: write the
inverse, run this script, and the twins' test
(`the_twins_inverses_undo_their_maps`) checks it; the GPU gates are
`a_twins_own_inverse_renders_the_flame_attractor` and
`a_builtin_borrows_its_twins_inverse` (escape::ifs).

Where it has no preimage an inverse gives `no_preimage()`; where its image
is not the whole plane, a `fn image_gap` (GAPS below) scores a point off
it. Splits' has none on three of its four branches at any point, which
mode D read as the set until the walk's radius stopped flushing a
no-preimage point's to zero (rhai-planners D4).

Each is transcribed from the built-in kernel's inverse in
`scene/ifs_analysis.rs` (`kernel_inverse_gen`), against the TWIN's forward
map where the two differ (spherical's 1e-6). A kernel whose branch count
the analysis reads off the ball (disc's rings, cylinder's turns) gets a
fixed count here: disc 12 rings, the analysis's cap; cylinder five turns
either side of zero, -2 to 2.

`translate_variation_twins.py` appends them as it writes a twin, so a
retranslation keeps them; `python scripts/twin_inverses.py` appends them to
the twins written before they existed.

  python scripts/twin_inverses.py    # append each to its twin, where it has none
"""
import re

TAU = "6.28318530717959"
PI = "3.14159265358979"

# name -> (INFO.inverse_branches, the function)
INVERSES = {
    "julia": (1, """fn inverse(z, p, branch) {
    z * z
}
"""),
    "julian": (1, """fn inverse(z, p, branch) {
    let power = to_float(p.power);
    let rr = pow(z.abs(), abs(power) / p.dist);
    let a = power * atan2(z.im, z.re);
    complex(rr * cos(a), rr * sin(a))
}
"""),
    # Its odd arms take the conjugate's root: the arm is v's sector's.
    "juliascope": (1, f"""fn inverse(z, p, branch) {{
    let power = to_int(to_float(p.power));
    let n = to_float(power);
    let m = abs(power);
    let th = atan2(z.im, z.re);
    let k = ((to_int(round(n * th / {TAU})) % m) + m) % m;
    let rr = pow(z.abs(), n / p.dist);
    let a = n * th;
    if (k & 1) == 1 {{
        complex(rr * cos(a), -rr * sin(a))
    }} else {{
        complex(rr * cos(a), rr * sin(a))
    }}
}}
"""),
    # The twin's 1e-6 kept: `z / (|z|^2 + 1e-6)` is two-to-one near the
    # origin, and this is its outer preimage, the one past 1e-3.
    "spherical": (1, """fn inverse(z, p, branch) {
    let x = z.norm_sqr();
    let d = 1.0 - 4.0e-6 * x;
    if d < 0.0 || x <= 0.0 {
        return no_preimage();
    }
    z * ((1.0 + sqrt(d)) / (2.0 * x))
}
"""),
    # The inner preimage (branch 0) or the outer, written so nothing
    # cancels (`Kernel::bubble_scale`).
    "bubble": (2, """fn inverse(z, p, branch) {
    let x = z.norm_sqr();
    if x > 1.0 {
        return no_preimage();
    }
    let root = sqrt(max(0.0, 1.0 - x));
    if branch == 0 {
        z * (2.0 / (1.0 + root))
    } else {
        z * (2.0 * (1.0 + root) / max(x, 1.0e-30))
    }
}
"""),
    "hemisphere": (1, """fn inverse(z, p, branch) {
    let x = z.norm_sqr();
    if x >= 1.0 {
        return no_preimage();
    }
    z / sqrt(1.0 - x)
}
"""),
    # The ring `branch`: its parity is the sign of the angle.
    "disc": (12, f"""fn inverse(z, p, branch) {{
    let rho = z.abs();
    if rho > 1.0 {{
        return no_preimage();
    }}
    let r = atan2(z.re, z.im) / {PI} + to_float(branch);
    if r < 0.0 {{
        return no_preimage();
    }}
    let theta = if (branch & 1) == 0 {{ {PI} * rho }} else {{ -{PI} * rho }};
    complex(r * sin(theta), r * cos(theta))
}}
"""),
    # A reflection in the diagonal over the angular scale.
    "blob": (1, """fn inverse(z, p, branch) {
    let th = atan2(z.im, z.re);
    let s = p.low + ((p.high - p.low) / 2.0) * (sin(to_float(p.waves) * th) + 1.0);
    complex(z.im / s, z.re / s)
}
"""),
    "elliptic": (1, """fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 {
        return no_preimage();
    }
    let h = 1.5707963267949;
    let e = exp(abs(z.im) * h) - 1.0;
    let s = 2.0 * e / (sqrt(1.0 + 4.0 * e) + 1.0);
    let m = s * s;
    let y = s * sqrt(m + 2.0) * cos(z.re * h);
    complex(sin(z.re * h) * (1.0 + m), if z.im < 0.0 { -y } else { y })
}
"""),
    # One branch per quadrant of the preimage: bit 0 x >= 0, bit 1 y >= 0;
    # none where the shifted point is not in the branch's quadrant, as the
    # built-in kernel (`Kernel::Splits`).
    "splits": (4, """fn inverse(z, p, branch) {
    let east = (branch & 1) == 1;
    let north = (branch & 2) == 2;
    let dx = (if east { p.x } else { -p.x }) + (if north { p.ushear } else { -p.dshear });
    let dy = (if north { p.y } else { -p.y }) + (if east { p.rshear } else { -p.lshear });
    let u = complex(z.re - dx, z.im - dy);
    if (u.re >= 0.0) == east && (u.im >= 0.0) == north {
        u
    } else {
        no_preimage()
    }
}
"""),
    # Two preimages a turn, asin(x) and pi - asin(x), over turns -2 to 2.
    "cylinder": (10, f"""fn inverse(z, p, branch) {{
    if abs(z.re) > 1.0 {{
        return no_preimage();
    }}
    let turn = to_float(branch / 2) - 2.0;
    let s = asin(z.re);
    let x = if (branch & 1) == 0 {{ s }} else {{ {PI} - s }};
    complex(x + {TAU} * turn, z.im)
}}
"""),
    # A rotation by |z|^2, which keeps |z|.
    "swirl": (1, """fn inverse(z, p, branch) {
    let r2 = z.norm_sqr();
    let s = sin(r2);
    let c = cos(r2);
    complex(z.re * s + z.im * c, z.im * s - z.re * c)
}
"""),
    # z^2 / (|z| + 1e-6): the square's two roots, the radius from the
    # quadratic its 1e-6 makes.
    "horseshoe": (2, f"""fn inverse(z, p, branch) {{
    let s = z.abs();
    let r = (s + sqrt(s * s + 4.0e-6 * s)) / 2.0;
    let th = atan2(z.im, z.re) / 2.0 + (if branch == 0 {{ 0.0 }} else {{ {PI} }});
    complex(r * cos(th), r * sin(th))
}}
"""),
    # (angle from +y over pi, radius - 1), with the twin's own pi.
    "polar": (1, """fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 || z.im < -1.0 {
        return no_preimage();
    }
    let th = 3.14159265359 * z.re;
    let r = z.im + 1.0;
    complex(r * sin(th), r * cos(th))
}
"""),
    # e^(x - 1) at the angle pi*y: y's turns -2 to 2.
    "exponential": (5, f"""fn inverse(z, p, branch) {{
    let m = z.abs();
    if m <= 0.0 {{
        return no_preimage();
    }}
    complex(1.0 + ln(m), atan2(z.im, z.re) / {PI} + 2.0 * (to_float(branch) - 2.0))
}}
"""),
    # The complex exponential: its logarithm, turns -1 to 1.
    "exp": (3, f"""fn inverse(z, p, branch) {{
    let m = z.abs();
    if m <= 0.0 {{
        return no_preimage();
    }}
    complex(ln(m), atan2(z.im, z.re) + {TAU} * (to_float(branch) - 1.0))
}}
"""),
    # (log_base |z|, the angle): one turn of the angle, its strip.
    "log": (1, """fn inverse(z, p, branch) {
    let denom = p.denom;
    let r = exp(z.re / denom * 0.5);
    complex(r * cos(z.im), r * sin(z.im))
}
"""),
    # 2r/(r + 1) on the swapped point, its radius from the quadratic the
    # twin's 1e-6 makes; onto the disc of radius 2.
    "fisheye": (1, """fn inverse(z, p, branch) {
    let s = z.abs();
    if s >= 2.0 {
        return no_preimage();
    }
    if s <= 0.0 {
        return complex(0.0, 0.0);
    }
    let e = 1.0e-6;
    let b = s * (1.0 + e);
    let r0 = (b + sqrt(b * b + 4.0 * (2.0 - s) * s * e)) / (2.0 * (2.0 - s));
    complex(z.im * r0 / s, z.re * r0 / s)
}
"""),
    # 2z/(|z| + 1), onto the disc of radius 2.
    "eyefish": (1, """fn inverse(z, p, branch) {
    let s = z.abs();
    if s >= 2.0 {
        return no_preimage();
    }
    z / (2.0 - s)
}
"""),
    # (x / (|z|^2 + 1e-6), y): y kept, x a root of u x^2 - x + u k = 0 with
    # k = y^2 + 1e-6 -- the small root, through the origin (branch 0), or
    # the large; none where 4 u^2 k > 1, nor on the large one at u = 0.
    "hyperbolic": (2, """fn inverse(z, p, branch) {
    let u = z.re;
    let k = z.im * z.im + 1.0e-6;
    let d = 1.0 - 4.0 * u * u * k;
    if d < 0.0 || (branch == 1 && u == 0.0) {
        return no_preimage();
    }
    let s = 1.0 + sqrt(d);
    let x = if branch == 0 { 2.0 * u * k / s } else { s / (2.0 * u) };
    complex(x, z.im)
}
"""),
    # sin on each axis: per axis two preimages a turn over turns -1 and
    # 0, four, and their sixteen pairs.
    "sinusoidal": (16, f"""fn inverse(z, p, branch) {{
    if abs(z.re) > 1.0 || abs(z.im) > 1.0 {{
        return no_preimage();
    }}
    let bx = branch % 4;
    let by = branch / 4;
    let sx = asin(z.re);
    let sy = asin(z.im);
    let x = (if (bx & 1) == 0 {{ sx }} else {{ {PI} - sx }}) + {TAU} * (to_float(bx / 2) - 1.0);
    let y = (if (by & 1) == 0 {{ sy }} else {{ {PI} - sy }}) + {TAU} * (to_float(by / 2) - 1.0);
    complex(x, y)
}}
"""),
}


# name -> `fn image_gap(z, p)`: how far a point lies outside the forward
# map's image, a lower bound, for a twin whose inverse has no preimage
# there -- the walk scores such a point by it, as it does the built-ins'
# unit discs (`NonlinearMap2::image_gap`).
GAPS = {
    "bubble": """fn image_gap(z, p) {
    z.abs() - 1.0
}
""",
    "hemisphere": """fn image_gap(z, p) {
    z.abs() - 1.0
}
""",
    "disc": """fn image_gap(z, p) {
    z.abs() - 1.0
}
""",
    "elliptic": """fn image_gap(z, p) {
    abs(z.re) - 1.0
}
""",
    "cylinder": """fn image_gap(z, p) {
    abs(z.re) - 1.0
}
""",
    "polar": """fn image_gap(z, p) {
    max(abs(z.re) - 1.0, -1.0 - z.im)
}
""",
    "log": f"""fn image_gap(z, p) {{
    abs(z.im) - {PI}
}}
""",
    "fisheye": """fn image_gap(z, p) {
    z.abs() - 2.0
}
""",
    "eyefish": """fn image_gap(z, p) {
    z.abs() - 2.0
}
""",
    "sinusoidal": """fn image_gap(z, p) {
    max(abs(z.re), abs(z.im)) - 1.0
}
""",
    # The image is |u| sqrt(v^2 + 1e-6) <= 1/2, inside |u v| <= 1/2. A point
    # (a, b) = (|u|, |v|) d from that: a box of half-side d about it holds
    # the nearest, whose |x y| >= (a - d)(b - d), so d is at least the small
    # root of (a - d)(b - d) = 1/2, written so nothing cancels.
    "hyperbolic": """fn image_gap(z, p) {
    let a = abs(z.re);
    let b = abs(z.im);
    2.0 * (a * b - 0.5) / (a + b + sqrt((a - b) * (a - b) + 2.0))
}
""",
}


# name -> `fn inverse3(v, p)`: a 3D twin's inverse for the solid (B3), one
# branch, as `scene::ifs_analysis::Kernel3`'s are.
INVERSES3 = {
    # julia3D: |out| = rho^(1/n) at the same elevation, azimuth over n --
    # undone by rho = |v|^n, the azimuth times n, z scaled back by |n|.
    "julia3D": """fn inverse3(v, p) {
    let pf = to_float(p.power);
    if to_int(pf) == 0 {
        return v;
    }
    let m = sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
    if m <= 0.0 {
        return vec3(0.0, 0.0, 0.0);
    }
    let rho = pow(m, pf);
    let rxy = sqrt(v.x * v.x + v.y * v.y);
    let th = pf * atan2(v.y, v.x);
    let s = rho * rxy / m;
    vec3(s * cos(th), s * sin(th), rho * v.z / m * abs(pf))
}
""",
    # julia3Dz: the plane's root on xy, z by r^(1 - n)/|n|; its twin's
    # special powers kept -- 1 the identity, -1 an inversion and -2 a root,
    # both conjugated.
    "julia3Dz": """fn inverse3(v, p) {
    let power = to_int(to_float(p.power));
    if power == 1 {
        return v;
    }
    let ro = v.x * v.x + v.y * v.y;
    if power == -1 {
        return vec3(v.x / ro, -v.y / ro, v.z / ro);
    }
    let pf = to_float(power);
    let r = sqrt(ro);
    let th = if power == -2 { -pf * atan2(v.y, v.x) } else { pf * atan2(v.y, v.x) };
    let rxy = pow(r, pf);
    vec3(rxy * cos(th), rxy * sin(th), v.z * abs(pf) * rxy / r)
}
""",
}


def with_inverse(name, text):
    """The twin's text with its inverse and image gap, where it has them and
    does not yet: the functions at the end, its branch count in `INFO`."""
    if name in INVERSES and "fn inverse(" not in text:
        branches, fn = INVERSES[name]
        if branches != 1:
            text = re.sub(r"(const INFO = #\{ .*?) \};", lambda m: m.group(1) + f", inverse_branches: {branches}" + " };", text, count=1)
        text = text.rstrip("\n") + "\n\n" + fn
    if name in GAPS and "fn image_gap(" not in text:
        text = text.rstrip("\n") + "\n\n" + GAPS[name]
    if name in INVERSES3 and "fn inverse3(" not in text:
        text = text.rstrip("\n") + "\n\n" + INVERSES3[name]
    return text


if __name__ == "__main__":
    for name in list(INVERSES) + [n for n in INVERSES3 if n not in INVERSES]:
        path = f"assets/variations/twins/{name}.rhai"
        text = open(path, encoding="utf-8").read()
        new = with_inverse(name, text)
        if new != text:
            open(path, "w", encoding="utf-8", newline="\n").write(new)
            print(f"  {name}: inverse or image gap appended")
