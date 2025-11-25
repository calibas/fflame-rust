# Configuration Versioning System

## Problem

As the application evolves, config file formats will change:
- New parameters added (easy: use `#[serde(default)]`)
- Parameters renamed (hard: need migration)
- Parameters removed (hard: need cleanup)
- Data structure changes (very hard: need complex migration)

Without versioning, we can't:
- Know which format a file uses
- Safely migrate old configs to new format
- Maintain backward compatibility
- Detect incompatible future formats

## Solution

Add version field to all serialized structures with migration support.

## Version Numbering

Use **simple integer versioning** (not semver):
- Starts at `1` for first versioned release
- Increments by `1` for each breaking change
- Simple to compare, easy to implement migration chains

Example:
```
Version 1: Initial release
Version 2: Added histogram_color_scale parameter
Version 3: Renamed speed_multiplier to iterations_per_thread
Version 4: Changed palette format from stops to indexed_colors
```

## Implementation

### 1. FractalConfig Versioning

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    /// Config format version (for migration/backward compatibility)
    #[serde(default = "default_config_version")]
    pub version: u32,

    // ... rest of fields
}

fn default_config_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

pub const CURRENT_CONFIG_VERSION: u32 = 1;
```

### 2. Deserialization with Migration

```rust
impl FractalConfig {
    /// Load from JSON with automatic migration
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut config: Self = serde_json::from_str(json)?;

        // Migrate if old version
        if config.version < CURRENT_CONFIG_VERSION {
            config = Self::migrate(config)?;
        }

        Ok(config)
    }

    /// Migrate old config to current version
    fn migrate(mut config: Self) -> Result<Self, String> {
        let original_version = config.version;

        // Migration chain: apply each migration in sequence
        while config.version < CURRENT_CONFIG_VERSION {
            config = match config.version {
                1 => Self::migrate_v1_to_v2(config)?,
                2 => Self::migrate_v2_to_v3(config)?,
                3 => Self::migrate_v3_to_v4(config)?,
                // Add new migrations here
                _ => return Err(format!("Unknown version {}", config.version)),
            };
        }

        log::info!("Migrated config from version {} to {}",
            original_version, CURRENT_CONFIG_VERSION);

        Ok(config)
    }

    // Example migration
    fn migrate_v1_to_v2(mut config: Self) -> Result<Self, String> {
        // v2 added histogram_color_scale parameter
        // serde(default) handles this automatically, just bump version
        config.version = 2;
        Ok(config)
    }

    fn migrate_v2_to_v3(mut config: Self) -> Result<Self, String> {
        // v3 renamed speed_multiplier to iterations_per_thread
        // This would require custom deserialization to handle old field name
        config.version = 3;
        Ok(config)
    }
}
```

### 3. Serialization (Always Current Version)

```rust
impl FractalConfig {
    /// Export to JSON (always writes current version)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut config = self.clone();
        config.version = CURRENT_CONFIG_VERSION;
        serde_json::to_string_pretty(&config)
    }
}
```

### 4. Local Storage Versioning

Each storage category gets its own version:

```rust
// System Settings
#[derive(Serialize, Deserialize)]
pub struct SystemSettings {
    #[serde(default = "default_settings_version")]
    pub version: u32,
    pub vsync_enabled: bool,
    pub target_fps: f32,
    // ...
}

const CURRENT_SETTINGS_VERSION: u32 = 1;

// Workspace Layout
#[derive(Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub metadata: WorkspaceMetadata,
    #[serde(default = "default_workspace_version")]
    pub version: u32,
    pub layout: egui_dock::DockState<PanelKind>,
}

const CURRENT_WORKSPACE_VERSION: u32 = 1;
```

### 5. Future Format Detection

Detect configs from future versions:

```rust
impl FractalConfig {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(json)
            .map_err(|e| format!("Parse error: {}", e))?;

        if config.version > CURRENT_CONFIG_VERSION {
            return Err(format!(
                "Config version {} is newer than supported version {}. \
                 Please update the application.",
                config.version, CURRENT_CONFIG_VERSION
            ));
        }

        // ... migration logic
    }
}
```

## Migration Examples

### Adding a New Parameter (Easy)

```rust
// Version 5: Added new parameter
pub struct FractalConfig {
    #[serde(default = "default_config_version")]
    pub version: u32,

    // New parameter with default
    #[serde(default = "default_new_param")]
    pub new_param: f32,
}

fn default_new_param() -> f32 {
    1.0
}

// Migration: just bump version (serde handles the rest)
fn migrate_v4_to_v5(mut config: Self) -> Result<Self, String> {
    config.version = 5;
    Ok(config)
}
```

### Renaming a Parameter (Medium)

```rust
// Custom deserializer to handle old field name
#[derive(Deserialize)]
#[serde(untagged)]
enum IterationsField {
    New { iterations_per_thread: u32 },
    Old { speed_multiplier: u32 },  // Old name
}

// Migration
fn migrate_v2_to_v3(mut config: Self) -> Result<Self, String> {
    // If old field exists, rename it
    // (This requires custom deserialization or JSON manipulation)
    config.version = 3;
    Ok(config)
}
```

### Complex Data Structure Change (Hard)

```rust
// Example: Palette format changed from stops to indexed_colors
fn migrate_v3_to_v4(mut config: Self) -> Result<Self, String> {
    if let Some(ref mut palette) = config.palette {
        // Convert old stops format to new indexed format
        if palette.is_indexed_256() {
            // Already in new format or can be converted
        }
    }
    config.version = 4;
    Ok(config)
}
```

## Version History Tracking

Keep a changelog in code:

```rust
// src/config/versions.rs

/// Current config version
pub const CURRENT_CONFIG_VERSION: u32 = 4;

/// Version history (for reference and migration logic)
pub const VERSION_HISTORY: &[&str] = &[
    "1: Initial versioned release",
    "2: Added histogram_color_scale parameter",
    "3: Renamed speed_multiplier to iterations_per_thread",
    "4: Changed palette format (indexed_colors support)",
];
```

## Benefits

1. **Safe Migration**: Old files automatically upgrade to new format
2. **Future Proofing**: Detect incompatible future versions
3. **Debugging**: Know exactly which format a file uses
4. **Confidence**: Can make breaking changes without fear
5. **User Experience**: Seamless upgrades, no manual migration
6. **Smaller Files**: Omit default values during serialization (see below)

## Compact Serialization (Omitting Defaults)

With versioning, defaults are implied by the version number. We can skip writing fields that match their defaults, significantly reducing file size.

### Implementation

```rust
impl FractalConfig {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        // Serialize to Value first
        let mut value = serde_json::to_value(self)?;

        // Remove fields that match defaults
        if let Some(obj) = value.as_object_mut() {
            let defaults = Self::default();

            // Only keep non-default values
            if self.zoom == defaults.zoom { obj.remove("zoom"); }
            if self.pan_x == defaults.pan_x { obj.remove("pan_x"); }
            if self.pan_y == defaults.pan_y { obj.remove("pan_y"); }
            if self.exposure == defaults.exposure { obj.remove("exposure"); }
            if self.gamma == defaults.gamma { obj.remove("gamma"); }
            // ... etc for all defaultable fields

            // Always keep: version, flame (required)
        }

        serde_json::to_string_pretty(&value)
    }
}
```

### Alternative: serde `skip_serializing_if`

```rust
#[derive(Serialize, Deserialize)]
pub struct FractalConfig {
    pub version: u32,
    pub flame: Flame,  // Always required

    #[serde(default = "default_zoom", skip_serializing_if = "is_default_zoom")]
    pub zoom: f32,

    #[serde(default = "default_exposure", skip_serializing_if = "is_default_exposure")]
    pub exposure: f32,

    // ...
}

fn is_default_zoom(v: &f32) -> bool { *v == 1.0 }
fn is_default_exposure(v: &f32) -> bool { *v == 1.0 }
```

### Size Comparison

Typical FractalConfig with all fields:
```json
{
  "version": 1,
  "flame": { ... },
  "zoom": 1.0,
  "pan_x": 0.0,
  "pan_y": 0.0,
  "rotation": 0.0,
  "camera_rotation_x": 0.0,
  "camera_rotation_y": 0.0,
  "density_scale": 1.0,
  "exposure": 1.0,
  "gamma": 2.2,
  ...
}
// ~2-3 KB
```

With defaults omitted:
```json
{
  "version": 1,
  "flame": { ... }
}
// ~500 bytes (for simple flames)
```

### Benefits
- **Smaller files**: 50-80% reduction for configs using mostly defaults
- **Cleaner diffs**: Only changed values show up in version control
- **Faster parsing**: Less JSON to parse
- **Clearer intent**: Non-default values stand out as intentional customizations

### ⚠️ Caution: Changing Default Values

If a default value changes in a future version, migration must explicitly set the old default for existing configs. Otherwise, omitted fields will silently get the new default.

**Example:** If `gamma` default changes from `2.2` to `2.4`:
```rust
fn migrate_v5_to_v6(mut config: Self) -> Result<Self, String> {
    // v6 changed gamma default from 2.2 to 2.4
    // Configs that omitted gamma were using 2.2, so preserve that
    if config.gamma == 2.4 {  // New default was applied during deserialize
        config.gamma = 2.2;    // Restore old default
    }
    config.version = 6;
    Ok(config)
}
```

**Best practice:** Avoid changing defaults when possible. If unavoidable:
1. Bump version
2. Add migration that preserves old default for existing configs
3. Document the change in VERSION_HISTORY

**Default values are defined in:** `src/config/defaults.rs`

## When to Bump Version

**Bump version when**:
- Renaming fields (breaking change)
- Removing fields (breaking change)
- Changing field types (breaking change)
- Changing data structure (breaking change)

**Don't bump version when**:
- Adding fields with `#[serde(default)]` (backward compatible)
- Fixing bugs (not a format change)
- Changing internal logic (not a format change)

## Implementation Order

1. ✅ **Phase 1**: Add version to FractalConfig (this PR)
   - Add `version` field with default
   - Implement migration framework
   - Compact serialization (omit default values)
   - Test with current version (no migrations yet)

2. ✅ **Phase 2**: Add versioning to local storage structures
   - ✅ SystemSettings version (compact serialization, migration chain)
   - WorkspaceEntry version (when workspace persistence is added)
   - Custom fractal/palette metadata versions (when needed)

**Phases 3 & 4 (Deferred):** Originally planned for migration traits and retroactive fixes, but not needed:
- Migration methods work fine inline on each struct
- No past breaking changes exist (new project)
- Tests already cover version handling

## Testing Strategy

```rust
#[test]
fn test_version_migration() {
    // Old v1 format (before histogram_color_scale existed)
    let old_json = r#"{
        "version": 1,
        "flame": { ... },
        "zoom": 1.0
    }"#;

    let config = FractalConfig::from_json(old_json).unwrap();
    assert_eq!(config.version, CURRENT_CONFIG_VERSION);
    assert_eq!(config.histogram_color_scale, DEFAULT_HISTOGRAM_COLOR_SCALE);
}

#[test]
fn test_future_version_rejection() {
    let future_json = r#"{"version": 999, ...}"#;
    let result = FractalConfig::from_json(future_json);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("newer than supported"));
}
```

## Files to Modify

- `src/config/fractal_config.rs` - Add version field and migration
- `src/config/versions.rs` - New file for version constants and history
- `src/config/migration.rs` - New file for migration logic
- `docs/projects/local-storage-system.md` - Update with versioning strategy

## Notes

- **Don't version internal runtime state** - only serialized configs
- **Version is always written** - even if it matches default
- **Migration is one-way** - no downgrading to old versions
- **Log all migrations** - helps debug user issues
