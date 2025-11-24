# Preset Browser & Fractal Config Gallery System

**Status:** Design Phase
**Created:** 2025-11-24
**Updated:** 2025-11-24

## Overview

A reusable gallery/browser system for viewing and selecting FractalConfigs in different contexts:
- Built-in presets (current dropdown replacement)
- User-saved fractals (personal library)
- Fractal packs (collections in files)
- Auto-saved backups (undo/recovery system)

## Design Goals

1. **Reusable Component:** Single `FractalConfigGallery` widget works for all use cases
2. **Multiple View Modes:** Grid (thumbnails) and List (details) views
3. **Visual Previews:** High-quality 512×512 thumbnails scaled in UI
4. **Performance:** Lazy loading, texture caching, smooth scrolling
5. **Responsive:** Adapts to window size, mobile-friendly layout
6. **Professional:** Modern asset browser UX (Unity/Blender style)

## User Interface

### View Modes

#### **Grid View** (Default)
```
┌─────────────────────────────────────────────────┐
│  Preset Browser                    [≡] [Grid] [X]│
├─────────────────────────────────────────────────┤
│  Search: [_______________] 🔍                   │
├─────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────┐  │
│  │                                           │  │
│  │   ┌────┐  ┌────┐  ┌────┐  ┌────┐        │  │
│  │   │    │  │    │  │    │  │    │        │  │  128×128 thumbnails
│  │   │ P1 │  │ P2 │  │ P3 │  │ P4 │        │  │  (scaled from 512×512)
│  │   │    │  │    │  │    │  │    │        │  │
│  │   └────┘  └────┘  └────┘  └────┘        │  │
│  │   Simple  Sphere  Spiral  Julia         │  │
│  │                                           │  │
│  │   ┌────┐  ┌────┐  ┌────┐  ┌────┐        │  │
│  │   │    │  │    │  │    │  │    │        │  │
│  │   │ P5 │  │ P6 │  │ P7 │  │ P8 │        │  │
│  │   │    │  │    │  │    │  │    │        │  │
│  │   └────┘  └────┘  └────┘  └────┘        │  │
│  │   Complex Flower  3D      JDisc          │  │
│  │                                           │  │
│  └──────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

#### **List View** (Details)
```
┌─────────────────────────────────────────────────┐
│  Preset Browser                    [≡] [List] [X]│
├─────────────────────────────────────────────────┤
│  Search: [_______________] 🔍                   │
├─────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────┐  │
│  │ [▶] Simple                               │  │
│  │     2 transforms, 2D, Linear+Sinusoidal  │  │
│  │                                           │  │
│  │ [▶] Spherical                            │  │
│  │     2 transforms, 2D, Spherical          │  │
│  │                                           │  │
│  │ [▶] Spiral                               │  │
│  │     2 transforms, 2D, Spiral+Linear      │  │
│  │                                           │  │
│  │ [▶] Julia                                │  │
│  │     1 transform, 2D, Julia               │  │
│  │                                           │  │
│  │ [▶] Complex                              │  │
│  │     4 transforms, 2D, Multi-variation    │  │
│  │                                           │  │
│  └──────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### Controls

**Toolbar:**
- **View Mode Toggle:** Grid ⬄ List (button or icon)
- **Sort:** By Name / Date / Transform Count (dropdown)
- **Search:** Text filter for names (future: filter by variations, mode, etc.)

**Grid Settings:**
- **Thumbnail Size:** Slider (64px to 256px, default 128px)
- **Columns:** Auto-calculated based on panel width and thumbnail size
- **Spacing:** 16px horizontal, 24px vertical (name below thumbnail)

**List Settings:**
- **Row Height:** Auto-calculated based on content
- **Expandable:** Click to show full details (variations, settings, etc.)

## Technical Architecture

### Core Component: `FractalConfigGallery`

**Purpose:** Reusable widget for browsing/selecting FractalConfigs

**API:**
```rust
pub struct FractalConfigGallery {
    /// Configs to display
    configs: Vec<FractalConfig>,

    /// Current view mode
    view_mode: GalleryViewMode,

    /// Search filter
    search_query: String,

    /// Thumbnail size (grid view)
    thumbnail_size: f32,

    /// Texture cache (egui TextureHandles)
    texture_cache: HashMap<usize, egui::TextureHandle>,

    /// Selected config index
    selected_index: Option<usize>,
}

pub enum GalleryViewMode {
    Grid,   // Thumbnail grid
    List,   // Detailed list
}

impl FractalConfigGallery {
    pub fn new(configs: Vec<FractalConfig>) -> Self;

    /// Render gallery UI, returns selected config index
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<usize>;

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: GalleryViewMode);

    /// Update search filter
    pub fn set_search(&mut self, query: String);

    /// Get filtered configs
    fn filtered_configs(&self) -> Vec<(usize, &FractalConfig)>;

    /// Render grid view
    fn render_grid(&mut self, ui: &mut egui::Ui) -> Option<usize>;

    /// Render list view
    fn render_list(&mut self, ui: &mut egui::Ui) -> Option<usize>;

    /// Get or generate thumbnail texture
    fn get_thumbnail(&mut self, ctx: &egui::Context, index: usize, config: &FractalConfig) -> egui::TextureHandle;
}
```

### Thumbnail System

**Approach:** Runtime headless rendering (on-demand generation)

**Architecture:**
```rust
pub struct ThumbnailRenderer {
    /// Shared GPU device/queue from main app
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    /// Thumbnail render size
    width: u32,
    height: u32,

    /// Target iterations per thumbnail
    iterations: u64,
}

impl ThumbnailRenderer {
    /// Create renderer using existing GPU resources
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            width: 512,
            height: 512,
            iterations: 200_000_000,
        }
    }

    /// Render config to RGBA image (blocking)
    pub fn render_thumbnail(&self, config: &FractalConfig) -> image::RgbaImage {
        // Reuse headless export code
        let renderer = FlameRenderer::new_headless(
            &self.device,
            &self.queue,
            self.width,
            self.height,
        );

        renderer.load_config(config);

        // Render to target iterations
        let dispatches = (self.iterations / ITERATIONS_PER_DISPATCH as u64) as u32;
        for _ in 0..dispatches {
            renderer.render_dispatch();
        }

        // Read pixels from GPU
        renderer.read_accumulation_buffer()
    }

    /// Async version (non-blocking)
    pub async fn render_thumbnail_async(&self, config: &FractalConfig) -> image::RgbaImage {
        // Same as above but with async GPU operations
        // Allows UI to remain responsive during generation
    }
}
```

**Integration with Gallery:**
```rust
impl FractalConfigGallery {
    /// Get or generate thumbnail texture
    fn get_thumbnail(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        config: &FractalConfig,
        renderer: &ThumbnailRenderer,
    ) -> Option<egui::TextureHandle> {
        let texture_id = egui::Id::new(("preset_thumbnail", index));

        // Check cache first
        if let Some(cached) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(texture_id)) {
            return Some(cached);
        }

        // Check if generation is in progress
        if self.rendering_thumbnails.contains(&index) {
            return None; // Show placeholder
        }

        // Start async generation
        self.rendering_thumbnails.insert(index);

        // TODO: Spawn async task to render thumbnail
        // For now, render synchronously (blocks UI briefly)
        let image = renderer.render_thumbnail(config);

        // Convert to egui ColorImage
        let size = [image.width() as usize, image.height() as usize];
        let pixels = image.into_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels.samples);

        // Upload to GPU
        let tex = ctx.load_texture(
            format!("preset_{}", index),
            color_image,
            egui::TextureOptions::LINEAR, // Smooth scaling
        );

        // Cache in egui memory
        ctx.data_mut(|data| {
            data.insert_temp(texture_id, tex.clone());
        });

        self.rendering_thumbnails.remove(&index);
        Some(tex)
    }
}
```

**Loading States:**
```rust
// While rendering, show placeholder
if let Some(texture) = self.get_thumbnail(ctx, index, config, renderer) {
    ui.image(&texture).fit_to_exact_size(egui::vec2(128.0, 128.0));
} else {
    // Show loading spinner
    ui.add_sized(
        egui::vec2(128.0, 128.0),
        egui::Spinner::new().size(32.0),
    );
}
```

**Performance Characteristics:**
- Rendering time: ~0.5-2 seconds per thumbnail @ 512×512, 200M iterations
- Total for 10 presets: ~5-20 seconds (one-time cost on first open)
- Subsequent opens: Instant (textures cached in egui memory)
- Memory usage: ~1 MB per thumbnail (10 MB for 10 presets)

**Optimization Strategies:**
1. **Lazy generation:** Only render visible thumbnails (virtual scrolling)
2. **Background generation:** Spawn async tasks to avoid blocking UI
3. **Lower quality preview:** Initial pass at 64×64, 10M iterations, then upgrade
4. **Disk cache:** Save rendered thumbnails to temp directory for future sessions
5. **Progressive rendering:** Show partial results during generation

### Grid Layout

**Responsive Column Count:**
```rust
fn calculate_columns(panel_width: f32, thumbnail_size: f32) -> usize {
    let spacing = 16.0;
    let total_item_width = thumbnail_size + spacing;
    let columns = (panel_width / total_item_width).floor() as usize;
    columns.max(1) // Minimum 1 column
}
```

**egui Grid Implementation:**
```rust
fn render_grid(&mut self, ui: &mut egui::Ui) -> Option<usize> {
    let mut selected = None;
    let panel_width = ui.available_width();
    let columns = self.calculate_columns(panel_width);

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("fractal_gallery_grid")
            .num_columns(columns)
            .spacing([16.0, 24.0])  // h_spacing, v_spacing
            .striped(false)
            .show(ui, |ui| {
                let filtered = self.filtered_configs();

                for (col, (index, config)) in filtered.iter().enumerate() {
                    // Get thumbnail texture
                    let texture = self.get_thumbnail(ui.ctx(), *index, config);

                    // Allocate space for card
                    let card_size = egui::vec2(self.thumbnail_size, self.thumbnail_size + 24.0);
                    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                    // Hover highlight
                    if response.hovered() {
                        ui.painter().rect_filled(
                            rect.expand(4.0),
                            4.0,
                            ui.visuals().widgets.hovered.bg_fill,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Render thumbnail
                    let img_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(self.thumbnail_size, self.thumbnail_size),
                    );
                    ui.put(img_rect, egui::Image::new(&texture)
                        .fit_to_exact_size(egui::vec2(self.thumbnail_size, self.thumbnail_size)));

                    // Render name below
                    let name_rect = egui::Rect::from_min_size(
                        img_rect.min + egui::vec2(0.0, self.thumbnail_size + 4.0),
                        egui::vec2(self.thumbnail_size, 20.0),
                    );
                    ui.painter().text(
                        name_rect.center_top() + egui::vec2(0.0, 2.0),
                        egui::Align2::CENTER_TOP,
                        &config.flame.name,
                        egui::FontId::default(),
                        ui.visuals().text_color(),
                    );

                    // Handle click
                    if response.clicked() {
                        selected = Some(*index);
                    }

                    // Move to next column
                    if (col + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });

    selected
}
```

### List Layout

**egui Implementation:**
```rust
fn render_list(&mut self, ui: &mut egui::Ui) -> Option<usize> {
    let mut selected = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        let filtered = self.filtered_configs();

        for (index, config) in filtered {
            // Collapsing header for each item
            let header_id = ui.make_persistent_id(format!("config_list_{}", index));

            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                false,  // Default collapsed
            )
            .show_header(ui, |ui| {
                // Row with name and summary
                ui.horizontal(|ui| {
                    // Play icon (clickable)
                    if ui.button("▶").clicked() {
                        selected = Some(index);
                    }

                    ui.strong(&config.flame.name);
                });

                // Summary line
                ui.label(format!(
                    "{} transforms, {}, {}",
                    config.flame.transforms.len(),
                    match config.flame.render_mode {
                        RenderMode::TwoD => "2D",
                        RenderMode::ThreeD => "3D",
                    },
                    self.format_variations(config),
                ));
            })
            .body(|ui| {
                // Expanded details
                ui.label(format!("Zoom: {:.2}", config.zoom));
                ui.label(format!("Max Iterations: {}", config.max_iterations));
                ui.label(format!("Color Mode: {:?}", config.color_mode));

                // Optional: Show small thumbnail in expanded view
                if let Some(texture) = self.texture_cache.get(&index) {
                    ui.image(texture).fit_to_exact_size(egui::vec2(64.0, 64.0));
                }
            });

            ui.separator();
        }
    });

    selected
}

fn format_variations(&self, config: &FractalConfig) -> String {
    // Collect unique variation names from all transforms
    let mut variations: Vec<String> = config.flame.transforms
        .iter()
        .flat_map(|t| t.variations.keys().cloned())
        .collect();
    variations.sort();
    variations.dedup();

    if variations.len() <= 3 {
        variations.join(", ")
    } else {
        format!("{}, {} more", variations[..2].join(", "), variations.len() - 2)
    }
}
```

## Panel Integration

### 1. Preset Browser Panel

**Location:** `src/ui/preset_browser.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryViewMode};
use crate::scene::presets::PresetLibrary;

pub struct PresetBrowserPanel {
    gallery: FractalConfigGallery,
}

impl PresetBrowserPanel {
    pub fn new(library: &PresetLibrary) -> Self {
        Self {
            gallery: FractalConfigGallery::new(library.presets().to_vec()),
        }
    }

    /// Render panel, returns selected preset index
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        self.gallery.render(ui)
    }
}
```

### 2. User Fractal Library Panel

**Location:** `src/ui/user_library.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryViewMode};
use crate::config::FractalConfig;

pub struct UserLibraryPanel {
    gallery: FractalConfigGallery,
}

impl UserLibraryPanel {
    pub fn new(user_configs: Vec<FractalConfig>) -> Self {
        Self {
            gallery: FractalConfigGallery::new(user_configs),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        ui.horizontal(|ui| {
            if ui.button("➕ Add Current").clicked() {
                // TODO: Add current config to library
            }
            if ui.button("🗑 Delete Selected").clicked() {
                // TODO: Delete selected config
            }
        });

        ui.separator();

        self.gallery.render(ui)
    }
}
```

### 3. Backup Browser Panel

**Location:** `src/ui/backup_browser.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryViewMode};
use crate::config::FractalConfig;

pub struct BackupBrowserPanel {
    gallery: FractalConfigGallery,
}

impl BackupBrowserPanel {
    pub fn new(backups: Vec<FractalConfig>) -> Self {
        Self {
            gallery: FractalConfigGallery::new(backups),
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        ui.label("Auto-saved backups from this session");
        ui.separator();

        self.gallery.render(ui)
    }
}
```

## Menu Integration

### File Menu

```rust
// src/ui/menu_bar.rs
ui.menu_button(t!("menu.file"), |ui| {
    if ui.button("📂 Open...").clicked() {
        menu_actions.file.load_config = true;
    }

    if ui.button("💾 Save As...").clicked() {
        menu_actions.file.save_config = true;
    }

    ui.separator();

    // NEW: Preset browser
    if ui.button("🎨 From Preset...").clicked() {
        workspace.open_floating_panel(PanelType::PresetBrowser);
    }

    // NEW: User library
    if ui.button("📚 My Fractals...").clicked() {
        workspace.open_floating_panel(PanelType::UserLibrary);
    }

    ui.separator();

    // ... rest of menu
});
```

### Window Menu

```rust
// Add to Window menu for panel visibility
let preset_browser_open = workspace.panel_exists(PanelType::PresetBrowser);
if ui.selectable_label(preset_browser_open, "🎨 Preset Browser").clicked() {
    workspace.open_floating_panel(PanelType::PresetBrowser);
}

let user_library_open = workspace.panel_exists(PanelType::UserLibrary);
if ui.selectable_label(user_library_open, "📚 User Library").clicked() {
    workspace.open_floating_panel(PanelType::UserLibrary);
}
```

## Workspace Panel Types

```rust
// src/ui/workspace.rs
pub enum PanelType {
    // ... existing variants ...
    PresetBrowser,   // Browse built-in presets
    UserLibrary,     // User-saved fractals
    BackupBrowser,   // Auto-saved backups
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing cases ...
            PanelType::PresetBrowser => write!(f, "Preset Browser"),
            PanelType::UserLibrary => write!(f, "My Fractals"),
            PanelType::BackupBrowser => write!(f, "Backups"),
        }
    }
}
```

## File Structure

```
src/
├── renderer/
│   ├── compute_kernel.rs     # Main FlameRenderer
│   └── thumbnail.rs          # NEW: ThumbnailRenderer (runtime generation)
├── ui/
│   ├── mod.rs                # Module declarations
│   ├── workspace.rs          # Panel types, layouts
│   ├── menu_bar.rs           # Menu integration
│   ├── fractal_gallery.rs    # NEW: Reusable gallery widget
│   ├── preset_browser.rs     # NEW: Preset browser panel
│   ├── user_library.rs       # NEW: User fractal library
│   ├── backup_browser.rs     # NEW: Backup browser
│   └── ... (existing panels)

assets/presets/
├── simple.fflame
├── spherical.fflame
├── spiral.fflame
└── ...
# Note: No thumbnails/ directory - generated at runtime
```

## Runtime Thumbnail Generation

**How it works:**
1. User opens Preset Browser panel for first time
2. Panel requests thumbnails for visible presets
3. `ThumbnailRenderer` creates headless GPU renderer
4. Each preset rendered to 512×512 @ 200M iterations
5. Texture uploaded to GPU and cached in egui memory
6. Subsequent opens use cached textures (instant)

**Performance:**
- First open: 5-20 seconds for 10 presets (one-time cost)
- Loading indicator: Spinner shown during generation
- Lazy loading: Only render visible thumbnails
- Session cache: Textures persist until app closes
- Future: Optional disk cache for persistence

**When adding new presets:**
1. Add `.fflame` file to `assets/presets/`
2. Restart app to load new preset
3. Thumbnail generated automatically on first view
4. No manual scripts or PNG files needed

## Migration Path

### Phase 1: Parallel Systems
- Keep existing dropdown in Rendering panel (lines 30-56 of `settings.rs`)
- Add new Preset Browser panel
- Both work simultaneously
- Users can choose preferred method

### Phase 2: User Testing
- Gather feedback on gallery UX
- Measure performance (texture loading, scrolling)
- Refine thumbnail quality/size trade-offs

### Phase 3: Deprecation
- Remove dropdown from Rendering panel
- Only use Preset Browser
- Cleaner UI, less code duplication

### Phase 4: Expansion
- Add User Library panel for saved fractals
- Add Backup Browser for auto-saves
- Add fractal pack import/export

## Future Enhancements

### Search & Filter
- **Text search:** Filter by name (case-insensitive)
- **Tag system:** Filter by variation types, render mode, complexity
- **Sort options:** Name, Date, Transform count, Popularity

### Thumbnail Features
- **Hover preview:** Show full-size (512×512) image on hover
- **Animation:** Show brief animation (3-5 frames) on hover
- **Metadata overlay:** Show transform count, variation types as badge

### User Library Features
- **Save current:** Add button to save current config to library
- **Organize:** Folders, tags, favorites, collections
- **Export/Import:** Share fractal libraries with others
- **Cloud sync:** Optional sync across devices (future)

### Fractal Packs
- **Pack format:** JSON array of FractalConfigs + metadata
- **Pack management:** Enable/disable packs like palettes
- **Community packs:** Download and install curated collections
- **Pack metadata:** Author, description, version, thumbnail

### Advanced Gallery Features
- **Multi-select:** Select multiple configs for batch operations
- **Comparison mode:** Side-by-side view of 2-4 fractals
- **Drag & drop:** Reorder, organize, drop into panels
- **Keyboard navigation:** Arrow keys, Enter to load, Delete to remove

## Performance Considerations

### Texture Memory
- **512×512 RGBA:** ~1 MB per texture
- **10 presets:** ~10 MB total (acceptable)
- **100 presets:** ~100 MB (may need virtual scrolling)
- **Mitigation:** Lazy load textures on-demand, unload off-screen textures

### Scrolling Performance
- **egui optimization:** Built-in scroll area is efficient
- **Texture caching:** Reuse uploaded GPU textures
- **Virtual scrolling:** Only render visible items (future enhancement)

### Thumbnail Generation
- **Runtime rendering:** Generated on-demand, one-time cost per session
- **Quality vs size:** 512×512 @ 200M iterations is good balance
- **Caching:** Textures persist in egui memory for session lifetime
- **Future:** Optional disk cache to persist between sessions

## egui View Mode Support

**Built-in support:**
- egui has `Grid` for grid layouts ✅
- egui has `CollapsingHeader` for list details ✅
- No built-in "view mode switcher" widget, but easy to implement:

```rust
ui.horizontal(|ui| {
    if ui.selectable_label(view_mode == GalleryViewMode::Grid, "🔲 Grid").clicked() {
        view_mode = GalleryViewMode::Grid;
    }
    if ui.selectable_label(view_mode == GalleryViewMode::List, "≡ List").clicked() {
        view_mode = GalleryViewMode::List;
    }
});
```

## Open Questions

1. **Should gallery state persist across sessions?**
   - View mode, thumbnail size, sort order
   - Store in egui memory or config file?

2. **Should we support drag & drop between galleries?**
   - E.g., drag preset to user library to save
   - Requires egui drag-and-drop system

3. **Thumbnail quality vs performance trade-off?**
   - Start with 512×512 @ 200M
   - Monitor generation time and file sizes
   - Reduce if needed (256×256 or 100M iterations)

4. **Should List view show thumbnails?**
   - Small thumbnail (64×64) in collapsed row?
   - Or only in expanded details?

5. **How to handle user library persistence?**
   - Save as individual `.fflame` files in user directory?
   - Single JSON array file with all saved fractals?
   - SQLite database for advanced queries?

## Implementation Priority

### Phase 1: Core Gallery Widget (Week 1)
1. Create `fractal_gallery.rs` with basic grid view
2. Implement texture loading and caching
3. Add responsive column calculation
4. Handle click selection

### Phase 2: Thumbnail Renderer (Week 1)
1. Create `ThumbnailRenderer` struct in `src/renderer/thumbnail.rs`
2. Implement `render_thumbnail()` using headless export code
3. Add GPU resource sharing (Arc<Device>, Arc<Queue>)
4. Test rendering speed and quality
5. Integrate with `FractalConfigGallery`

### Phase 3: Preset Browser Integration (Week 1)
1. Add `PanelType::PresetBrowser` to workspace
2. Create `preset_browser.rs` wrapper
3. Add "From Preset..." menu item
4. Implement loading states and spinners
5. Test with existing presets

### Phase 4: List View & Polish (Week 2)
1. Implement list view in gallery
2. Add view mode switcher
3. Add search/filter functionality
4. Polish hover effects and animations

### Phase 5: Additional Panels (Week 2-3)
1. User Library panel
2. Backup Browser panel
3. Fractal pack system
4. Import/export functionality

### Phase 6: Advanced Features (Future)
1. Async thumbnail generation (non-blocking)
2. Disk cache for thumbnails (persist between sessions)
3. Advanced search and filtering
4. Hover previews and animations
5. Multi-select and batch operations
6. Cloud sync and sharing

---

**Next Steps:**
1. Review and approve design
2. Implement `ThumbnailRenderer` for runtime generation
3. Create `FractalConfigGallery` widget
4. Integrate Preset Browser panel
5. Test rendering performance and quality
6. Iterate on UX and optimization
