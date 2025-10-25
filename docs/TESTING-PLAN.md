# Visual Regression Testing Plan

Comprehensive strategy for visual quality verification across platforms.

---

## Overview

**Goal**: Ensure fractal flame renders are visually correct across:
- Desktop (Win10/macOS/Linux)
- WASM (browser)
- PNG exports
- Compared to Apophysis 7X (ground truth)

**Key Insight**: PNG export uses the same GPU rendering path as the app display, so testing exports gives high confidence in visual correctness without complex screenshot automation.

---

## Test Suite Architecture

```
tests/visual/
├── references/          # Known-good baseline images
│   ├── desktop/        # Win10 app PNG exports (baseline)
│   ├── apophysis/      # Apophysis 7X renders (ground truth)
│   ├── wasm/           # WASM canvas captures (future)
│   └── checksums.json  # Image hashes for quick comparison
│
├── current/            # Generated during test runs
│   ├── desktop/
│   ├── wasm/
│   └── checksums.json
│
├── diffs/              # Visual diff outputs (when tests fail)
│   ├── desktop/
│   ├── wasm/
│   └── report.html     # Visual diff review page
│
├── configs/            # Test case .flame files
│   ├── variations/     # One test per variation (26 files)
│   ├── render_modes/   # 2D/3D, ortho/perspective
│   ├── color_modes/    # Transform/palette/speed
│   ├── edge_cases/     # Min/max values, edge conditions
│   ├── presets/        # Built-in presets
│   └── manifest.json   # Test suite metadata
│
└── tools/              # Testing utilities
    ├── generate_references.rs   # Create baseline images
    ├── run_visual_tests.rs      # Compare current vs baseline
    ├── compare_images.rs        # Image diff with metrics
    └── export_apophysis.rs      # Convert .flame → Apophysis format
```

---

## Test Categories

### 1. Per-Variation Tests (26 tests)
One test case showcasing each variation in isolation:
- `linear.flame` - 100% linear variation
- `sinusoidal.flame` - 100% sinusoidal
- `spherical.flame` - 100% spherical
- ... (one for each of 26 variations)

**Purpose**: Catch algorithm bugs in individual variations

### 2. Render Mode Tests (6 tests)
- `2d_orthographic.flame` - Classic 2D rendering
- `3d_orthographic.flame` - 3D without perspective
- `3d_perspective_weak.flame` - Perspective strength 1.0
- `3d_perspective_strong.flame` - Perspective strength 5.0
- `3d_camera_pitch45.flame` - Camera rotation test
- `3d_camera_yaw90.flame` - Camera rotation test

**Purpose**: Verify 3D rendering, camera, projection

### 3. Color Mode Tests (5 tests)
- `color_transform.flame` - Transform color mode
- `color_palette.flame` - Palette lookup mode
- `color_speed.flame` - Speed-based coloring
- `color_gradient.flame` - Smooth gradient test
- `color_background.flame` - Background blending test

**Purpose**: Verify color interpolation and blending

### 4. Variation Parameter Tests (2 tests)
- `julian_params.flame` - JuliaN with various power/dist values
- `blob_params.flame` - Blob with various high/low/waves values

**Purpose**: Verify parameterized variations work correctly

### 5. Edge Case Tests (8 tests)
- `single_transform.flame` - Minimum (1 transform)
- `max_transforms.flame` - Maximum (32 transforms)
- `tiny_zoom.flame` - Zoom 0.001 (extreme zoom in)
- `huge_zoom.flame` - Zoom 1000.0 (extreme zoom out)
- `zero_weight.flame` - Edge case: all weights near zero
- `high_iteration.flame` - Quality test (100M iterations)
- `mixed_2d_3d.flame` - 2D + 3D variations combined
- `all_variations.flame` - All 26 variations active at once

**Purpose**: Catch crashes, artifacts, edge condition bugs

### 6. Preset Tests (Current Built-ins)
- `simple.flame`
- `complex.flame`
- `julia.flame`
- `spherical.flame`
- `flower_of_life.flame`
- ... (all built-in presets)

**Purpose**: Regression testing for known-good configs

### 7. Tone Curve Tests (4 tests)
- `tonecurve_off.flame` - No curve
- `tonecurve_linear.flame` - Linear curve
- `tonecurve_s.flame` - S-curve
- `tonecurve_custom.flame` - Custom curve shape

**Purpose**: Verify tone mapping doesn't affect background

**Total Test Cases: ~60 configurations**

---

## PNG Metadata Specification

All exported PNGs will embed comprehensive metadata for traceability:

### Metadata Fields

```rust
struct PngMetadata {
    // Build Info
    version: String,           // "0.1.0"
    build_number: u32,         // 123
    git_hash: String,          // "dba27e8"
    git_branch: String,        // "main"
    build_timestamp: String,   // "2025-10-24T12:34:56Z"
    platform: String,          // "windows-x86_64"

    // Render Settings
    width: u32,                // 1920
    height: u32,               // 1080
    total_iterations: u64,     // 10000000
    quality: String,           // "high"
    render_time_ms: f64,       // 1234.56

    // Flame Config (JSON embedded)
    config_json: String,       // Full FractalConfig serialized
    config_checksum: String,   // SHA256 of config

    // Test Metadata (if from test suite)
    test_name: Option<String>, // "linear_variation"
    test_category: Option<String>, // "variations"
    reference_image: Option<String>, // Path to reference

    // Color/Display Settings
    background_color: [f32; 3],
    exposure: f32,
    gamma: f32,
    use_tone_curve: bool,

    // Rendering Mode
    render_mode: String,       // "2D" or "3D"
    projection: String,        // "Orthographic" or "Perspective{2.5}"
}
```

### PNG Text Chunks (tEXt/iTXt)

PNG format supports text chunks for metadata:

```
Software: Fractal Flame Renderer v0.1.0
Build: #123 (dba27e8)
Platform: windows-x86_64
BuildDate: 2025-10-24T12:34:56Z
Resolution: 1920x1080
Iterations: 10000000
RenderTime: 1234.56ms
Config: <compressed JSON>
ConfigChecksum: sha256:abc123...
TestName: linear_variation
RenderMode: 2D
```

**Benefits:**
- Reproducibility: Can recreate exact render from PNG
- Debugging: Know exactly what build/config created the image
- Comparison: Verify test image came from correct config
- Archival: PNGs are self-documenting

---

## Implementation Phases

### Phase 1: PNG Metadata (✅ COMPLETE)
**Completed Tasks:**
1. ✅ PNG metadata writing to all exports (GUI and CLI)
2. ✅ Implemented `PngMetadata` struct in `src/png_metadata.rs`
3. ✅ Serialize config + build info to PNG tEXt chunks
4. ✅ Embedded checksums (SHA256 of config JSON)
5. ✅ Metadata reading utility for verification

**Modified Files:**
- ✅ `src/renderer/compute_kernel.rs` - PNG export with metadata
- ✅ `src/version.rs` - Build info capture
- ✅ `src/config.rs` - Complete config serialization
- ✅ `src/png_metadata.rs` - Metadata handling (new file)

**Metadata Fields Embedded:**
- Version, build number, git hash, git branch, build timestamp
- Platform, resolution, total iterations, render time
- Full FractalConfig JSON (complete reproducibility)
- SHA256 checksum of config
- Test category (if provided)

**Deliverable:** ✅ All PNG exports include full metadata for reproducibility

### Phase 2: Headless PNG Export (✅ COMPLETE)
**Completed Tasks:**
1. ✅ Added CLI export mode to main app (clap subcommands)
2. ✅ Implemented headless GPU rendering in `src/app.rs::export_headless()`
3. ✅ Load .flame configs without GUI
4. ✅ Render to texture using same code as interactive app
5. ✅ Export PNG with full metadata
6. ✅ Batch process all .flame files in directory

**Commands:**
```bash
# Generate images from single config
cargo run --release -- export --input config.flame --output output.png

# Batch export all configs in directory
cargo run --release -- export --input tests/visual/configs --output tests/visual/current

# Custom resolution (overrides config)
cargo run --release -- export --input config.flame --output out.png --width 1920 --height 1080

# Include test category in metadata
cargo run --release -- export --input config.flame --output out.png --category variations
```

**Deliverable:** ✅ Automated PNG generation using same rendering code as app (~0.5s for 10M iterations @ 800x600)

### Phase 3: Image Comparison (✅ COMPLETE - Basic)
**Completed Tasks:**
1. ✅ Implemented `src/bin/compare_images.rs` tool
2. ✅ Basic metrics: Mean Diff, Max Diff, Diff %
3. ✅ Advanced metrics (--advanced flag): SSIM, MSE, PSNR
4. ✅ Visual diff image generation (saves to `{name}_diff.png`)
5. ⏳ HTML report (not yet implemented)

**Commands:**
```bash
# Basic comparison
cargo run --release --bin compare_images -- --image1 ref.png --image2 current.png

# Advanced metrics (SSIM, MSE, PSNR)
cargo run --release --bin compare_images -- --image1 ref.png --image2 current.png --advanced
```

**Current Metrics:**
- **Basic**: Mean Diff, Max Diff, Diff % (pixels changed)
- **Advanced**: SSIM (structural similarity), MSE (mean squared error), PSNR (peak signal-to-noise ratio)
- **Visual**: Diff image highlighting changed regions

**Recommended Thresholds:**
- **SSIM** > 0.99 = PASS (allows minor float/AA differences)
- **Diff %** < 1.0% = PASS (% of pixels different)
- **Max Diff** < 10 (0-255 scale) = PASS

**Deliverable:** ✅ Manual visual regression detection (automated test harness still pending)

### Phase 4: Test Config Library (🔄 IN PROGRESS)
**Current Status:**
1. ✅ Directory structure created: `tests/visual/configs/variations/`
2. ✅ Initial test configs: `linear.flame`, `sinusoidal.flame`, `spherical.flame`
3. ⏳ Remaining 23 variation tests
4. ⏳ Organize by category (variations/, render_modes/, color_modes/, edge_cases/)
5. ⏳ Create `manifest.json` with test metadata
6. ⏳ Document expected results

**Completed Configs:**
- `tests/visual/configs/variations/linear.flame` (10M iterations)
- `tests/visual/configs/variations/sinusoidal.flame` (10M iterations)
- `tests/visual/configs/variations/spherical.flame` (10M iterations)

**Next Steps:**
- Create remaining 23 variation test configs
- Add render_mode tests (2D/3D, ortho/perspective)
- Add color_mode tests
- Add edge case tests

**Deliverable:** ⏳ Comprehensive test suite (~60 configs)

### Phase 5: Apophysis Ground Truth (Future)
**Tasks:**
1. Export test configs to Apophysis 7X format
2. Manually render in Apophysis at 1920×1080
3. Save as `tests/visual/references/apophysis/*.png`
4. Use as ground truth for correctness

**Purpose:** Verify algorithm correctness vs reference implementation

**Deliverable:** Apophysis baseline for all test cases

### Phase 6: WASM Testing (Future)
**Tasks:**
1. Add canvas capture to WASM build
2. Create Playwright/Puppeteer test harness
3. Automated headless browser testing
4. Screenshot comparison

**Commands:**
```bash
wasm-pack test --headless --chrome
```

**Deliverable:** Automated WASM visual testing

### Phase 7: CI/CD Integration (Future)
**Tasks:**
1. GitHub Actions workflow
2. Run visual tests on every PR
3. Store references in Git LFS
4. Fail PR if visual diffs exceed threshold
5. Upload diff images as artifacts

**Deliverable:** Continuous visual regression detection

---

## Comparison Workflow

### Current Manual Workflow (Phase 3 Complete, Phase 7 Pending)
```bash
# 1. Generate reference images (one-time or when intentionally changed)
cargo run --release -- export --input tests/visual/configs --output tests/visual/references

# 2. Make code changes
# ... edit src/renderer/compute_kernel.rs ...

# 3. Generate current images
cargo run --release -- export --input tests/visual/configs --output tests/visual/current

# 4. Compare images manually
cargo run --release --bin compare_images -- --image1 tests/visual/references/linear.png --image2 tests/visual/current/linear.png --advanced

# 5. Review diff images
# Check tests/visual/current/linear_diff.png
```

### Future Automated Workflow (Phase 7: CI/CD Integration)
```bash
# Automated test command (future)
cargo test --test visual_regression

# Review failures (if any)
open tests/visual/diffs/report.html
```

### Manual Review Process
When tests fail:

1. **View Diff Report**: Open `tests/visual/diffs/report.html`
2. **Side-by-Side Comparison**: See reference vs current vs diff
3. **Metrics Review**: Check SSIM, MSE, diff % for each failure
4. **Decision**:
   - **Bug**: Fix code, re-run tests
   - **Intentional**: Update references: `cargo run --bin visual_test -- update-references`

### Baseline Update
```bash
# After verifying changes are correct, regenerate references
cargo run --release -- export --input tests/visual/configs/variations/linear.flame --output tests/visual/references/linear.png

# Or regenerate all references
cargo run --release -- export --input tests/visual/configs --output tests/visual/references
```

---

## Integration with Existing Tests

### Current Testing (from TESTING-GUIDE.md)
- **Unit tests**: Transform math, variations, palette
- **Regression tests**: CPU determinism, all variations work
- **Benchmarks**: CPU iteration, individual variations
- **Manual testing**: Run app, visual inspection

### New Visual Tests (This Plan)
- **PNG export tests**: Headless rendering + image comparison
- **Apophysis comparison**: Ground truth verification
- **WASM screenshots**: Browser rendering validation
- **Automated regression**: CI/CD integration

### Combined Workflow
```bash
# Before commit
cargo test                                # Unit tests
cargo test --test regression              # Integration tests
cargo run --release -- export -i tests/visual/configs -o tests/visual/current  # Generate test images
# (Manual comparison for now, automated test harness pending)
cargo bench                               # Benchmarks

# All must pass before merge
```

---

## Test Execution Strategy

### Fast Tests (Every Commit)
- Unit tests
- Regression tests
- Visual tests with **reduced iteration count** (1M iterations, ~1 sec/test)
- **Total time**: ~5 minutes for 60 test cases

### Slow Tests (Pre-Release / Nightly)
- Visual tests with **full iteration count** (10M+ iterations, ~10 sec/test)
- Apophysis comparison (manual)
- WASM screenshot tests
- **Total time**: ~15 minutes

### Manual Tests (Release Candidate)
- Run app on all platforms
- Visual inspection of all presets
- User interaction testing
- Performance validation

---

## Success Criteria

### Phase 1 (PNG Metadata) - ✅ COMPLETE
- [x] All PNG exports include version, build, config metadata
- [x] Metadata can be read and verified
- [x] Config from PNG can recreate exact render

### Phase 2 (Headless Export) - ✅ COMPLETE
- [x] Can batch-generate PNGs from .flame files without GUI
- [x] Generates test images quickly (~0.5s for 10M iterations @ 800x600)
- [x] All images include complete metadata

### Phase 3 (Image Comparison) - ✅ BASIC COMPLETE (HTML report pending)
- [x] Automated SSIM/MSE comparison working
- [x] Visual diff images generated
- [ ] HTML report shows side-by-side comparison (future)
- [x] Pass/fail thresholds defined (manual verification for now)

### Phase 4 (Test Library) - 🔄 IN PROGRESS
- [x] Initial test .flame files created (3/60)
- [x] Directory structure created
- [ ] All 26 variations covered (3/26)
- [ ] All render modes covered
- [ ] Edge cases documented

### Full System - Complete When:
- [ ] Visual tests run in <5 minutes (fast mode)
- [ ] CI/CD integration working
- [ ] Apophysis ground truth available
- [ ] WASM testing automated
- [ ] Zero visual regressions in production

---

## Open Questions & Future Work

### Questions to Resolve
1. **Iteration count for tests?**
   - Fast mode: 1M iterations (~1 sec/test)
   - Full mode: 10M iterations (~10 sec/test)
   - Reference mode: 100M iterations (~100 sec/test, for Apophysis comparison)

2. **Tolerance thresholds?**
   - SSIM > 0.99? 0.999? Platform-dependent?
   - Should we allow higher tolerance for WASM due to shader precision differences?

3. **Test image resolution?**
   - 1920×1080 (full HD, large files)
   - 1280×720 (HD, faster)
   - 512×512 (quick tests, small files)

4. **Reference storage?**
   - Git LFS for large PNGs?
   - Compressed archive?
   - Checksum-only with on-demand generation?

### Future Enhancements
- **Perceptual hash**: Faster comparison than pixel-by-pixel
- **Automatic Apophysis render**: Script Apophysis to generate ground truth
- **Mobile testing**: iOS/Android screenshot capture
- **Animation testing**: Verify frame-to-frame consistency
- **Performance regression**: Track render time per test case

---

## References

### Related Documentation
- [TESTING-GUIDE.md](TESTING-GUIDE.md) - Current testing infrastructure
- [ARCHITECTURE.md](ARCHITECTURE.md) - Codebase organization
- [WASM-STATUS.md](WASM-STATUS.md) - WASM build status

### External Resources
- [PNG Specification](http://www.libpng.org/pub/png/spec/1.2/PNG-Contents.html) - Text chunks for metadata
- [Structural Similarity (SSIM)](https://en.wikipedia.org/wiki/Structural_similarity) - Image comparison metric
- [Apophysis 7X](https://sourceforge.net/projects/apophysis7x/) - Reference implementation

---

**Status**: Phase 3 Complete, Phase 4 In Progress (3/60 test configs)
**Last Updated**: 2025-10-24
**Next Milestone**: Complete variation test config library (23 remaining configs)
