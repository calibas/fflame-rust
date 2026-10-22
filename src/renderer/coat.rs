//! The material's coat, as every tier evaluates it (docs/projects/
//! 3d-look-and-tools.md, P4): its directional albedo at reflectance 1, a
//! fit to the coat's own integral, and the energy compensation built on
//! it -- WGSL generated from the one fit for each shader that lights a
//! coat: the escape and simulation rig's (`ifs_coat_`, both tiers) and
//! the 3D flame's shade pass's (`coat_`). Here, not in the escape engine,
//! because the flame's shade pass is built without it.

/// The coat's directional albedo at reflectance 1 -- the share of the
/// light one scattering off its microfacets keeps, seen at cos `mu`, at
/// roughness `r` -- for the coat as every tier evaluates it: GGX, Smith's
/// separable masking, sampled by the visible normals (3d-look-and-tools
/// P4). Karis's EnvBRDFApprox, fitted to height-correlated masking, was
/// up to half off for ours. A fit, `1 - r^3 P(mu, r)` with P's terms
/// `COAT_ESS_FIT[i][j]` of `mu^i r^j`, to the integral over the whole
/// range of mu (`the_coat_albedo_fit_is_the_integral`): the compensation
/// `1 / E` within 1.6% from mu = 0.2 up, and from -11% to +5% at a glance,
/// where the albedo dips at low roughness as no small polynomial follows.
/// (Fitted from mu = 0.25 and clamped there, it gave a rough metal 24% too
/// much at a glance, where its albedo rises again.)
pub(crate) const COAT_ESS_FIT: [[f32; 4]; 4] = [
    [12.567384, -44.989223, 55.380245, -22.600027],
    [-21.612877, 109.279373, -158.189835, 71.258842],
    [-13.835137, -2.0193479, 52.886459, -37.669228],
    [22.578863, -59.14378, 48.020164, -11.221471],
];

/// [`COAT_ESS_FIT`]'s albedo, as the shaders evaluate it.
#[cfg(test)]
pub(crate) fn coat_ess(mu: f32, r: f32) -> f32 {
    let m = mu.clamp(0.0, 1.0);
    let q = r.clamp(0.0, 1.0);
    let row = |c: [f32; 4]| ((c[3] * q + c[2]) * q + c[1]) * q + c[0];
    let [c0, c1, c2, c3] = COAT_ESS_FIT.map(row);
    (1.0 - q * q * q * (((c3 * m + c2) * m + c1) * m + c0)).min(1.0)
}

/// The coat's albedo ([`coat_ess`]) and the energy it gives back, in
/// WGSL -- `{prefix}ess` and `{prefix}energy` -- from the one fit, so no
/// shader's copy can drift: the rig's (`ifs_coat_`, spliced at
/// `//__COAT_ENERGY__`, which both tiers of every escape and simulation
/// 3D view read) and the 3D flame's shade pass's (`coat_`).
pub(crate) fn coat_energy_wgsl(prefix: &str) -> String {
    let c = COAT_ESS_FIT;
    let row = |k: usize| format!("(({:?} * q + {:?}) * q + {:?}) * q + {:?}", c[k][3], c[k][2], c[k][1], c[k][0]);
    format!(
        r#"// The coat's directional albedo at reflectance 1, seen at cos `mu` at
// roughness `r` (`renderer::coat::coat_ess`, a fit to the integral).
fn {prefix}ess(mu: f32, r: f32) -> f32 {{
    let m = clamp(mu, 0.0, 1.0);
    let q = clamp(r, 0.0, 1.0);
    let c0 = {r0};
    let c1 = {r1};
    let c2 = {r2};
    let c3 = {r3};
    return min(1.0 - q * q * q * (((c3 * m + c2) * m + c1) * m + c0), 1.0);
}}

// What single scattering loses to the light bouncing between the
// microfacets, given back (Kulla and Conty 2017; Turquin 2019): the
// coat's lobe times 1 + F0 (1 / E - 1), so a rough metal keeps its
// brightness.
fn {prefix}energy(f0: vec3<f32>, mu: f32, r: f32) -> vec3<f32> {{
    return vec3<f32>(1.0, 1.0, 1.0) + f0 * (1.0 / max({prefix}ess(mu, r), 0.05) - 1.0);
}}"#,
        r0 = row(0),
        r1 = row(1),
        r2 = row(2),
        r3 = row(3),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The coat's albedo at reflectance 1, integrated as the path tracer
    /// samples it: half-vectors of the visible normals (Heitz 2018), each
    /// worth Smith's masking of the light (`pt_path`'s throughput at F =
    /// 1), in f64.
    fn coat_albedo(mu: f64, r: f64, samples: u32) -> f64 {
        let alpha = (r * r).max(1.0e-4);
        let a2 = alpha * alpha;
        let g1 = |x: f64| 2.0 * x / (x + (a2 + (1.0 - a2) * x * x).sqrt());
        let v = [(1.0 - mu * mu).max(0.0).sqrt(), 0.0, mu];
        let vh = {
            let h = [alpha * v[0], alpha * v[1], v[2]];
            let l = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
            [h[0] / l, h[1] / l, h[2] / l]
        };
        let lensq = vh[0] * vh[0] + vh[1] * vh[1];
        let t1 = if lensq > 0.0 { [-vh[1] / lensq.sqrt(), vh[0] / lensq.sqrt(), 0.0] } else { [1.0, 0.0, 0.0] };
        let t2 = [vh[1] * t1[2] - vh[2] * t1[1], vh[2] * t1[0] - vh[0] * t1[2], vh[0] * t1[1] - vh[1] * t1[0]];
        // A low-discrepancy pair (the golden ratio's and the plastic
        // number's sequences): the same estimate every run.
        let (mut sum, g) = (0.0f64, 1.324_717_957_244_746);
        for i in 0..samples {
            let u0 = (0.5 + i as f64 / g).fract();
            let u1 = (0.5 + i as f64 / (g * g)).fract();
            let rr = u0.sqrt();
            let phi = std::f64::consts::TAU * u1;
            let p1 = rr * phi.cos();
            let s = 0.5 * (1.0 + vh[2]);
            let p2 = (1.0 - s) * (1.0 - p1 * p1).max(0.0).sqrt() + s * rr * phi.sin();
            let pz = (1.0 - p1 * p1 - p2 * p2).max(0.0).sqrt();
            let nh = [0, 1, 2].map(|k| p1 * t1[k] + p2 * t2[k] + pz * vh[k]);
            let h = [alpha * nh[0], alpha * nh[1], nh[2].max(0.0)];
            let hl = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
            let h = h.map(|c| c / hl);
            let vdh = v[0] * h[0] + v[1] * h[1] + v[2] * h[2];
            let lz = 2.0 * vdh * h[2] - v[2];
            if lz > 0.0 {
                sum += g1(lz);
            }
        }
        sum / samples as f64
    }

    /// The fit the shaders compensate the coat's energy by
    /// (`COAT_ESS_FIT`) is the coat's own albedo: its compensation,
    /// `1 / E`, within 2% from mu = 0.2 up, and never more than 6% too
    /// much at a glance (energy from nowhere); at a glance it may give
    /// back too little, which is only the loss as before.
    #[test]
    fn the_coat_albedo_fit_is_the_integral() {
        let mut worst_bulk = 0.0f64;
        let mut worst_gain = f64::MIN;
        for r in [0.05, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0] {
            for mu in [0.02, 0.08, 0.2, 0.35, 0.5, 0.7, 0.9, 1.0] {
                let e = coat_albedo(mu, r, 200_000);
                let gain = e / (coat_ess(mu as f32, r as f32) as f64).max(0.05) - 1.0;
                if mu >= 0.2 {
                    worst_bulk = worst_bulk.max(gain.abs());
                }
                worst_gain = worst_gain.max(gain);
            }
        }
        println!("compensation error from mu 0.2 up {worst_bulk:.4}, most gained at a glance {worst_gain:+.4}");
        assert!(worst_bulk < 0.02, "{worst_bulk}");
        assert!(worst_gain < 0.06, "{worst_gain}");
        // And the integral is what the fit was made from: a rough metal
        // loses most of its light to single scattering's masking.
        assert!((coat_albedo(1.0, 1.0, 200_000) - 0.306).abs() < 0.01);
    }

    /// The WGSL is the fit: generated from it, it names its functions by
    /// the prefix, and holds every coefficient.
    #[test]
    fn the_coat_energy_wgsl_is_the_fit() {
        let w = coat_energy_wgsl("ifs_coat_");
        assert!(w.contains("fn ifs_coat_ess(") && w.contains("fn ifs_coat_energy(") && w.contains("ifs_coat_ess(mu, r)"));
        for row in COAT_ESS_FIT {
            for c in row {
                assert!(w.contains(&format!("{c:?}")), "{c:?}");
            }
        }
    }
}
