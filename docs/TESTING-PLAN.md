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

### Phase 1: PNG Metadata (Immediate)
**Tasks:**
1. Add PNG metadata writing to existing export code
2. Implement `PngMetadata` struct
3. Serialize config + build info to PNG text chunks
4. Add metadata reading utility for verification

**Files to Modify:**
- `src/renderer/compute_kernel.rs` - PNG export
- `src/version.rs` - Build info
- `src/config.rs` - Config serialization
- Create `src/png_metadata.rs` - Metadata handling

**Deliverable:** All PNG exports include full metadata

### Phase 2: Headless PNG Export (Near-term)
**Tasks:**
1. Create `src/bin/visual_test.rs` - Headless test harness
2. Implement `tests/visual/tools/generate_references.rs`
3. Load .flame configs without GUI
4. Render to texture
5. Export PNG with metadata
6. Batch process all test configs

**Commands:**
```bash
# Generate reference images (one-time)
cargo run --bin visual_test -- generate-references

# Generate current images for comparison
cargo run --bin visual_test -- generate-current

# Run comparison
cargo run --bin visual_test -- compare
```

**Deliverable:** Automated PNG generation from .flame files

### Phase 3: Image Comparison (Near-term)
**Tasks:**
1. Integrate `image-compare` crate (or similar)
2. Implement SSIM (structural similarity) comparison
3. Calculate MSE (mean squared error)
4. Generate visual diff images
5. Create HTML report with side-by-side comparison

**Metrics:**
- **SSIM** > 0.99 = PASS (allows minor float/AA differences)
- **Diff %** < 1.0% = PASS (% of pixels different)
- **Max Diff** < 10 (0-255 scale) = PASS

**Deliverable:** Automated visual regression detection

### Phase 4: Test Config Library (Near-term)
**Tasks:**
1. Create ~60 test .flame files in `tests/visual/configs/`
2. Organize by category (variations/, render_modes/, etc.)
3. Create `manifest.json` with test metadata
4. Document expected results

**Deliverable:** Comprehensive test suite

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

### Automated Testing
```bash
# 1. Generate baseline (one-time or when intentionally changed)
cargo run --bin visual_test -- generate-references

# 2. Make code changes
# ... edit src/gpu/buffers.rs ...

# 3. Run visual regression test
cargo test --test visual_regression

# 4. Review failures (if any)
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
# After verifying changes are correct
cargo run --bin visual_test -- update-references --test linear_variation

# Or update all
cargo run --bin visual_test -- update-references --all
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
cargo test                        # Unit tests
cargo test --test regression      # Integration tests
cargo test --test visual_regression # Visual tests (NEW)
cargo bench                       # Benchmarks

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

### Phase 1 (PNG Metadata) - Complete When:
- [ ] All PNG exports include version, build, config metadata
- [ ] Metadata can be read and verified
- [ ] Config from PNG can recreate exact render

### Phase 2 (Headless Export) - Complete When:
- [ ] Can batch-generate PNGs from .flame files without GUI
- [ ] Generates 60 test images in <2 minutes (fast mode)
- [ ] All images include complete metadata

### Phase 3 (Image Comparison) - Complete When:
- [ ] Automated SSIM/MSE comparison working
- [ ] Visual diff images generated
- [ ] HTML report shows side-by-side comparison
- [ ] Pass/fail thresholds configurable

### Phase 4 (Test Library) - Complete When:
- [ ] 60+ test .flame files created
- [ ] All variations covered
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

**Status**: Phase 1 (PNG Metadata) - In Progress
**Last Updated**: 2025-10-24
**Next Milestone**: Implement PNG metadata writing
