// Julia, the built-in variation's twin: the square root of the point, one of its two roots at random.

const INFO = #{ title: "Julia", name: "rhai_julia" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.im, z.re);
    let sqrt_r = sqrt(r);
    let omega = if rand() < 0.5 { 3.14159265359 } else { 0.0 };
    let half_theta = theta / 2.0 + omega;
    complex(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta))
}
