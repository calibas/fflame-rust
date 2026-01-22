# Animation UI Overhaul

## Overview

Complete redesign of the animation panel with a proper timeline interface, track visualization, and separated concerns for track editing and export.

## Status: ✅ Complete

All phases implemented as of 2026-01-21:
- **Phase 1-3**: Layout restructure, track visualization, timeline interaction
- **Phase 4-5**: Export panel separated, preview mode for scrubbing
- **Phase 6**: Removed AnimationQualityMode (didn't have meaningful effect)
- **Phase 7**: Unified Track Editor with hierarchical target selector

## Previous State (Before Overhaul)

- Animation panel had inline controls mixed together
- "New Animation" button required to start animating
- Track editing was inline and cramped
- No visual timeline representation of tracks

## Target Design

### Panel Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ TOP LEFT                              │ TOP RIGHT                       │
│ [▶][⏸][⏹] [◀][▶] Speed:[1x▼]       │ Name: [________] [Save][Load]   │
│ Duration: [10.0s] Loop: [Loop▼]       │ [Export Animation]              │
├───────────────────────────────────────┴─────────────────────────────────┤
│ TIMELINE SCRUBBER (300px+ wide)                                         │
│             |----●-------------------------------|  0.0s          10.0s │
│                  ▲ (red vertical position line)                         │
├─────────────────────────────────────────────────────────────────────────┤
│ TRACKS                                                                  |
| [+ Add Track]                                                           │
│                                                                         │
│ Zoom        |--●====●========●-------------------|     [Edit] [Delete]  │
│ Exposure    |-----------------●=================●|     [Edit] [Delete]  │
│ Rotation    |●==================================●|     [Edit] [Delete]  │
│                  ▲                                                      │
│             (red position line extends through all tracks)              │
└─────────────────────────────────────────────────────────────────────────┘
```

### Components

#### Top Left - Playback Controls
- **Play** button (▶)
- **Pause** button (⏸)
- **Stop** button (⏹) - resets to beginning
- **Step Back** button (◀) - go back 1 frame
- **Step Forward** button (▶) - go forward 1 frame
- **Playback Speed** dropdown (0.25x, 0.5x, 1x, 2x, 4x)
- **Duration** input field (seconds)
- **Loop Mode** dropdown (Once, Loop, Ping-Pong)

#### Top Right - File & Export
- **Animation Name** text field
- **Save Animation** button
- **Load Animation** button
- **Export Animation** button - opens Export Animation panel

#### Timeline Scrubber
- Minimum 300px wide
- Shows time range (0 to duration)
- Draggable position marker
- Click anywhere to jump to that time
- Red vertical line at current position
- **CRITICAL**: Timeline area must align vertically with track bars below

#### Tracks Section
Contains the Add Track button and track list.

**Add Track Button**
- Located at top of tracks section
- Opens the Add/Edit Track panel in "Add" mode

**Track List**
- Each track displayed as a row:
  - **Label**: Parameter name on left (e.g., "Zoom", "Exposure")
  - **Bar**: Horizontal bar from first keyframe time to last keyframe time (NOT full duration)
  - **Keyframe dots**: Visual indicators at each keyframe position on the bar
  - **Edit button**: Opens Add/Edit Track panel in "Edit" mode
  - **Delete button**: Removes the track (with confirmation?)
- **CRITICAL**: Track bars must align vertically with the timeline scrubber above
- Red position line extends through scrubber and all tracks at same X position
- **Hover on keyframe dot**: Shows tooltip with value
- **Click on keyframe dot**: Opens Edit Track panel with that keyframe selected

### New Panels

#### Unified Track Editor Panel (Phase 7 - Added 2026-01-21)

A single window for both adding new tracks and editing existing tracks:

```
┌─────────────────────────────────────────────────────────────────────┐
│ Track Editor (Add Track / Edit Track)                       [X]     │
├─────────────────────────────────────────────────────────────────────┤
│ Type: [Keyframe ▼]                                                  │
├─────────────────────────────────────────────────────────────────────┤
│ Target:                                                             │
│ → PanX                                                    [✕]       │
│ ▼ Change target                                                     │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │ 🔍 [Search parameters...]                                   │   │
│   │ ▼ View                                                      │   │
│   │   Zoom                                                      │   │
│   │   Pan X                                                     │   │
│   │   Pan Y                                                     │   │
│   │   Rotation                                                  │   │
│   │   ...                                                       │   │
│   │ ▶ Color                                                     │   │
│   │ ▶ Tone Mapping                                              │   │
│   │ ▶ Rendering                                                 │   │
│   │ ▶ Transform 0                                               │   │
│   │ ▶ Transform 1                                               │   │
│   │ ▶ Final Transform                                           │   │
│   └─────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│ Keyframes: 3                                                        │
│ ┌─────────────────────────────────────────────────────────────────┐ │
│ │ 0.00s   0.500   Lin  [✕]                                        │ │
│ │ 2.50s   1.200   I/O  [✕]                                        │ │
│ │ 5.00s   0.500   Lin  [✕]                                        │ │
│ └─────────────────────────────────────────────────────────────────┘ │
│ [+ Add at Current Time]  [+ Add at End]                             │
├─────────────────────────────────────────────────────────────────────┤
│ Interpolation: [Linear ▼]                                           │
├─────────────────────────────────────────────────────────────────────┤
│ [Create Track]                                         [Close]      │
└─────────────────────────────────────────────────────────────────────┘
```

**Features:**
- **Type selector**: Keyframes, Oscillator, or Circular
- **Hierarchical target selector**: Collapsible categories with search filter
  - Categories: View, Color, Tone Mapping, Rendering, Transform N, Final Transform
  - Dynamic content based on active variations and effects
- **Type-specific subpanels**:
  - **Keyframes**: Inline keyframe list with time/value/easing, interpolation mode
  - **Oscillator**: Waveform type, center, amplitude, frequency, phase
  - **Circular**: Center X/Y, radius, speed, phase (with separate Target X and Target Y)
- **Auto-create behavior**: Track appears when valid target + type are selected
- **Triggers**:
  - Add Track button → Opens panel in "Add" mode
  - Edit button on track row → Opens panel in "Edit" mode with track loaded
  - Keyframe dot click → Opens panel with that keyframe selected

#### Export Animation Panel
- Moved from inline controls to separate panel
- **Codec**: H.264, H.265, VP9 selector with hardware acceleration options
- **Resolution**: Width x Height inputs with quick presets (720p, 1080p, 4K)
- **Frame Rate**: FPS input with quick presets (24, 30, 60)
- **Quality settings**: CRF slider, encoding preset, tune options
- **Iterations per thread**: Quality vs speed tradeoff
- **Output path** (desktop only)
- **Export button**
- **Progress indicator** with ETA during export

### Behavior Changes

1. **Remove "New Animation" button**
   - Animation is always present (with 0 tracks by default)
   - Animation only affects rendering when actively playing
   - No need to "create" an animation first

2. **Track visualization**
   - Tracks visually show their time span
   - Keyframes are visible as dots on the bar
   - Easy to see animation structure at a glance

3. **Position indicator**
   - Single red vertical line through scrubber and all tracks
   - Always shows current animation time
   - Updates in real-time during playback

## Implementation Phases

### Phase 1: Layout Restructure ✅
- Reorganize animation panel layout
- Move controls to top left/right sections
- Implement wider timeline scrubber
- Remove "New Animation" button

### Phase 2: Track Visualization ✅
- Implement track bar rendering
- Add keyframe dot visualization
- Implement red position line through all tracks
- Add hover tooltip for keyframe values

### Phase 3: Track Interaction ✅
- Click on timeline/track bar to seek to time
- Drag scrubber position with preview mode
- Click on keyframe dot to edit
- Track Edit/Delete buttons

### Phase 4: Add/Edit Track Panel ✅ (Legacy)
- Original inline dialog implementation
- Replaced by unified Track Editor in Phase 7

### Phase 5: Export Animation Panel ✅
- Extract export controls to separate window
- Add codec selection (H.264, H.265, VP9)
- Hardware acceleration options (NVENC, QuickSync, AMF)
- Quality presets and encoding tuning
- Progress indicator with ETA

### Phase 6: Polish ✅
- Added 0.1x playback speed option
- Preview mode for scrubbing (seek_changed vs seek_drag_stopped)
- Removed AnimationQualityMode (didn't have meaningful effect on quality)

### Phase 7: Unified Track Editor ✅ (Added 2026-01-21)
- **Hierarchical Target Selector** (`src/ui/target_selector.rs`)
  - Reusable component for selecting ConfigPath targets
  - Collapsible categories with search filtering
  - Dynamic content based on flame configuration
  - Categories: View, Color, Tone Mapping, Rendering, Transform N, Final Transform
- **Unified Track Editor Panel** (`src/ui/track_editor.rs`)
  - Single window for Add Track and Edit Track
  - Type selector: Keyframes, Oscillator, Circular
  - Type-specific subpanels with inline editing
  - Track target can be changed anytime
- **Integration Points**
  - Add Track button → opens panel in Add mode
  - Edit button on track row → opens panel in Edit mode
  - Keyframe dot click → opens panel with keyframe selected

## Files Modified

### Phase 1-6
- `src/ui/animation_panel.rs` - Main panel layout, playback controls, timeline scrubber
- `src/ui/track_editor.rs` - Track visualization, keyframe dots, seek on click
- `src/ui/mod.rs` - Register panels, render calls
- `src/animation/mod.rs` - Animation always exists, export settings
- `locales/en.yml` - UI strings

### Phase 7
- `src/ui/target_selector.rs` - **NEW** - Hierarchical target selector component
- `src/ui/track_editor.rs` - Added unified Track Editor panel functions:
  - `render_track_editor_panel()` - Main window render
  - `render_track_editor_panel_content()` - Panel content
  - `render_keyframe_subpanel()` - Keyframe-specific UI
  - `render_oscillator_subpanel()` - Oscillator-specific UI
  - `render_circular_subpanel()` - Circular track-specific UI
  - `open_add_track_panel()` / `open_edit_track_panel()` - Entry points
- `src/ui/mod.rs` - Added render call for Track Editor panel
- `locales/en.yml` - New localization strings for Track Editor

## Notes

- Animation UI overhaul is complete
- Track target is changeable at any time (user can explore without accidental creation)
- Clicking track row still seeks to time (per user preference)
- Clicking keyframe dots opens Track Editor (not legacy keyframe editor)
- Old "Add Track Dialog" still exists for backwards compatibility but is deprecated
