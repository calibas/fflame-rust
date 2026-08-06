//! Reproduce a census corpus random flame: `random:N` in the report
//! means exactly the config this prints for seed N (post the
//! randomize.rs ordering fix, seeds are stable across processes and
//! platforms).
//!
//!     census_dumpseed 77 > seed77.fflame
//!     variation_probe census seed77.fflame

fn main() {
    use rand::SeedableRng;
    let seed: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(77);
    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(seed);
    let settings = fractal_flame_wgpu::scene::randomize::RandomGeneratorSettings::default();
    let flame = fractal_flame_wgpu::scene::randomize::generate_random_flame_with_rng(&settings, &mut rng);
    let mut config = fractal_flame_wgpu::config::FractalConfig::default();
    config.flame = flame;
    config.deterministic_rng = true;
    println!("{}", serde_json::to_string_pretty(&config).unwrap());
}
