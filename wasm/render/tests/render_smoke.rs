//! Native smoke test: the module's exact code path (parse → device →
//! unified render → destroy) against the script module's committed
//! fixture, on whatever GPU the machine has. Skips cleanly when no
//! adapter exists (CI without a GPU) — presence of pixels is asserted
//! whenever one does.

#[pollster::test]
async fn a_fixture_config_renders_to_nonblank_pixels() {
    let config = include_str!("../../script/tests/fixtures/basic_random_seed7.fflame");

    let tile = match fflame_render::render_impl(config, 64, 64, Some(500_000)).await {
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
    assert!(tile.iterations > 0);
    assert!(
        tile.pixels.chunks(4).any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0),
        "all pixels black — the render produced nothing"
    );
}
