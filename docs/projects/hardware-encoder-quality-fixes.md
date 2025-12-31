# Hardware Encoder Quality Fixes + Preset/Tune Options

**Branch:** `fix/animation-export-race-conditions` (expanded scope)
**Created:** 2025-12-31
**Status:** In Progress

## Problem Statement

Hardware encoder quality settings (CRF) don't work correctly for NVIDIA NVENC, Intel QSV, and Apple VideoToolbox encoders. Additionally, users cannot control encoding speed/quality tradeoffs (preset) or optimization targets (tune) that are available in FFmpeg.

## Issues Found

### 1. NVIDIA NVENC - CRF Ignored ❌

**Current Code:**
```rust
HardwareAccel::Nvenc => {
    ffmpeg.arg("-rc").arg("vbr");  // Variable bitrate mode
    ffmpeg.arg("-cq").arg(settings.quality.to_string());  // Ignored without bitrate!
    ffmpeg.arg("-preset").arg("p4");
}
```

**Problem:**
- `-rc vbr` = Variable Bit Rate mode (designed for bitrate targets, not quality)
- `-cq` in VBR mode requires `-b:v` bitrate to be set
- Without bitrate, NVENC ignores the quality setting entirely
- Result: Encoder uses default bitrate mode regardless of CRF slider

**Fix:**
```rust
HardwareAccel::Nvenc => {
    ffmpeg.arg("-rc").arg("constqp");  // Constant Quantization Parameter
    ffmpeg.arg("-qp").arg(settings.quality.to_string());  // 0-51, same scale as CRF
    ffmpeg.arg("-preset").arg(nvenc_preset);  // p1-p7 (varies by preset setting)
}
```

### 2. Intel QSV - Missing Look-Ahead Disable ⚠️

**Current Code:**
```rust
HardwareAccel::Qsv => {
    ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
    ffmpeg.arg("-preset").arg("medium");
}
```

**Problem:**
- `-global_quality` works but QSV may enable look-ahead bitrate optimization by default
- Look-ahead can override constant quality mode in some scenarios
- Inconsistent behavior across QSV driver versions

**Fix:**
```rust
HardwareAccel::Qsv => {
    ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
    ffmpeg.arg("-look_ahead").arg("0");  // Force pure constant quality
    ffmpeg.arg("-preset").arg(qsv_preset);  // veryfast/faster/fast/medium/slow/slower/veryslow
}
```

### 3. Apple VideoToolbox - Wrong Quality Scale ❌

**Current Code:**
```rust
HardwareAccel::VideoToolbox => {
    // Convert CRF-style (lower=better) to VT-style (higher=better)
    let vt_quality = 100 - (settings.quality as i32 * 2).min(100).max(0);
    ffmpeg.arg("-q:v").arg(vt_quality.to_string());
}
```

**Problem:**
- VideoToolbox `-q:v` scale: 0 = auto, 1-100 = quality (100 = best)
- Current formula: `100 - (crf * 2)` → CRF 18 becomes VT 64 (mid-quality!)
- Should be: CRF 18 → VT ~80-85 for visually lossless
- Scale is non-linear and conversion is incorrect

**Fix:**
```rust
HardwareAccel::VideoToolbox => {
    // Map CRF 0-51 to VT 100-1 (linear interpolation)
    // CRF 0 → VT 100 (best), CRF 18 → VT 82, CRF 51 → VT 1 (worst)
    let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
    ffmpeg.arg("-q:v").arg(vt_quality.to_string());
}
```

### 4. AMD AMF - Already Correct ✅

**Current Code:**
```rust
HardwareAccel::Amf => {
    ffmpeg.arg("-rc").arg("cqp");  // Constant QP mode
    ffmpeg.arg("-qp").arg(settings.quality.to_string());
}
```

**Status:** No changes needed! AMF implementation is already correct.

## New Feature: Preset and Tune Options

### Preset (Speed/Quality Tradeoff)

**Purpose:** Control encoding speed vs compression efficiency
- **Faster presets** = quicker encoding, larger files
- **Slower presets** = better compression, smaller files at same quality

**FFmpeg Parameter:** `-preset <value>`

**Options by Encoder:**

| Encoder | Presets Available |
|---------|-------------------|
| **CPU (libx264/libx265)** | ultrafast, superfast, veryfast, faster, fast, medium, slow, slower, veryslow, placebo |
| **NVENC** | p1 (fastest), p2, p3, p4 (balanced), p5, p6, p7 (slowest) |
| **QSV** | veryfast, faster, fast, medium, slow, slower, veryslow |
| **AMF** | speed, balanced, quality |
| **VideoToolbox** | Not supported (no preset parameter) |

**Default:** `medium` for CPU, `p4` (balanced) for NVENC, `balanced` for AMF

### Tune (Optimization Target)

**Purpose:** Optimize encoder for specific content types
- **film** - Live action with grain
- **animation** - Flat colors, sharp edges (OUR USE CASE!)
- **grain** - Preserve film grain
- **stillimage** - Slides/presentations
- **fastdecode** - Optimize for playback performance

**FFmpeg Parameter:** `-tune <value>`

**Availability:**
- ✅ **CPU (libx264/libx265)** - Full tune support
- ❌ **NVENC** - Has `-tune` but limited options (hq, ll, ull, lossless)
- ❌ **QSV** - No tune parameter
- ❌ **AMF** - No tune parameter
- ❌ **VideoToolbox** - No tune parameter

**Recommendation:**
- Offer tune options only for CPU encoders
- Default to `animation` for fractal flame content
- Hide tune UI when hardware acceleration is enabled

## Implementation Plan

### Phase 1: Data Model Changes

#### 1.1 Add Preset/Tune Enums

**File:** `src/animation/export.rs`

```rust
/// Encoding preset (speed/quality tradeoff)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingPreset {
    // CPU presets (libx264/libx265)
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
    Placebo,

    // NVENC presets
    NvencP1,  // Fastest
    NvencP2,
    NvencP3,
    NvencP4,  // Balanced
    NvencP5,
    NvencP6,
    NvencP7,  // Slowest

    // QSV presets
    QsvVeryfast,
    QsvFaster,
    QsvFast,
    QsvMedium,
    QsvSlow,
    QsvSlower,
    QsvVeryslow,

    // AMF presets
    AmfSpeed,
    AmfBalanced,
    AmfQuality,
}

impl Default for EncodingPreset {
    fn default() -> Self {
        Self::Medium
    }
}

impl EncodingPreset {
    /// Get FFmpeg preset string for this preset + hardware accel combination
    pub fn ffmpeg_arg(&self, hw_accel: HardwareAccel) -> &'static str {
        match hw_accel {
            HardwareAccel::None => match self {
                Self::Ultrafast => "ultrafast",
                Self::Superfast => "superfast",
                Self::Veryfast => "veryfast",
                Self::Faster => "faster",
                Self::Fast => "fast",
                Self::Medium => "medium",
                Self::Slow => "slow",
                Self::Slower => "slower",
                Self::Veryslow => "veryslow",
                Self::Placebo => "placebo",
                _ => "medium",  // Fallback for mismatched preset
            },
            HardwareAccel::Nvenc => match self {
                Self::NvencP1 => "p1",
                Self::NvencP2 => "p2",
                Self::NvencP3 => "p3",
                Self::NvencP4 => "p4",
                Self::NvencP5 => "p5",
                Self::NvencP6 => "p6",
                Self::NvencP7 => "p7",
                _ => "p4",  // Fallback
            },
            HardwareAccel::Qsv => match self {
                Self::QsvVeryfast => "veryfast",
                Self::QsvFaster => "faster",
                Self::QsvFast => "fast",
                Self::QsvMedium => "medium",
                Self::QsvSlow => "slow",
                Self::QsvSlower => "slower",
                Self::QsvVeryslow => "veryslow",
                _ => "medium",
            },
            HardwareAccel::Amf => match self {
                Self::AmfSpeed => "speed",
                Self::AmfBalanced => "balanced",
                Self::AmfQuality => "quality",
                _ => "balanced",
            },
            HardwareAccel::VideoToolbox => "",  // No preset support
        }
    }

    /// Get available presets for a hardware accelerator
    pub fn available_for(hw_accel: HardwareAccel) -> Vec<Self> {
        match hw_accel {
            HardwareAccel::None => vec![
                Self::Ultrafast, Self::Superfast, Self::Veryfast,
                Self::Faster, Self::Fast, Self::Medium,
                Self::Slow, Self::Slower, Self::Veryslow, Self::Placebo,
            ],
            HardwareAccel::Nvenc => vec![
                Self::NvencP1, Self::NvencP2, Self::NvencP3, Self::NvencP4,
                Self::NvencP5, Self::NvencP6, Self::NvencP7,
            ],
            HardwareAccel::Qsv => vec![
                Self::QsvVeryfast, Self::QsvFaster, Self::QsvFast, Self::QsvMedium,
                Self::QsvSlow, Self::QsvSlower, Self::QsvVeryslow,
            ],
            HardwareAccel::Amf => vec![
                Self::AmfSpeed, Self::AmfBalanced, Self::AmfQuality,
            ],
            HardwareAccel::VideoToolbox => vec![],  // No presets
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ultrafast => "Ultrafast (fastest encoding)",
            Self::Superfast => "Superfast",
            Self::Veryfast => "Very Fast",
            Self::Faster => "Faster",
            Self::Fast => "Fast",
            Self::Medium => "Medium (balanced)",
            Self::Slow => "Slow",
            Self::Slower => "Slower",
            Self::Veryslow => "Very Slow",
            Self::Placebo => "Placebo (slowest, minimal gain)",

            Self::NvencP1 => "P1 (fastest)",
            Self::NvencP2 => "P2",
            Self::NvencP3 => "P3",
            Self::NvencP4 => "P4 (balanced)",
            Self::NvencP5 => "P5",
            Self::NvencP6 => "P6",
            Self::NvencP7 => "P7 (slowest)",

            Self::QsvVeryfast => "Very Fast",
            Self::QsvFaster => "Faster",
            Self::QsvFast => "Fast",
            Self::QsvMedium => "Medium (balanced)",
            Self::QsvSlow => "Slow",
            Self::QsvSlower => "Slower",
            Self::QsvVeryslow => "Very Slow",

            Self::AmfSpeed => "Speed (fastest)",
            Self::AmfBalanced => "Balanced",
            Self::AmfQuality => "Quality (slowest)",
        }
    }
}

/// Encoding tune (optimization target) - CPU encoders only
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingTune {
    None,        // No tune parameter
    Film,        // Live action with grain
    Animation,   // Flat colors, sharp edges (recommended for fractals!)
    Grain,       // Preserve film grain
    StillImage,  // Slides/presentations
    FastDecode,  // Optimize for playback
}

impl Default for EncodingTune {
    fn default() -> Self {
        Self::Animation  // Perfect for fractal flames!
    }
}

impl EncodingTune {
    /// Get FFmpeg tune string (empty if None)
    pub fn ffmpeg_arg(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Film => Some("film"),
            Self::Animation => Some("animation"),
            Self::Grain => Some("grain"),
            Self::StillImage => Some("stillimage"),
            Self::FastDecode => Some("fastdecode"),
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Film => "Film (live action with grain)",
            Self::Animation => "Animation (flat colors, recommended)",
            Self::Grain => "Grain (preserve film grain)",
            Self::StillImage => "Still Image (slides/presentations)",
            Self::FastDecode => "Fast Decode (optimize playback)",
        }
    }
}
```

#### 1.2 Update VideoEncodingSettings

**File:** `src/animation/export.rs`

```rust
#[derive(Debug, Clone)]
pub struct VideoEncodingSettings {
    pub codec: VideoCodec,
    pub hardware_accel: HardwareAccel,
    pub quality: u8,
    pub preset: EncodingPreset,   // NEW
    pub tune: EncodingTune,       // NEW (CPU only)
}

impl Default for VideoEncodingSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::default(),
            hardware_accel: HardwareAccel::default(),
            quality: 18,
            preset: EncodingPreset::default(),  // Medium
            tune: EncodingTune::default(),      // Animation
        }
    }
}
```

### Phase 2: FFmpeg Command Building

#### 2.1 Update build_ffmpeg_command() for H.264

**File:** `src/animation/export.rs` (lines 784-818)

```rust
VideoCodec::H264 => {
    if is_hardware {
        match settings.hardware_accel {
            HardwareAccel::Nvenc => {
                // FIX: Use constqp instead of vbr
                ffmpeg.arg("-rc").arg("constqp");
                ffmpeg.arg("-qp").arg(settings.quality.to_string());

                // NEW: Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                if !preset.is_empty() {
                    ffmpeg.arg("-preset").arg(preset);
                }
            }
            HardwareAccel::Qsv => {
                // FIX: Add look_ahead 0
                ffmpeg.arg("-global_quality").arg(settings.quality.to_string());
                ffmpeg.arg("-look_ahead").arg("0");

                // NEW: Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                if !preset.is_empty() {
                    ffmpeg.arg("-preset").arg(preset);
                }
            }
            HardwareAccel::Amf => {
                // Already correct
                ffmpeg.arg("-rc").arg("cqp");
                ffmpeg.arg("-qp").arg(settings.quality.to_string());

                // NEW: Apply preset
                let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
                if !preset.is_empty() {
                    ffmpeg.arg("-preset").arg(preset);
                }
            }
            HardwareAccel::VideoToolbox => {
                // FIX: Correct quality scale
                let vt_quality = (100 - (settings.quality as i32 * 100 / 51)).clamp(1, 100);
                ffmpeg.arg("-q:v").arg(vt_quality.to_string());
                // No preset support for VideoToolbox
            }
            HardwareAccel::None => unreachable!(),
        }
    } else {
        // CPU libx264
        ffmpeg.arg("-crf").arg(settings.quality.to_string());

        // NEW: Apply preset
        let preset = settings.preset.ffmpeg_arg(settings.hardware_accel);
        ffmpeg.arg("-preset").arg(preset);

        // NEW: Apply tune (CPU only)
        if let Some(tune) = settings.tune.ffmpeg_arg() {
            ffmpeg.arg("-tune").arg(tune);
        }
    }
    ffmpeg.arg("-pix_fmt").arg("yuv420p");
}
```

Apply same pattern to H.265 (lines 819-848) and VP9 (lines 849-859).

#### 2.2 Update Second FFmpeg Builder

**File:** `src/animation/export.rs` (lines 1393-1485)

Apply identical changes to the duplicated `build_ffmpeg_args()` function.

### Phase 3: UI Changes

#### 3.1 Add Preset Dropdown

**File:** `src/ui/animation_panel.rs`

```rust
// After hardware acceleration dropdown
ui.horizontal(|ui| {
    ui.label("Preset:");

    // Get available presets for current hardware accel
    let available_presets = EncodingPreset::available_for(settings.hardware_accel);

    if available_presets.is_empty() {
        ui.label("(not supported)");
    } else {
        egui::ComboBox::from_id_source("preset_combo")
            .selected_text(settings.preset.display_name())
            .show_ui(ui, |ui| {
                for preset in available_presets {
                    ui.selectable_value(
                        &mut settings.preset,
                        preset,
                        preset.display_name()
                    );
                }
            });
    }
});
```

#### 3.2 Add Tune Dropdown (CPU Only)

**File:** `src/ui/animation_panel.rs`

```rust
// Show tune only for CPU encoders
if settings.hardware_accel == HardwareAccel::None {
    ui.horizontal(|ui| {
        ui.label("Tune:");
        egui::ComboBox::from_id_source("tune_combo")
            .selected_text(settings.tune.display_name())
            .show_ui(ui, |ui| {
                for tune in [
                    EncodingTune::None,
                    EncodingTune::Animation,  // Recommended default
                    EncodingTune::Film,
                    EncodingTune::Grain,
                    EncodingTune::StillImage,
                    EncodingTune::FastDecode,
                ] {
                    ui.selectable_value(
                        &mut settings.tune,
                        tune,
                        tune.display_name()
                    );
                }
            });
    });
    ui.small("Animation tune is recommended for fractal flames");
}
```

#### 3.3 Update AnimationExportSettings

**File:** `src/ui/animation_panel.rs`

```rust
#[derive(Clone)]
pub struct AnimationExportSettings {
    pub output_path: std::path::PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub iterations_per_thread: u32,
    pub video_codec: VideoCodec,
    pub hardware_accel: HardwareAccel,
    pub video_quality: u8,
    pub preset: EncodingPreset,   // NEW
    pub tune: EncodingTune,       // NEW
}

impl Default for AnimationExportSettings {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            preset: EncodingPreset::default(),
            tune: EncodingTune::default(),
        }
    }
}
```

#### 3.4 Wire Up Settings to VideoEncodingSettings

**File:** `src/app/mod.rs` (animation export handler)

```rust
let video_settings = VideoEncodingSettings {
    codec: export_settings.video_codec,
    hardware_accel: export_settings.hardware_accel,
    quality: export_settings.video_quality,
    preset: export_settings.preset,   // NEW
    tune: export_settings.tune,       // NEW
};
```

### Phase 4: Testing Plan

#### 4.1 Encoder Quality Tests

For each hardware encoder:

1. **Set CRF to 18** (visually lossless)
2. **Export 30-second test animation**
3. **Check file size** (should be reasonable, not bloated)
4. **Visual inspection** (no compression artifacts)
5. **Vary CRF 0/18/30/51** (verify quality changes)

**Encoders to test:**
- ✅ CPU libx264 (baseline)
- ⚠️ NVIDIA NVENC (primary fix)
- ⚠️ Intel QSV (if available)
- ⚠️ Apple VideoToolbox (macOS only)
- ⚠️ AMD AMF (if available)

#### 4.2 Preset/Tune Tests

1. **CPU encoder with different presets:**
   - `ultrafast` vs `medium` vs `veryslow` (check encoding time + file size)
2. **CPU encoder with different tunes:**
   - `animation` vs `film` vs `none` (check file size + quality)
3. **NVENC with different presets:**
   - `p1` vs `p4` vs `p7` (check encoding time + quality)
4. **Preset UI updates correctly** when switching hardware accel

#### 4.3 Regression Tests

- ✅ Existing animations export without errors
- ✅ Default settings produce good quality
- ✅ CLI export still works (add CLI args for preset/tune)

## Implementation Checklist

- [x] Create project document
- [ ] Add EncodingPreset enum with all variants
- [ ] Add EncodingTune enum with all variants
- [ ] Update VideoEncodingSettings struct
- [ ] Fix NVENC: vbr → constqp
- [ ] Fix QSV: Add look_ahead 0
- [ ] Fix VideoToolbox: Correct quality conversion
- [ ] Apply preset to H.264 FFmpeg command
- [ ] Apply preset to H.265 FFmpeg command
- [ ] Apply preset to VP9 FFmpeg command
- [ ] Apply tune to CPU encoders
- [ ] Update second FFmpeg builder (avoid duplication later)
- [ ] Add preset dropdown to UI
- [ ] Add tune dropdown to UI (CPU only)
- [ ] Update AnimationExportSettings struct
- [ ] Wire up UI → VideoEncodingSettings
- [ ] Add CLI args for preset/tune
- [ ] Test NVENC quality fix
- [ ] Test VideoToolbox quality fix
- [ ] Test preset options
- [ ] Test tune options
- [ ] Commit changes

## Future Improvements (Out of Scope)

1. **Deduplicate FFmpeg builders** - Merge two nearly-identical functions
2. **Encoder auto-detection** - Probe ffmpeg for available encoders
3. **Per-encoder quality ranges** - Some encoders use 0-63, not 0-51
4. **Two-pass encoding** - Better quality for target file size
5. **Bitrate mode option** - Some users prefer CBR/VBR over CRF
6. **NVENC tune support** - Add `hq` (high quality) tune for NVENC
7. **Custom FFmpeg args** - Advanced users can inject arbitrary parameters

## References

- [FFmpeg H.264 encoding guide](https://trac.ffmpeg.org/wiki/Encode/H.264)
- [FFmpeg H.265 encoding guide](https://trac.ffmpeg.org/wiki/Encode/H.265)
- [NVENC documentation](https://docs.nvidia.com/video-technologies/video-codec-sdk/ffmpeg-with-nvidia-gpu/)
- [Intel QSV documentation](https://trac.ffmpeg.org/wiki/Hardware/QuickSync)
- [AMD AMF documentation](https://github.com/GPUOpen-LibrariesAndSDKs/AMF)
- [VideoToolbox documentation](https://ffmpeg.org/ffmpeg-codecs.html#videotoolbox)
