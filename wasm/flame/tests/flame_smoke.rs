//! Native smoke test for the flame-only module: a flame renders, the
//! full catalog is present, and an escape config is REFUSED rather
//! than silently rendered as something else.
//!
//! That refusal is the contract. An escape config still parses here —
//! the render mode round-trips, so a file passing through this module
//! is never rewritten — and the render reports a missing engine
//! instead of drawing the flame the file happens to also carry.

#[pollster::test]
async fn a_fixture_config_renders_to_nonblank_pixels() {
    let config = include_str!("../../script/tests/fixtures/basic_random_seed7.fflame");

    let tile = match fflame_flame::render_impl(config, 64, 64, Some(500_000), None).await {
        Ok(t) => t,
        Err(e) if e.contains("no GPU adapter") => {
            eprintln!("skipped: {e}");
            return;
        }
        Err(e) => panic!("render failed: {e}"),
    };

    assert_eq!(tile.pixels.len(), 64 * 64 * 4);
    assert!(
        tile.pixels.chunks(4).any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0),
        "all pixels black — the render produced nothing"
    );
}

#[test]
fn the_whole_flame_catalog_is_present() {
    let registry = fractal_flame_wgpu::variations::global_registry();
    let n = registry.names().len();
    assert!(
        n > 500,
        "the flame module carries only {n} variations — engine-flame is off, \
         and configs using anything but `linear` will render wrong"
    );
}

/// An escape config must fail loudly here.
#[pollster::test]
async fn an_escape_config_is_refused_rather_than_mis_rendered() {
    let config = include_str!("../../../tests/visual/configs/escape/mandelbrot-smooth.fflame");

    match fflame_flame::render_impl(config, 32, 32, Some(100_000), None).await {
        Err(e) if e.contains("no GPU adapter") => eprintln!("skipped: {e}"),
        Err(e) => assert!(
            e.contains("escape"),
            "an escape config failed for the wrong reason: {e}"
        ),
        Ok(_) => panic!(
            "an escape config rendered in the flame-only module — it has no escape \
             engine, so whatever it drew is not what the file describes"
        ),
    }
}
