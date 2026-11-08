"""The twins' inverses, written by hand (docs/projects/rhai-planners.md, B1).

The Flame Attractor walks a flame backwards and needs each variation's
exact inverse. The built-ins carry theirs in Rust (`variations::inverse`);
a twin carries its own in Rhai, `fn inverse(z, p, branch)`, so an edited
twin keeps one while it still undoes the twin's forward map -- checked
when it is used (`variations::user_inverse::check`).

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
    # One branch per quadrant of the preimage: bit 0 x >= 0, bit 1 y >= 0.
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
    return text


if __name__ == "__main__":
    for name in INVERSES:
        path = f"assets/variations/twins/{name}.rhai"
        text = open(path, encoding="utf-8").read()
        new = with_inverse(name, text)
        if new != text:
            open(path, "w", encoding="utf-8", newline="\n").write(new)
            print(f"  {name}: inverse or image gap appended")
