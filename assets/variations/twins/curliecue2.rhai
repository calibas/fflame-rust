/// Self-iterating Curlicue walker — ignores the input position and emits a
/// deterministic point sequence from a 4-slot per-thread state machine `(x,
/// y, θ, φ)`. Each iteration advances `(x, y)` by `0.001 · (cos φ, sin φ)`,
/// then updates `φ ← (θ + φ) mod 2π` and `θ ← (θ + 2π·speed) mod 2π`. With
/// `speed` set to an irrational fraction (e.g. golden ratio), the
/// trajectory traces the classical curlicue fractal of Berry and Goldberg.
/// Upstream cpp randomizes `speed` once at flame init via `GOODRAND_01`; we
/// expose it as a user parameter for reproducibility.
///
/// # Authors
/// - Jesus Sosa

// Curliecue 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    speed: #{
        value: 0.5, min: 0.0, max: 1.0,
        caption: "Speed",
        hint: "Angular velocity of the curlicue walker as a fraction of 2π per iteration. Small irrational values (e.g. golden-ratio fractions) produce the classical curlicue fractal pattern; rational values produce closed loops.",
    },
};

const INFO = #{ title: "Curliecue 2", name: "rhai_curliecue2", category: "advanced_2d" };

fn init(p) {
    #{ x0: 0.0, y0: 0.0, theta: 0.0, phi: 0.0 }
}

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let speed = p.speed;
    let x0 = this.x0;
    let y0 = this.y0;
    let theta = this.theta;
    let phi = this.phi;
    let x1 = x0 + 0.001 * cos(phi);
    let y1 = y0 + 0.001 * sin(phi);
    x0 = x1;
    y0 = y1;
    let new_phi = (theta + phi) - to_float(floor((theta + phi) / two_pi)) * two_pi;
    let new_theta = (theta + two_pi * speed) - to_float(floor((theta + two_pi * speed) / two_pi)) * two_pi;
    this.x0 = x0;
    this.y0 = y0;
    this.theta = new_theta;
    this.phi = new_phi;
    complex(x0, y0)
}
