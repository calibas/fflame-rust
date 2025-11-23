# Local Storage System Design

## Problem Statement

Currently, all settings (visual + system + performance) are stored in `FractalConfig`, which is designed for per-fractal configuration. This causes issues:

1. **System settings** (VSync, target FPS, iterations per thread) shouldn't be tracked per-fractal
2. **Custom user data** (palettes, fractals) has no persistent storage
3. **No auto-backup** system for fractal work
4. **No unified storage** across Windows, macOS, and WASM

## Requirements

### Cross-Platform Storage
- **Desktop (Windows/macOS)**: Local filesystem (standardized location)
- **WASM (Browser)**: `localStorage` or IndexedDB
- **Unified API**: Same code path for all platforms where possible

### Storage Categories

#### 1. System Settings (Device-Specific)
Settings that apply globally to the application on this device:

**Performance & Rendering:**
- `vsync_enabled: bool` (default: true)
- `target_fps: f32` (default: 60.0, when VSync off)
- `iterations_per_thread: u32` (default: 256) - GPU performance tuning
- `speed_multiplier: u32` (default: 1) - Frame rate multiplier
- `accumulation_batch_size: u32` (default: 4) - GPU batching

**UI/UX:**
- `language: String` (default: "en")
- `theme: Theme` (Light/Dark, future)
- `last_workspace_layout: WorkspaceLayout` (docking panel positions)
- `ui_scale: f32` (DPI scaling override, future)

**Export Defaults:**
- `default_export_width: u32` (default: 1920)
- `default_export_height: u32` (default: 1080)
- `default_export_quality: ExportQuality` (future)

**File Paths (Desktop only):**
- `last_opened_file: Option<PathBuf>`
- `recent_files: Vec<PathBuf>` (MRU list, max 10)
- `default_save_location: Option<PathBuf>`

**Total:** ~15-20 settings

#### 2. Custom Fractal Library
User-created or imported fractals saved for reuse:

**Structure:**
```json
{
  "name": "My Awesome Flame",
  "category": "Custom" | "Favorites" | "Imported",
  "created_date": "2025-11-22T12:34:56Z",
  "modified_date": "2025-11-22T13:45:00Z",
  "tags": ["3D", "cool", "work-in-progress"],
  "config": { /* FractalConfig JSON */ }
}
```

**Features:**
- Searchable by name/tags
- Sortable by date/name
- Categorization (folders/tags)
- Thumbnail preview (future: base64 PNG embedded)

**Storage Limit:**
- Desktop: Unlimited (filesystem)
- WASM: ~5-10MB total (localStorage limit), warn user when approaching

#### 3. Custom Palette Library
User-created or imported color palettes:

**Structure:**
```json
{
  "name": "Sunset Vibes",
  "category": "Custom" | "Favorites" | "Imported",
  "created_date": "2025-11-22T12:34:56Z",
  "palette": { /* Palette JSON */ }
}
```

**Features:**
- Same as fractal library (search, sort, categorize)
- Import from `.palette` files
- Export individual palettes

#### 4. Auto-Backup System
Automatic snapshots of work to prevent data loss:

**Backup Types:**
- **Auto-save**: Current fractal state every N minutes (configurable, default: 5min)
- **Session snapshots**: Save on app close, load on app open
- **Named backups**: Manual "Save Checkpoint" feature (future)

**Structure:**
```json
{
  "timestamp": "2025-11-22T13:45:00Z",
  "type": "auto-save" | "session" | "checkpoint",
  "config": { /* FractalConfig JSON */ }
}
```

**Retention:**
- Auto-saves: Keep last 10 (rolling window)
- Session: Keep last 5
- Checkpoints: User-managed (unlimited, future)

**Storage Location:**
- Desktop: `{app_data}/backups/*.fflame`
- WASM: IndexedDB (larger storage quota than localStorage)

#### 5. Application State (Session)
Ephemeral state that persists across sessions but isn't "settings":

- Current fractal (loaded or working on)
- Undo/redo history (optional: persist across sessions)
- Panel visibility states
- Last selected preset/palette indices

## Storage Architecture

### File Structure (Desktop)

**Windows:**
```
%APPDATA%\fractal_flame_wgpu\
  ├── settings.json           # System settings
  ├── custom_fractals\        # User fractal library
  │   ├── my_flame_1.fflame
  │   ├── awesome_3d.fflame
  │   └── index.json          # Metadata (name, tags, dates)
  ├── custom_palettes\        # User palette library
  │   ├── sunset.palette
  │   └── index.json
  └── backups\                # Auto-backups
      ├── auto_save_001.fflame
      ├── auto_save_002.fflame
      └── session_001.fflame
```

**macOS:**
```
~/Library/Application Support/fractal_flame_wgpu/
  ├── (same structure as Windows)
```

**Cross-platform path resolution:**
```rust
use directories::ProjectDirs;

fn app_data_dir() -> PathBuf {
    ProjectDirs::from("com", "fractalflame", "FractalFlameWGPU")
        .unwrap()
        .data_dir()
        .to_path_buf()
}
```

### Storage API (WASM)

**localStorage** (for small settings):
```javascript
localStorage.setItem('fflame_settings', JSON.stringify(settings));
```

**IndexedDB** (for larger data: fractals, backups):
```javascript
// Database: 'fractal_flame_wgpu'
// Stores:
//   - 'settings' (key-value)
//   - 'custom_fractals' (id, name, config, metadata)
//   - 'custom_palettes' (id, name, palette, metadata)
//   - 'backups' (id, timestamp, config)
```

## Implementation Plan

### Phase 1: Core Storage Infrastructure
1. Create `src/storage/mod.rs` module
2. Implement platform-agnostic `Storage` trait:
   ```rust
   pub trait Storage {
       fn save_settings(&self, settings: &SystemSettings) -> Result<()>;
       fn load_settings(&self) -> Result<SystemSettings>;
       fn save_fractal(&self, name: &str, config: &FractalConfig) -> Result<()>;
       fn load_fractal(&self, name: &str) -> Result<FractalConfig>;
       // ... etc
   }
   ```
3. Implement `FilesystemStorage` (desktop)
4. Implement `WebStorage` (WASM with localStorage + IndexedDB)

### Phase 2: Settings Migration
1. Create `SystemSettings` struct (split from FractalConfig)
2. Move performance/system settings to SystemSettings
3. Load SystemSettings on app start, save on change
4. Keep FractalConfig for visual-only settings

### Phase 3: Custom Libraries
1. Implement custom fractal save/load
2. Implement custom palette save/load
3. Add UI for browsing/managing libraries
4. Add search/filter/sort capabilities

### Phase 4: Auto-Backup
1. Implement periodic auto-save (tokio timer on desktop, setInterval on WASM)
2. Implement session save/restore
3. Add UI for backup management (restore previous state)
4. Add "Recover Lost Work" feature

### Phase 5: Polish
1. Add import/export for entire libraries
2. Add cloud sync capability (future, optional)
3. Add thumbnail generation for fractal library
4. Add migration tool for old configs

## Data Structures

### SystemSettings
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    // Performance
    pub vsync_enabled: bool,
    pub target_fps: f32,
    pub iterations_per_thread: u32,
    pub speed_multiplier: u32,
    pub accumulation_batch_size: u32,

    // UI/UX
    pub language: String,
    pub workspace_layout: Option<WorkspaceLayout>,

    // Export
    pub default_export_width: u32,
    pub default_export_height: u32,

    // File paths (desktop only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_opened_file: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recent_files: Vec<String>,
}
```

### FractalMetadata (for library)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalMetadata {
    pub id: String,  // UUID
    pub name: String,
    pub category: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub tags: Vec<String>,
    pub thumbnail: Option<String>,  // Base64 PNG, future
}
```

### LibraryEntry
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub metadata: FractalMetadata,
    pub config: FractalConfig,
}
```

## Storage Limits & Quotas

### Desktop
- Settings: ~10KB (negligible)
- Custom fractals: Unlimited (filesystem)
- Custom palettes: Unlimited
- Backups: ~1-5MB total (10 auto-saves × ~100KB each)

### WASM (Browser)
- **localStorage**: 5-10MB total (varies by browser)
  - Settings: ~10KB
  - Small data only

- **IndexedDB**: 50MB+ (quota, can request more)
  - Custom fractals: ~5-10MB (50-100 fractals @ ~100KB each)
  - Custom palettes: ~500KB (100 palettes @ ~5KB each)
  - Backups: ~1-2MB (10-20 backups)

**Warning UI:**
- Show storage usage meter
- Warn at 80% capacity
- Offer cleanup tools (delete old backups, etc.)

## Migration Strategy

### Version 1 → Version 2 (Settings Split)
```rust
fn migrate_v1_to_v2(old_config: &FractalConfigV1) -> (SystemSettings, FractalConfig) {
    let system = SystemSettings {
        vsync_enabled: old_config.vsync_enabled,
        target_fps: old_config.target_fps,
        iterations_per_thread: old_config.iterations_per_thread,
        // ... etc
        ..Default::default()
    };

    let fractal = FractalConfig {
        flame: old_config.flame.clone(),
        zoom: old_config.zoom,
        // ... only visual settings
    };

    (system, fractal)
}
```

### Detecting Version
```json
{
  "version": 2,
  "data": { /* settings */ }
}
```

## Error Handling

### Desktop
- File not found: Use defaults
- Permission denied: Show error, use in-memory only
- Corrupted JSON: Show error, backup corrupted file, use defaults

### WASM
- localStorage full: Switch to IndexedDB, show warning
- IndexedDB quota exceeded: Show cleanup UI, disable auto-save
- Browser doesn't support IndexedDB: Degrade gracefully (localStorage only)

## Security & Privacy

- No sensitive data stored (just fractal art settings)
- All data stored locally (no network transmission)
- Optional cloud sync in future (explicit user opt-in)
- Clear data button (for privacy compliance)

## Future Enhancements

1. **Cloud Sync** (optional, paid tier?)
   - Sync settings/libraries across devices
   - Requires backend service + auth

2. **Collaborative Features**
   - Share fractal links (serialize config to URL)
   - Import from URL

3. **Gallery/Community**
   - Upload/browse user-created fractals
   - Rating/commenting system

4. **Version Control**
   - Full history tracking for fractals
   - Branching/merging (like git for art)

## Dependencies

**Desktop:**
- `directories` - Cross-platform app data paths (already in deps)
- `serde_json` - JSON serialization (already in deps)
- `chrono` - Timestamps (already in deps)

**WASM:**
- `web-sys` - localStorage/IndexedDB access (already in deps)
- `wasm-bindgen` - JS interop (already in deps)
- `gloo-storage` - Higher-level storage API (consider adding)

## Testing Strategy

1. Unit tests for storage trait implementations
2. Integration tests for save/load roundtrip
3. Migration tests (v1 → v2)
4. Quota limit tests (WASM)
5. Manual testing on Windows/macOS/browsers

## Open Questions

1. **Undo/redo persistence**: Should history persist across sessions?
   - Pro: Never lose work, great UX
   - Con: Large storage footprint, complexity
   - **Decision**: Phase 2 feature, opt-in setting

2. **Fractal library sync**: Import/export entire library?
   - **Decision**: Yes, Phase 3 feature (export as ZIP)

3. **Thumbnail generation**: Auto-generate or manual?
   - **Decision**: Phase 5, auto-generate on save (async)

4. **Settings UI**: Separate "Preferences" window?
   - **Decision**: Add to existing Settings panel, new "System" section

## Success Metrics

- Settings persist across app restarts ✓
- Custom fractals/palettes saved and loadable ✓
- Auto-backup prevents data loss ✓
- Works identically on Windows/macOS/WASM ✓
- Storage usage stays under quota limits ✓

---

**Status**: Design phase
**Target**: Implement in Q1 2026
**Priority**: High (needed before 1.0 release)
