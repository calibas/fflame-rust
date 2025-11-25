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
3. **Visual Previews:** High-quality 512x512 thumbnails scaled in UI
4. **Performance:** Hash-based disk cache, lazy generation, smooth scrolling
5. **Responsive:** Adapts to window size, mobile-friendly layout
6. **Professional:** Modern asset browser UX (Unity/Blender style)

## User Interface

### View Modes

#### **Grid View** (Default)
```
+---------------------------------------------------+
|  Preset Library                    [=] [Grid] [X] |
+---------------------------------------------------+
|  Search: [_______________]                        |
+---------------------------------------------------+
|  +----------------------------------------------+ |
|  |                                              | |
|  |   +----+  +----+  +----+  +----+             | |
|  |   |    |  |    |  |    |  |    |             | |  128x128 thumbnails
|  |   | P1 |  | P2 |  | P3 |  | P4 |             | |  (scaled from 512x512)
|  |   |    |  |    |  |    |  |    |             | |
|  |   +----+  +----+  +----+  +----+             | |
|  |   Simple  Sphere  Spiral  Julia              | |
|  |                                              | |
|  |   +----+  +----+  +----+  +----+             | |
|  |   |    |  |    |  |    |  |    |             | |
|  |   | P5 |  | P6 |  | P7 |  | P8 |             | |
|  |   |    |  |    |  |    |  |    |             | |
|  |   +----+  +----+  +----+  +----+             | |
|  |   Complex Flower  3D      JDisc              | |
|  |                                              | |
|  +----------------------------------------------+ |
+---------------------------------------------------+
```

#### **List View** (Details)
```
+---------------------------------------------------+
|  Preset Library                    [=] [List] [X] |
+---------------------------------------------------+
|  Search: [_______________]                        |
+---------------------------------------------------+
|  +----------------------------------------------+ |
|  | [>] Simple                                   | |
|  |     2 transforms, 2D, Linear+Sinusoidal     | |
|  |                                              | |
|  | [>] Spherical                               | |
|  |     2 transforms, 2D, Spherical             | |
|  |                                              | |
|  | [>] Spiral                                  | |
|  |     2 transforms, 2D, Spiral+Linear         | |
|  |                                              | |
|  | [>] Julia                                   | |
|  |     1 transform, 2D, Julia                  | |
|  |                                              | |
|  | [>] Complex                                 | |
|  |     4 transforms, 2D, Multi-variation       | |
|  |                                              | |
|  +----------------------------------------------+ |
+---------------------------------------------------+
```

### Progress UI During Thumbnail Generation
```
+---------------------------------------+
|  Generating Thumbnails...             |
|                                       |
|  ############............  5/12      |
|                                       |
|  Rendering "Spiral"...                |
+---------------------------------------+
```

### Controls

**Toolbar:**
- **View Mode Toggle:** Grid <-> List (button or icon)
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
    /// Configs to display (with unique IDs for cache lookup)
    configs: Vec<GalleryItem>,

    /// Current view mode
    view_mode: GalleryViewMode,

    /// Search filter
    search_query: String,

    /// Thumbnail size (grid view)
    thumbnail_size: f32,

    /// Texture cache (hash -> TextureHandle)
    texture_cache: HashMap<String, egui::TextureHandle>,

    /// Thumbnail disk cache
    disk_cache: ThumbnailCache,

    /// Pending thumbnail generation queue
    pending_thumbnails: VecDeque<usize>,

    /// Currently generating (for progress UI)
    generating_index: Option<usize>,

    /// Selected config index
    selected_index: Option<usize>,
}

pub struct GalleryItem {
    pub config: FractalConfig,
    pub hash: String,  // SHA256 of config JSON for cache lookup
}

pub enum GalleryViewMode {
    Grid,   // Thumbnail grid
    List,   // Detailed list
}

impl FractalConfigGallery {
    pub fn new(configs: Vec<FractalConfig>, cache: ThumbnailCache) -> Self;

    /// Add configs (e.g., when loading a new library file)
    pub fn add_configs(&mut self, configs: Vec<FractalConfig>);

    /// Render gallery UI, returns selected config if clicked
    /// Also handles one-per-frame thumbnail generation
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut FlameRenderer,
    ) -> GalleryResponse;

    /// Check if thumbnail generation is in progress
    pub fn is_generating(&self) -> bool;

    /// Get generation progress (current, total)
    pub fn generation_progress(&self) -> Option<(usize, usize)>;
}

pub struct GalleryResponse {
    /// Config was selected (clicked)
    pub selected: Option<FractalConfig>,

    /// Request to close the panel (e.g., after selection)
    pub close_requested: bool,
}
```

### Thumbnail Disk Cache

**Purpose:** Persist thumbnails across sessions using hash-based filenames

**Storage Location:**
- Desktop: `{user_data_dir}/FractalFlame/cache/thumbnails/{hash}.png`
- WASM: Memory cache only (regenerate each session)

**Architecture:**
```rust
pub struct ThumbnailCache {
    /// Cache directory path
    cache_dir: PathBuf,

    /// In-memory index of known cached hashes
    cached_hashes: HashSet<String>,
}

impl ThumbnailCache {
    /// Create cache, scan existing thumbnails
    pub fn new() -> Self {
        let cache_dir = get_cache_dir().join("thumbnails");
        std::fs::create_dir_all(&cache_dir).ok();

        // Scan existing files to build index
        let cached_hashes = Self::scan_cache_dir(&cache_dir);

        Self { cache_dir, cached_hashes }
    }

    /// Check if thumbnail exists in cache
    pub fn exists(&self, hash: &str) -> bool {
        self.cached_hashes.contains(hash)
    }

    /// Load thumbnail from disk
    pub fn load(&self, hash: &str) -> Option<image::RgbaImage> {
        let path = self.cache_dir.join(format!("{}.png", hash));
        image::open(&path).ok()?.into_rgba8().into()
    }

    /// Save thumbnail to disk
    pub fn save(&mut self, hash: &str, image: &image::RgbaImage) -> anyhow::Result<()> {
        let path = self.cache_dir.join(format!("{}.png", hash));
        image.save(&path)?;
        self.cached_hashes.insert(hash.to_string());
        Ok(())
    }

    /// Generate hash from FractalConfig
    pub fn config_hash(config: &FractalConfig) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Serialize to JSON for consistent hashing
        let json = serde_json::to_string(config).unwrap_or_default();

        // Use DefaultHasher (SipHash) - fast, no external dependency
        // Not cryptographically secure, but fine for local cache keys
        let mut hasher = DefaultHasher::new();
        json.hash(&mut hasher);
        format!("{:016x}", hasher.finish())  // 16-char hex string
    }
}

fn get_cache_dir() -> PathBuf {
    // Same as SystemSettings storage location
    directories::ProjectDirs::from("", "", "FractalFlame")
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".cache"))
}
```

### One-Per-Frame Thumbnail Generation

**Approach:** Render one thumbnail per frame to allow UI progress updates

**Flow:**
```rust
impl FractalConfigGallery {
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut FlameRenderer,
    ) -> GalleryResponse {
        // Step 1: Check for missing thumbnails and queue them
        self.queue_missing_thumbnails();

        // Step 2: If generating, show progress modal and render one thumbnail
        if let Some(index) = self.pending_thumbnails.front().copied() {
            self.show_generation_progress(ui, index);
            self.generate_one_thumbnail(renderer, index);
            return GalleryResponse::default(); // Block interaction during generation
        }

        // Step 3: Normal gallery rendering
        match self.view_mode {
            GalleryViewMode::Grid => self.render_grid(ui),
            GalleryViewMode::List => self.render_list(ui),
        }
    }

    fn queue_missing_thumbnails(&mut self) {
        for (index, item) in self.configs.iter().enumerate() {
            if !self.texture_cache.contains_key(&item.hash)
               && !self.disk_cache.exists(&item.hash)
               && !self.pending_thumbnails.contains(&index)
            {
                self.pending_thumbnails.push_back(index);
            }
        }
    }

    fn generate_one_thumbnail(&mut self, renderer: &mut FlameRenderer, index: usize) {
        let item = &self.configs[index];

        // Check disk cache first (might have been generated in previous session)
        if let Some(image) = self.disk_cache.load(&item.hash) {
            self.upload_texture(ui.ctx(), &item.hash, image);
            self.pending_thumbnails.pop_front();
            return;
        }

        // Render thumbnail using existing headless export code
        let image = render_thumbnail(&item.config, renderer);

        // Save to disk cache
        self.disk_cache.save(&item.hash, &image).ok();

        // Upload to GPU texture
        self.upload_texture(ui.ctx(), &item.hash, image);

        // Remove from queue
        self.pending_thumbnails.pop_front();
    }

    fn show_generation_progress(&self, ui: &mut egui::Ui, current_index: usize) {
        let total = self.pending_thumbnails.len() +
                    self.configs.len() - self.pending_thumbnails.len();
        let completed = self.configs.len() - self.pending_thumbnails.len();
        let config_name = &self.configs[current_index].config.flame.name;

        egui::Window::new("Generating Thumbnails...")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.add(egui::ProgressBar::new(completed as f32 / total as f32)
                    .text(format!("{}/{}", completed, total)));
                ui.label(format!("Rendering \"{}\"...", config_name));
            });
    }
}

/// Render a single thumbnail (blocking, ~1-2 seconds)
fn render_thumbnail(config: &FractalConfig, renderer: &mut FlameRenderer) -> image::RgbaImage {
    const THUMBNAIL_SIZE: u32 = 512;
    const THUMBNAIL_ITERATIONS: u64 = 50_000_000;  // 50M for fast generation

    // Use headless export with modified settings
    let mut export_config = config.clone();
    export_config.max_iterations = THUMBNAIL_ITERATIONS;

    // Render using existing headless export infrastructure
    renderer.export_headless(&export_config, THUMBNAIL_SIZE, THUMBNAIL_SIZE)
}
```

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
fn render_grid(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
    let mut response = GalleryResponse::default();
    let panel_width = ui.available_width();
    let columns = self.calculate_columns(panel_width);

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("fractal_gallery_grid")
            .num_columns(columns)
            .spacing([16.0, 24.0])  // h_spacing, v_spacing
            .striped(false)
            .show(ui, |ui| {
                let filtered = self.filtered_configs();

                for (item_index, (original_index, item)) in filtered.iter().enumerate() {
                    // Get thumbnail texture
                    let texture = self.texture_cache.get(&item.hash);

                    // Allocate space for card
                    let card_size = egui::vec2(self.thumbnail_size, self.thumbnail_size + 24.0);
                    let (rect, click_response) = ui.allocate_exact_size(card_size, egui::Sense::click());

                    // Hover highlight
                    if click_response.hovered() {
                        ui.painter().rect_filled(
                            rect.expand(4.0),
                            4.0,
                            ui.visuals().widgets.hovered.bg_fill,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Render thumbnail or placeholder
                    let img_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(self.thumbnail_size, self.thumbnail_size),
                    );

                    if let Some(texture) = texture {
                        ui.put(img_rect, egui::Image::new(texture)
                            .fit_to_exact_size(egui::vec2(self.thumbnail_size, self.thumbnail_size)));
                    } else {
                        // Placeholder (should rarely happen with disk cache)
                        ui.painter().rect_filled(img_rect, 4.0, egui::Color32::DARK_GRAY);
                    }

                    // Render name below
                    let name_rect = egui::Rect::from_min_size(
                        img_rect.min + egui::vec2(0.0, self.thumbnail_size + 4.0),
                        egui::vec2(self.thumbnail_size, 20.0),
                    );
                    ui.painter().text(
                        name_rect.center_top() + egui::vec2(0.0, 2.0),
                        egui::Align2::CENTER_TOP,
                        &item.config.flame.name,
                        egui::FontId::default(),
                        ui.visuals().text_color(),
                    );

                    // Handle click
                    if click_response.clicked() {
                        response.selected = Some(item.config.clone());
                    }

                    // Move to next row after filling columns
                    if (item_index + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });

    response
}
```

### List Layout

**egui Implementation:**
```rust
fn render_list(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
    let mut response = GalleryResponse::default();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let filtered = self.filtered_configs();

        for (original_index, item) in filtered {
            // Collapsing header for each item
            let header_id = ui.make_persistent_id(format!("config_list_{}", original_index));

            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                false,  // Default collapsed
            )
            .show_header(ui, |ui| {
                // Row with name and summary
                ui.horizontal(|ui| {
                    // Play icon (clickable)
                    if ui.button(">").clicked() {
                        response.selected = Some(item.config.clone());
                    }

                    ui.strong(&item.config.flame.name);
                });

                // Summary line
                ui.label(format!(
                    "{} transforms, {}, {}",
                    item.config.flame.transforms.len(),
                    match item.config.flame.render_mode {
                        RenderMode::TwoD => "2D",
                        RenderMode::ThreeD => "3D",
                    },
                    self.format_variations(&item.config),
                ));
            })
            .body(|ui| {
                // Expanded details
                ui.label(format!("Zoom: {:.2}", item.config.zoom));
                ui.label(format!("Max Iterations: {}", item.config.max_iterations));
                ui.label(format!("Color Mode: {:?}", item.config.color_mode));

                // Show small thumbnail in expanded view
                if let Some(texture) = self.texture_cache.get(&item.hash) {
                    ui.image(texture).fit_to_exact_size(egui::vec2(64.0, 64.0));
                }
            });

            ui.separator();
        }
    });

    response
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

## Multi-Config .fflame Files

**Extended Format:** `.fflame` files can now hold single configs or arrays

```json
// Single config (current format - still supported)
{ "flame": {...}, "zoom": 1.0, ... }

// Multi-config (new library format)
[
  { "flame": {...}, "zoom": 1.0, ... },
  { "flame": {...}, "zoom": 2.0, ... }
]
```

**Loading Logic:**
```rust
pub fn load_fflame_file(path: &Path) -> anyhow::Result<Vec<FractalConfig>> {
    let contents = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&contents)?;

    match value {
        // Array of configs
        serde_json::Value::Array(arr) => {
            arr.into_iter()
                .map(|v| serde_json::from_value(v))
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        }
        // Single config (wrap in vec)
        _ => {
            let config: FractalConfig = serde_json::from_value(value)?;
            Ok(vec![config])
        }
    }
}
```

## Panel Integration

### 1. Preset Library Panel

**Location:** `src/ui/preset_library.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryViewMode, GalleryResponse};
use crate::scene::presets::PresetLibrary;

pub struct PresetLibraryPanel {
    gallery: FractalConfigGallery,
}

impl PresetLibraryPanel {
    pub fn new(library: &PresetLibrary, cache: ThumbnailCache) -> Self {
        Self {
            gallery: FractalConfigGallery::new(library.presets().to_vec(), cache),
        }
    }

    /// Load additional presets from file (e.g., user-provided .fflame)
    pub fn load_library_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let configs = load_fflame_file(path)?;
        self.gallery.add_configs(configs);
        Ok(())
    }

    /// Render panel, returns selected preset
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut FlameRenderer,
    ) -> GalleryResponse {
        self.gallery.render(ui, renderer)
    }
}
```

### 2. User Fractal Library Panel

**Location:** `src/ui/user_library.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;

pub struct UserLibraryPanel {
    gallery: FractalConfigGallery,
    library_path: PathBuf,
}

impl UserLibraryPanel {
    pub fn new(cache: ThumbnailCache) -> Self {
        let library_path = get_user_library_path();
        let configs = Self::load_user_library(&library_path);

        Self {
            gallery: FractalConfigGallery::new(configs, cache),
            library_path,
        }
    }

    fn load_user_library(path: &Path) -> Vec<FractalConfig> {
        if path.exists() {
            load_fflame_file(path).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn add_current(&mut self, config: FractalConfig) {
        self.gallery.add_configs(vec![config]);
        self.save_library();
    }

    fn save_library(&self) {
        // Save all configs as JSON array
        let json = serde_json::to_string_pretty(&self.gallery.configs()).unwrap();
        std::fs::write(&self.library_path, json).ok();
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut FlameRenderer,
        current_config: &FractalConfig,
    ) -> GalleryResponse {
        ui.horizontal(|ui| {
            if ui.button("+ Add Current").clicked() {
                self.add_current(current_config.clone());
            }
            // Future: Delete selected, export, etc.
        });

        ui.separator();

        self.gallery.render(ui, renderer)
    }
}

fn get_user_library_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "FractalFlame")
        .map(|dirs| dirs.data_dir().join("user_library.fflame"))
        .unwrap_or_else(|| PathBuf::from("user_library.fflame"))
}
```

### 3. Backup Library Panel

**Location:** `src/ui/backup_library.rs` (new file)

```rust
use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;

pub struct BackupLibraryPanel {
    gallery: FractalConfigGallery,
}

impl BackupLibraryPanel {
    pub fn new(backups: Vec<FractalConfig>, cache: ThumbnailCache) -> Self {
        Self {
            gallery: FractalConfigGallery::new(backups, cache),
        }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        renderer: &mut FlameRenderer,
    ) -> GalleryResponse {
        ui.label("Auto-saved backups from this session");
        ui.separator();

        self.gallery.render(ui, renderer)
    }
}
```

## Menu Integration

### File Menu

```rust
// src/ui/menu_bar.rs
ui.menu_button(t!("menu.file"), |ui| {
    if ui.button("Open...").clicked() {
        menu_actions.file.load_config = true;
    }

    if ui.button("Save As...").clicked() {
        menu_actions.file.save_config = true;
    }

    ui.separator();

    // NEW: Preset library
    if ui.button("From Preset...").clicked() {
        workspace.open_panel(PanelType::PresetLibrary);
    }

    // NEW: User library
    if ui.button("My Fractals...").clicked() {
        workspace.open_panel(PanelType::UserLibrary);
    }

    ui.separator();

    // ... rest of menu
});
```

### Window Menu

```rust
// Add to Window menu for panel visibility
let preset_library_open = workspace.panel_exists(PanelType::PresetLibrary);
if ui.selectable_label(preset_library_open, "Preset Library").clicked() {
    workspace.toggle_panel(PanelType::PresetLibrary);
}

let user_library_open = workspace.panel_exists(PanelType::UserLibrary);
if ui.selectable_label(user_library_open, "User Library").clicked() {
    workspace.toggle_panel(PanelType::UserLibrary);
}
```

## Workspace Panel Types

```rust
// src/ui/workspace.rs
pub enum PanelType {
    // ... existing variants ...
    PresetLibrary,   // Browse built-in presets
    UserLibrary,     // User-saved fractals
    BackupLibrary,   // Auto-saved backups
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing cases ...
            PanelType::PresetLibrary => write!(f, "Preset Library"),
            PanelType::UserLibrary => write!(f, "My Fractals"),
            PanelType::BackupLibrary => write!(f, "Backups"),
        }
    }
}
```

## File Structure

```
src/
+-- renderer/
|   +-- compute_kernel.rs     # Main FlameRenderer
|   +-- thumbnail.rs          # NEW: render_thumbnail() helper
+-- storage/
|   +-- thumbnail_cache.rs    # NEW: Hash-based disk cache
+-- ui/
|   +-- mod.rs                # Module declarations
|   +-- workspace.rs          # Panel types, layouts
|   +-- menu_bar.rs           # Menu integration
|   +-- fractal_gallery.rs    # NEW: Reusable gallery widget
|   +-- preset_library.rs     # NEW: Preset library panel
|   +-- user_library.rs       # NEW: User fractal library
|   +-- backup_library.rs     # NEW: Backup library
|   +-- ... (existing panels)

{user_data_dir}/FractalFlame/
+-- cache/
|   +-- thumbnails/           # Hash-based PNG cache
|       +-- a1b2c3d4...png
|       +-- e5f6g7h8...png
+-- user_library.fflame       # User's saved fractals (JSON array)

assets/presets/
+-- simple.fflame
+-- spherical.fflame
+-- spiral.fflame
+-- ...
```

## Thumbnail Generation Flow

**Complete Flow:**
1. User opens Preset Library panel
2. Gallery checks each config against disk cache (by hash)
3. **Cache hit:** Load PNG from disk -> upload to GPU texture -> display
4. **Cache miss:** Add to pending queue
5. If pending queue non-empty:
   - Show "Generating Thumbnails..." modal with progress bar
   - Render ONE thumbnail per frame (blocking, ~50-100ms each)
   - Save to disk cache
   - Upload to GPU texture
   - Update progress, continue next frame
6. When queue empty, show normal gallery UI

**Performance:**
- First open with 12 presets (all cache miss): ~1-2 seconds total
- Subsequent opens (all cache hit): Instant (<100ms)
- Adding new preset: ~50-100ms for single thumbnail
- Cache invalidation: Automatic via hash (config change = new hash = new thumbnail)

Note: At ~1 billion iterations/second, 50M iterations renders in ~50ms.

**When adding new presets:**
1. Add `.fflame` file to `assets/presets/` or load via UI
2. Open Preset Library panel
3. New preset detected (hash not in cache)
4. Thumbnail generated automatically
5. Cached for future sessions

## WASM Considerations

**Memory Cache Only:**
- WASM cannot use filesystem directly
- Thumbnails regenerated each session (~1-2s on first open)
- Same generation flow as desktop, just no disk persistence
- Future enhancement: IndexedDB for cross-session persistence

**Platform Abstraction:**
```rust
impl ThumbnailCache {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            // WASM: Memory-only cache
            Self {
                cache_dir: PathBuf::new(),  // Unused
                cached_hashes: HashSet::new(),
                disk_enabled: false,
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Desktop: Full disk cache
            let cache_dir = get_cache_dir().join("thumbnails");
            std::fs::create_dir_all(&cache_dir).ok();
            let cached_hashes = Self::scan_cache_dir(&cache_dir);
            Self {
                cache_dir,
                cached_hashes,
                disk_enabled: true,
            }
        }
    }

    pub fn load(&self, hash: &str) -> Option<image::RgbaImage> {
        if !self.disk_enabled {
            return None;  // WASM: Always cache miss on disk
        }
        // Desktop: Load from disk...
    }

    pub fn save(&mut self, hash: &str, image: &image::RgbaImage) -> anyhow::Result<()> {
        if !self.disk_enabled {
            return Ok(());  // WASM: No-op
        }
        // Desktop: Save to disk...
    }
}
```

## Migration Path

### Phase 1: Core Implementation
- Implement `ThumbnailCache` with hash-based storage
- Implement `FractalConfigGallery` with one-per-frame generation
- Add `PanelType::PresetLibrary` to workspace
- Keep existing dropdown (parallel systems)

### Phase 2: Polish & Testing
- Test generation performance (target <2s per thumbnail)
- Test cache hit/miss behavior
- Refine progress UI
- Add search/filter functionality

### Phase 3: Additional Panels
- User Library panel with save/load
- Backup Library panel
- Multi-config .fflame loading

### Phase 4: Deprecation
- Remove dropdown from Settings panel
- Preset Library becomes primary preset selection method

## Future Enhancements

### Search & Filter
- **Text search:** Filter by name (case-insensitive)
- **Tag system:** Filter by variation types, render mode, complexity
- **Sort options:** Name, Date, Transform count

### Thumbnail Features
- **Hover preview:** Show full-size (512x512) image on hover
- **Metadata overlay:** Show transform count, variation types as badge

### User Library Features
- **Organize:** Folders, tags, favorites
- **Export/Import:** Share libraries with others
- **Delete/rename:** Manage saved fractals

### Performance Optimizations
- **Virtual scrolling:** Only render visible thumbnails (for large libraries)
- **Parallel generation:** Multiple thumbnails simultaneously (future)
- **Progressive quality:** Quick low-res preview, then high-res upgrade

### Cache Management
- **Size limit:** Cap total cache size, evict old thumbnails
- **Manual clear:** Button to clear thumbnail cache
- **Integrity check:** Verify cached thumbnails match config hashes

## Dependencies

**No new dependencies required.**

**Existing dependencies used:**
- `image` - PNG encoding/decoding
- `directories` - User data directory paths
- `serde_json` - Config serialization for hashing
- `std::collections::hash_map::DefaultHasher` - Fast hashing (built-in)

---

**Next Steps:**
1. Implement `ThumbnailCache` in `src/storage/thumbnail_cache.rs`
2. Implement `FractalConfigGallery` in `src/ui/fractal_gallery.rs`
3. Implement `render_thumbnail()` helper
4. Add `PresetLibraryPanel` and workspace integration
5. Test thumbnail generation and caching
6. Iterate on UX
