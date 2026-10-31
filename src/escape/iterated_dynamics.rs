//! Iterated Dynamics' orbit maps written in Rhai as flame variations
//! (`assets/formulas/variation-*.rhai`: Hopalong, Martin, Chip,
//! Quadruptwo, Threeply, the Gingerbread Man, Latoocarfian, the KAM
//! torus, Pickover's popcorn, the dynamic system, Pickover's attractor,
//! the Rossler attractor): tests that each steps as the program's code
//! does. The references below are Iterated Dynamics' orbit functions
//! (the GPL Fractint successor, libid/fractals/lorenz.cpp and
//! popcorn.cpp) written out in f64.
//!
//! One step at a time, from points on the attractor (the reference's own
//! orbit) and across a grid: a chaotic orbit doubles any difference
//! every few steps, so whole orbits would compare the rounding of the
//! two languages, not the maps.

use super::lang::builtins::C64;
use super::lang::interp::{ParamValues, VariationRun};
use super::lang::rhai;
use crate::storage::formula_store::EXAMPLES;

fn example(file: &str) -> &'static str {
    EXAMPLES.iter().find(|(f, _)| *f == file).map(|(_, t)| *t).unwrap_or_else(|| panic!("no example {file}"))
}

/// Iterated Dynamics' `sign` (libid/include/math/sign.h): 0 at 0.
fn sign(x: f64) -> f64 {
    ((0.0 < x) as i32 - (x < 0.0) as i32) as f64
}

/// The points to step from: `n` of the reference's orbit from `start`
/// (past a transient), and a grid over `extent` about the origin.
fn starts(map: &dyn Fn(f64, f64) -> (f64, f64), start: (f64, f64), n: usize, extent: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let (mut x, mut y) = start;
    for i in 0..n + 20 {
        (x, y) = map(x, y);
        if i >= 20 && x.is_finite() && y.is_finite() {
            out.push((x, y));
        }
    }
    for i in 0..15 {
        for j in 0..15 {
            out.push((extent * (i as f64 / 7.0 - 1.0) + 0.013, extent * (j as f64 / 7.0 - 1.0) - 0.007));
        }
    }
    out
}

/// The file's variation, at its defaults, against `map` at each start.
fn steps_as(file: &str, map: &dyn Fn(f64, f64) -> (f64, f64), start: (f64, f64), extent: f64) {
    let src = example(file);
    let p = rhai::transformation(src).unwrap_or_else(|d| panic!("{file}: {:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>()));
    let params = ParamValues::defaults(&p);
    let mut run = VariationRun::new(&p, &params, 1);
    let mut checked = 0;
    for (x, y) in starts(map, start, 300, extent) {
        let (want_x, want_y) = map(x, y);
        if !(want_x.is_finite() && want_y.is_finite()) {
            continue;
        }
        let (got, _) = run.call(C64::new(x, y), C64::ZERO, 1.0);
        let tol = 1e-12 * (1.0 + want_x.abs().max(want_y.abs()));
        assert!(
            (got.re - want_x).abs() <= tol && (got.im - want_y).abs() <= tol,
            "{file} at ({x}, {y}): ({}, {}) against ({want_x}, {want_y})",
            got.re,
            got.im
        );
        checked += 1;
    }
    assert!(checked > 300, "{file}: only {checked} points checked");
}

/// The file's 3D variation, at its defaults, against `map`, from the
/// reference's orbit and a grid.
fn steps3_as(file: &str, map: &dyn Fn([f64; 3]) -> [f64; 3], start: [f64; 3], extent: f64) {
    let src = example(file);
    let p = rhai::transformation3(src).unwrap_or_else(|d| panic!("{file}: {:?}", d.iter().map(|x| x.render(src)).collect::<Vec<_>>()));
    let params = ParamValues::defaults(&p);
    let mut run = VariationRun::new(&p, &params, 1);
    let mut points = Vec::new();
    let mut v = start;
    for i in 0..400 {
        v = map(v);
        if i >= 50 {
            points.push(v);
        }
    }
    for i in 0..8 {
        for j in 0..8 {
            for k in 0..8 {
                let at = |n: i32| extent * (n as f64 / 3.5 - 1.0) + 0.011;
                points.push([at(i), at(j), at(k)]);
            }
        }
    }
    for v in points {
        let want = map(v);
        let (got, _) = run.call3(v, [0.0; 3], 1.0);
        let tol = 1e-12 * (1.0 + want.iter().fold(0.0f64, |m, x| m.max(x.abs())));
        assert!((0..3).all(|k| (got[k] - want[k]).abs() <= tol), "{file} at {v:?}: {got:?} against {want:?}");
    }
}

/// hopalong2d_orbit, at Iterated Dynamics' defaults a = 0.4, b = 1, c = 0.
#[test]
fn hopalong_steps_as_iterated_dynamics() {
    let (a, b, c) = (0.4, 1.0, 0.0);
    steps_as("variation-hopalong.rhai", &|x, y| (y - sign(x) * (b * x - c).abs().sqrt(), a - x), (0.1, 0.0), 3.0);
}

/// martin2d_orbit, at a = 3.14.
#[test]
fn martin_steps_as_iterated_dynamics() {
    let a = 3.14;
    steps_as("variation-martin.rhai", &|x, y| (y - x.sin(), a - x), (0.1, 0.0), 30.0);
}

/// chip2d_orbit, at a = -15, b = -19, c = 1.
#[test]
fn chip_steps_as_iterated_dynamics() {
    let (a, b, c) = (-15.0f64, -19.0f64, 1.0f64);
    let map = move |x: f64, y: f64| {
        let tmp = y - sign(x) * ((b * x - c).abs().ln().powi(2)).cos() * ((c * x - b).abs().ln().powi(2)).atan();
        (tmp, a - x)
    };
    steps_as("variation-chip.rhai", &map, (0.1, 0.0), 700.0);
}

/// quadrup_two2d_orbit, at a = 34, b = 1, c = 5.
#[test]
fn quadruptwo_steps_as_iterated_dynamics() {
    let (a, b, c) = (34.0f64, 1.0f64, 5.0f64);
    let map = move |x: f64, y: f64| {
        let tmp = y - sign(x) * (b * x - c).abs().ln().sin() * ((c * x - b).abs().ln().powi(2)).atan();
        (tmp, a - x)
    };
    steps_as("variation-quadruptwo.rhai", &map, (0.1, 0.0), 90.0);
}

/// three_ply2d_orbit, at a = -55, b = -1, c = -42.
#[test]
fn threeply_steps_as_iterated_dynamics() {
    let (a, b, c) = (-55.0f64, -1.0f64, -42.0f64);
    let (cos_b, sin_sum) = (b.cos(), (a + b + c).sin());
    let map = move |x: f64, y: f64| (y - sign(x) * (x.sin() * cos_b + c - x * sin_sum).abs(), a - x);
    steps_as("variation-threeply.rhai", &map, (0.1, 0.0), 7000.0);
}

/// ginger_bread_orbit, from its default start (-0.1, 0).
#[test]
fn the_gingerbread_man_steps_as_iterated_dynamics() {
    steps_as("variation-gingerbreadman.rhai", &|x, y| (1.0 - y + x.abs(), x), (-0.1, 0.0), 6.0);
}

/// latoo_orbit with the sines its comment writes, at a = -0.966918,
/// b = 2.879879, c = 0.765145, d = 0.744728.
#[test]
fn latoocarfian_steps_as_iterated_dynamics() {
    let (a, b, c, d) = (-0.966918f64, 2.879879f64, 0.765145f64, 0.744728f64);
    let map = move |x: f64, y: f64| ((y * b).sin() + c * (x * b).sin(), (x * a).sin() + d * (y * a).sin());
    steps_as("variation-latoocarfian.rhai", &map, (1.0, 1.0), 2.0);
}

/// kam_torus_orbit's map, at angle 1.3 (r is x, s is y).
#[test]
fn the_kam_torus_steps_as_iterated_dynamics() {
    let (sin_x, cos_x) = (1.3f64.sin(), 1.3f64.cos());
    let map = move |r: f64, s: f64| {
        let srr = s - r * r;
        (r * cos_x - srr * sin_x, r * sin_x + srr * cos_x)
    };
    steps_as("variation-kamtorus.rhai", &map, (0.2, 0.2), 0.8);
}

/// Popcorn's generalized step with sin, tan, sin, tan and a real step and
/// constant (h = 0.05, C = 3): its complex parts are then zero, and
/// new.x = x - h sin(y + tan(C y)), new.y = y - h sin(x + tan(C x)).
#[test]
fn pickovers_popcorn_steps_as_iterated_dynamics() {
    let (h, c) = (0.05f64, 3.0f64);
    let map = move |x: f64, y: f64| (x - h * (y + (c * y).tan()).sin(), y - h * (x + (c * x).tan()).sin());
    steps_as("variation-pickover-popcorn.rhai", &map, (0.3, -0.2), 2.5);
}

/// dynamic_orbit with sin, dt = 0.1, a = 1, b = 3, both parts from the
/// old point (a positive time step).
#[test]
fn the_dynamic_system_steps_as_iterated_dynamics() {
    let (dt, a, b) = (0.1f64, 1.0f64, 3.0f64);
    let map = move |x: f64, y: f64| {
        let new_y = y + dt * (x + a * (b * x).sin()).sin();
        let new_x = x - dt * (y + a * (b * y).sin()).sin();
        (new_x, new_y)
    };
    steps_as("variation-dynamic-system.rhai", &map, (1.0, 2.0), 18.0);
}

/// pickover_orbit, at a = 2.24, b = 0.43, c = -0.65, d = -2.43.
#[test]
fn pickovers_attractor_steps_as_iterated_dynamics() {
    let (a, b, c, d) = (2.24f64, 0.43f64, -0.65f64, -2.43f64);
    let map = move |[x, y, z]: [f64; 3]| [(a * y).sin() - z * (b * x).cos(), z * (c * x).sin() - (d * y).cos(), x.sin()];
    steps3_as("variation-pickover-attractor.rhai", &map, [1.0, 1.0, 1.0], 2.0);
}

/// rossler_orbit, at dt = 0.04, a = 0.2, b = 0.2, c = 5.7: an explicit
/// step from the old point.
#[test]
fn the_rossler_attractor_steps_as_iterated_dynamics() {
    let (dt, a, b, c) = (0.04f64, 0.2f64, 0.2f64, 5.7f64);
    let map = move |[x, y, z]: [f64; 3]| {
        let (xdt, ydt) = (x * dt, y * dt);
        let dx = -ydt - z * dt;
        let dy = xdt + y * (a * dt);
        let dz = b * dt + z * xdt - z * (c * dt);
        [x + dx, y + dy, z + dz]
    };
    steps3_as("variation-rossler-attractor.rhai", &map, [1.0, 1.0, 1.0], 12.0);
}

/// The dynamic system's Euler order: y first, x from the new y.
#[test]
fn the_dynamic_systems_euler_order_steps_y_first() {
    let src = example("variation-dynamic-system.rhai");
    let p = rhai::transformation(src).unwrap();
    let mut params = ParamValues::defaults(&p);
    let i = p.params.iter().position(|x| x.name == "update").expect("the Update choice");
    params.values[i] = (1.0, 0.0);
    let mut run = VariationRun::new(&p, &params, 1);
    let (dt, a, b) = (0.1f64, 1.0f64, 3.0f64);
    for (x, y) in [(1.0f64, 2.0f64), (-3.5, 0.25), (7.0, -11.0)] {
        let new_y = y + dt * (x + a * (b * x).sin()).sin();
        let new_x = x - dt * (new_y + a * (b * new_y).sin()).sin();
        let (got, _) = run.call(C64::new(x, y), C64::ZERO, 1.0);
        assert!((got.re - new_x).abs() < 1e-12 && (got.im - new_y).abs() < 1e-12, "({x}, {y}): {got:?}");
    }
}
