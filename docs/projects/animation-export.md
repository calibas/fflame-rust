# Animation Export System

## Overview

Export animations as high-quality PNG sequences or video files. Each frame is rendered to completion (max_iterations) for production-quality output.

## Architecture

### Core Export Engine (`src/animation/export.rs`)

The core engine is designed to be reusable by both CLI and UI:

```rust
/// Animation export configuration
pub struct AnimationExportConfig {
    /// Base fractal config (will be modified by animation)
    pub config: FractalConfig,
    /// Animation to render
    pub animation: Animation,
    /// Output directory for frames
    pub output_dir: PathBuf,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Iterations per thread (GPU compute setting)
    pub iterations_per_thread: u32,
    /// Optional: encode to video after frames complete
    pub encode_video: bool,
    /// Video codec (if encode_video is true)
    pub video_codec: VideoCodec,
}

/// Export progress callback
pub trait ExportProgressCallback {
    fn on_frame_start(&mut self, frame: u32, total: u32, time: f64);
    fn on_frame_complete(&mut self, frame: u32, total: u32, elapsed_ms: f64);
    fn on_export_complete(&mut self, total_frames: u32, total_time_ms: f64);
    fn is_cancelled(&self) -> bool;
}

/// Core export function - reusable by CLI and UI
pub async fn export_animation(
    export_config: AnimationExportConfig,
    progress: &mut dyn ExportProgressCallback,
) -> Result<AnimationExportResult, AnimationExportError>;
```

### Export Process

For each frame:
1. Calculate frame time: `time = frame_index / fps`
2. Evaluate animation at time → get parameter values
3. Apply parameters to a copy of the base config
4. Create fresh renderer (or reset existing one)
5. Render until `max_iterations` reached
6. Save frame as PNG with metadata
7. Report progress

### CLI Integration (`src/main.rs`)

```bash
# Export animation to PNG sequence
fractal_flame export-animation \
  --config flame.fflame \
  --animation zoom.anim \
  --output ./frames/ \
  --width 1920 --height 1080 \
  --fps 30

# Export with video encoding (requires ffmpeg)
fractal_flame export-animation \
  --config flame.fflame \
  --animation zoom.anim \
  --output animation.mp4 \
  --width 1920 --height 1080 \
  --fps 30 \
  --video
```

### UI Integration

Add "Export Animation" button to Animation panel:
- Opens dialog with export settings (resolution, fps, output path)
- Shows progress bar during export
- Runs export in background thread (keeps UI responsive)
- Can be cancelled mid-export

## Implementation Phases

### Phase 1: Core Export Engine ✅
- [x] Create `src/animation/export.rs`
- [x] Add `AnimationExportConfig` struct
- [x] Add `ExportProgressCallback` trait
- [x] Implement `export_animation()` function
- [x] Add `evaluate_at_time()` to AnimationController

### Phase 2: CLI Support ✅
- [x] Add `export-animation` subcommand to clap
- [x] Implement CLI progress callback (prints to stdout)
- [x] Wire up CLI to core export engine

### Phase 3: UI Support ✅ (Basic)
- [x] Add "Export Animation" button to Animation panel
- [x] Create export settings dialog (collapsible in Animation panel)
- [ ] Implement background export with progress UI (currently blocks UI)
- [ ] Add cancel support

### Phase 4: Video Encoding (Optional)
- [ ] Detect ffmpeg availability
- [ ] Shell out to ffmpeg for video encoding
- [ ] Support common codecs (H.264, H.265, VP9)

## File Format

### PNG Sequence Output
```
output_dir/
  frame_0001.png
  frame_0002.png
  ...
  frame_0300.png
```

Each PNG includes metadata:
- Frame number and total frames
- Animation time
- All standard PNG metadata from single-frame export

### Video Output (Phase 4)
Uses ffmpeg command:
```bash
ffmpeg -framerate 30 -i frame_%04d.png -c:v libx264 -pix_fmt yuv420p output.mp4
```

## Performance Estimates

Based on current single-frame export benchmarks:
- 800×600 @ 10M iterations: ~0.5s per frame
- 1920×1080 @ 10M iterations: ~2-3s per frame

Example render times:
- 10s animation @ 30fps = 300 frames
- At 1080p: ~10-15 minutes total
- At 4K: ~30-45 minutes total

## Progress Reporting

CLI output:
```
Exporting animation: Zoom and Pan Demo
  Resolution: 1920x1080
  Duration: 10.0s @ 30fps = 300 frames

  Frame 127/300 (42.3%) - 2.3s/frame - ETA: 6:38
```

UI shows:
- Progress bar with percentage
- Current frame / total frames
- Time per frame average
- Estimated time remaining
- Cancel button

## Error Handling

- Validate animation exists before starting
- Validate output directory is writable
- Handle GPU device loss (retry once, then fail)
- Clean up partial output on cancellation
- Report specific errors (disk full, GPU timeout, etc.)
