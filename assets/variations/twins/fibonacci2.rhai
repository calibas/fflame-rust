/// Binet-formula complex Fibonacci — extends the Fibonacci sequence to
/// complex inputs via the closed form `F(z) = (φ^z − (−φ)^(−z)) / √5`, with
/// `sc` scaling the magnitude and `sc2` scaling the polar-radius
/// exponential growth rate. Produces spiral patterns based on the golden
/// ratio.
///
/// # Authors
/// - Larry Berlin

// Fibonacci 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sc: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Output magnitude scale.",
    },
    sc2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale 2",
        hint: "Polar-radius exponential scale — controls how quickly the magnitude grows along X.",
    },
};

const INFO = #{ title: "Fibonacci 2", name: "rhai_fibonacci2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let sc = p.sc;
    let sc2 = p.sc2;
    let pi = 3.14159265358979;
    let ffive = 0.4472135954999580;
    let fnatlog = 0.4812118250596034;
    let a = z.im * fnatlog;
    let snum1 = sin(a);
    let cnum1 = cos(a);
    let b = -(z.re * pi + z.im * fnatlog);
    let snum2 = sin(b);
    let cnum2 = cos(b);
    let eradius1 = sc * exp(sc2 * (z.re * fnatlog));
    let eradius2 = sc * exp(sc2 * (-(z.re * fnatlog - z.im * pi)));
    complex((eradius1 * cnum1 - eradius2 * cnum2) * ffive, (eradius1 * snum1 - eradius2 * snum2) * ffive)
}

fn variation3(v, p) {
    let sc = p.sc;
    let sc2 = p.sc2;
    let pi = 3.14159265358979;
    let ffive = 0.4472135954999580;
    let fnatlog = 0.4812118250596034;
    let a = v.y * fnatlog;
    let snum1 = sin(a);
    let cnum1 = cos(a);
    let b = -(v.x * pi + v.y * fnatlog);
    let snum2 = sin(b);
    let cnum2 = cos(b);
    let eradius1 = sc * exp(sc2 * (v.x * fnatlog));
    let eradius2 = sc * exp(sc2 * (-(v.x * fnatlog - v.y * pi)));
    vec3((eradius1 * cnum1 - eradius2 * cnum2) * ffive, (eradius1 * snum1 - eradius2 * snum2) * ffive, v.z)
}
