# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added - 2025-10-20
- **Palette Import/Export System**
  - Export palette to clipboard as JSON
  - Save palette as `.palette` file (desktop) or download (WASM)
  - Import palette from JSON text
  - Load palette from `.palette` file (desktop) or clipboard (WASM)
  - Palettes include a `name` field in the JSON format
  - Imported palettes automatically added to palette library
  - Palette editor automatically updated with imported palette
  - Full cross-platform support (desktop + WASM)

#### Implementation Details
- Files modified:
  - `src/ui/mod.rs` - Added Import/Export section to Palette Editor window
  - `src/app.rs` - Added handlers for palette import/export operations
- JSON format matches existing `Palette` struct serialization
- Reuses existing file dialog patterns from config import/export
- Preparation for future assets folder auto-loading system

#### JSON Format Example
```json
{
  "name": "My Custom Palette",
  "stops": [
    {
      "position": 0.0,
      "color": [1.0, 0.0, 0.0]
    },
    {
      "position": 0.5,
      "color": [0.0, 1.0, 0.0]
    },
    {
      "position": 1.0,
      "color": [0.0, 0.0, 1.0]
    }
  ]
}
```

---

## [0.1.0] - Initial Release

### Features
- GPU-accelerated fractal flame rendering using wgpu
- 16 variation functions (Linear, Sinusoidal, Spherical, etc.)
- Three color modes: Transform, Palette, Speed
- Interactive palette editor with gradient stops
- Real-time progressive refinement
- Full undo/redo system (50 states)
- Config import/export (.flame files)
- PNG export with transparency support
- Full WASM support for web builds
- Mouse and keyboard viewport navigation
- Performance metrics display

See [STATUS.md](STATUS.md) for detailed implementation status.
