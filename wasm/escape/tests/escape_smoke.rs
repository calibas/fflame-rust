//! Native smoke test for the escape-only module: an escape config
//! renders, and the flame catalog really is gone.
//!
//! The second half is the point of this crate existing. Its saving
//! comes entirely from `engine-flame` being off, which is invisible in
//! a render of an escape config — so it is asserted directly against
//! the registry rather than inferred from a file size.

#[pollster::test]
async fn an_escape_config_renders_to_nonblank_pixels() {
    let config = include_str!("../../../tests/visual/configs/escape/mandelbrot-smooth.fflame");

    let tile = match fflame_escape::render_impl(config, 64, 64, Some(500_000), None).await {
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
        "all pixels black — the escape render produced nothing"
    );
}

/// The flame catalog is absent, which is where this module's 49% comes
/// from.
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
        "the escape-only module carries {n} variations — engine-flame is on, \
         and the module is paying for a catalog it cannot use"
    );
    assert!(
        registry.get("linear").is_some(),
        "`linear` must still resolve: a default config's transforms name it"
    );
}
