//! Native smoke test for the simulation-only module: a simulation
//! config renders, and neither of the other two engines is linked in.
//!
//! The second half is the point of this crate existing. Its saving
//! comes entirely from `engine-flame` and `engine-escape` being off,
//! which is invisible in a render of a simulation config — so it is
//! asserted directly against the registry rather than inferred from a
//! file size.

#[pollster::test]
async fn a_simulation_config_renders_to_nonblank_pixels() {
    let config = include_str!("../../../tests/visual/configs/sim/brusselator-turing.fflame");

    let tile = match fflame_sim::render_impl(config, 64, 64, None, None).await {
        Ok(t) => t,
        Err(e) if e.contains("no GPU adapter") => {
            eprintln!("skipped: {e}");
            return;
        }
        Err(e) => panic!("render failed: {e}"),
    };

    assert_eq!(tile.width, 64);
    assert_eq!(tile.height, 64);
    assert_eq!(tile.pixels.len(), 64 * 64 * 4);
    assert!(
        tile.pixels.chunks(4).any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0),
        "all pixels black — the simulation render produced nothing"
    );
}

/// A simulation is a RUN, not a formula: the same config at two step
/// counts is two different pictures. This is the property that makes
/// the module worth having at all, and the one a broken build would
/// most plausibly lose (a grid that never steps still renders — as its
/// seed).
#[pollster::test]
async fn the_grid_actually_steps() {
    let base: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/visual/configs/sim/brusselator-turing.fflame"
    ))
    .expect("the shipped config parses");

    let at = |steps: u64| {
        let mut c = base.clone();
        c["sim"]["steps"] = serde_json::json!(steps);
        c.to_string()
    };

    let early = match fflame_sim::render_impl(&at(20), 64, 64, None, None).await {
        Ok(t) => t,
        Err(e) if e.contains("no GPU adapter") => {
            eprintln!("skipped: {e}");
            return;
        }
        Err(e) => panic!("render failed: {e}"),
    };
    let late = fflame_sim::render_impl(&at(2000), 64, 64, None, None)
        .await
        .expect("the second render");

    assert_ne!(
        early.pixels, late.pixels,
        "20 steps and 2000 steps gave the same image — the grid is not stepping"
    );
}

/// The flame catalog is absent, which is most of this module's saving.
///
/// One variation remains on purpose: a default `FractalConfig` carries
/// a flame whose transforms name `linear`, and every lookup path
/// expects that name to resolve. What must NOT be here is the other
/// 646 defs and their 1.1 MB of inline WGSL.
#[test]
fn the_flame_catalog_is_not_linked_in() {
    let registry = fractal_flame_wgpu::variations::global_registry();
    let n = registry.names().len();
    assert!(
        n <= 2,
        "the simulation-only module carries {n} variations — engine-flame is on, \
         and the module is paying for a catalog it cannot use"
    );
    assert!(
        registry.get("linear").is_some(),
        "`linear` must still resolve: a default config's transforms name it"
    );
}

/// And the escape engine, whose deep-zoom machinery is the weight
/// rather than its shader source.
///
/// Asserted through the public render path rather than a registry
/// count, because there is no escape registry to consult when the
/// feature is off — the module reports the missing engine, which is
/// the behaviour a caller sees and the thing that must not silently
/// become "renders a flame instead".
#[pollster::test]
async fn an_escape_config_is_refused_rather_than_mis_rendered() {
    let config = include_str!("../../../tests/visual/configs/escape/mandelbrot-smooth.fflame");

    match fflame_sim::render_impl(config, 32, 32, Some(50_000), None).await {
        Err(e) if e.contains("no GPU adapter") => eprintln!("skipped: {e}"),
        Err(e) => assert!(
            e.contains("escape"),
            "an escape config should report the missing escape engine, got: {e}"
        ),
        Ok(_) => panic!(
            "an escape config rendered in the simulation-only module — \
             it can only have drawn something the file never described"
        ),
    }
}
