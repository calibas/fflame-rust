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

/// Oversized dimensions must come back as an error, not a panic.
///
/// Unchecked, this reached wgpu's validation and PANICKED
/// (`Dimension X value 8193 exceeds the limit of 8192`). In wasm a
/// panic poisons the module: every later call fails and the page has to
/// reload — so one hostile tile request killed the whole session.
#[pollster::test]
async fn oversized_dimensions_return_an_error_rather_than_panicking() {
    let config = include_str!("../../script/tests/fixtures/basic_random_seed7.fflame");
    let err = match fflame_render::render_impl(config, 100_000, 100_000, Some(1000)).await {
        Err(e) => e,
        Ok(_) => panic!("a 100000x100000 render was accepted"),
    };
    if err.contains("no GPU adapter") {
        eprintln!("skipped: no adapter");
        return;
    }
    assert!(
        err.contains("exceeds this device's maximum texture dimension"),
        "wrong error: {err}"
    );

    // And the module still works afterwards — the point of not panicking.
    match fflame_render::render_impl(config, 32, 32, Some(100_000)).await {
        Ok(t) => assert_eq!(t.pixels.len(), 32 * 32 * 4),
        Err(e) => panic!("module unusable after a rejected render: {e}"),
    }
}

/// A config's own `max_iterations` is attacker-supplied too: one asking
/// for 5e11 ground for 90s and was still going when killed. Both the
/// config's value and the caller's argument are clamped.
#[pollster::test]
async fn a_hostile_iteration_count_is_clamped() {
    let config = include_str!("../../script/tests/fixtures/basic_random_seed7.fflame");
    let bomb = config.replacen('{', r#"{"max_iterations": 500000000000,"#, 1);

    let t0 = std::time::Instant::now();
    let tile = match fflame_render::render_impl(&bomb, 64, 64, None).await {
        Ok(t) => t,
        Err(e) if e.contains("no GPU adapter") => {
            eprintln!("skipped: {e}");
            return;
        }
        Err(e) => panic!("render failed: {e}"),
    };
    // The clamp bounds the work; it does not land on the number exactly,
    // because the chaos game runs in whole dispatch batches and cannot
    // stop mid-batch. One batch of overshoot is expected — what matters
    // is that 5e11 became ~8e9 rather than running to completion.
    assert!(
        tile.iterations < 9_000_000_000,
        "iterations not clamped: {}",
        tile.iterations
    );
    // Sanity: the clamp has to actually bound the wall clock.
    assert!(
        t0.elapsed().as_secs() < 300,
        "clamped render still took {:?}",
        t0.elapsed()
    );
}

/// Many tiles in a row on one thread — the shape a gallery scroll has,
/// and the shape that broke.
///
/// The module used to create a device per tile and destroy it. On
/// WebGPU that is the ONLY synchronous reclamation available, because
/// wgpu implements `Drop` for buffers and textures as a no-op there, so
/// a discarded renderer's memory survives until the JS garbage
/// collector runs. A scroll created devices faster than the browser
/// reclaimed them and eventually `requestDevice` itself failed with
/// "Not enough memory left" — reported from the real gallery.
///
/// One device and one renderer are now reused across calls, so this
/// asserts the reuse path works over a run long enough to have failed
/// before: repeated renders, a config change, and a size change (which
/// takes the rebuild branch rather than plain `load_config`).
///
/// It cannot prove memory is flat from inside the process — that needs a
/// browser and about:gpu — but it does prove the reused renderer keeps
/// producing correct, non-blank output rather than blank tiles or an
/// error, which is how the failure presented.
#[pollster::test]
async fn many_renders_in_a_row_reuse_the_device() {
    let config = include_str!("../../script/tests/fixtures/basic_random_seed7.fflame");

    // First one doubles as the adapter probe.
    match fflame_render::render_impl(config, 48, 48, Some(200_000)).await {
        Ok(_) => {}
        Err(e) if e.contains("no GPU adapter") => {
            eprintln!("skipped: {e}");
            return;
        }
        Err(e) => panic!("first render failed: {e}"),
    }

    // Same size, repeatedly: the pure `load_config` reuse path.
    for i in 0..12 {
        let tile = fflame_render::render_impl(config, 48, 48, Some(200_000))
            .await
            .unwrap_or_else(|e| panic!("render {i} failed: {e}"));
        assert_eq!(tile.pixels.len(), 48 * 48 * 4, "render {i}");
        assert!(
            tile.pixels.chunks(4).any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0),
            "render {i} came out entirely black — the symptom of an exhausted device"
        );
    }

    // Size changes take the rebuild branch, which destroys the previous
    // renderer explicitly. Alternating sizes exercises it repeatedly.
    for (i, (w, h)) in [(64u32, 64u32), (48, 48), (32, 96), (48, 48)].iter().enumerate() {
        let tile = fflame_render::render_impl(config, *w, *h, Some(200_000))
            .await
            .unwrap_or_else(|e| panic!("resize render {i} ({w}x{h}) failed: {e}"));
        assert_eq!(tile.width, *w);
        assert_eq!(tile.height, *h);
        assert_eq!(tile.pixels.len(), (*w as usize) * (*h as usize) * 4);
    }

    // `release()` must leave the module usable, not poisoned — the next
    // call transparently builds a new device.
    fflame_render::release_impl();
    let after = fflame_render::render_impl(config, 48, 48, Some(200_000))
        .await
        .expect("render after release() should rebuild the device");
    assert_eq!(after.pixels.len(), 48 * 48 * 4);
}
