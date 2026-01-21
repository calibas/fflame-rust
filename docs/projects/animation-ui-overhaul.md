# Animation UI Overhaul

## Overview

Complete redesign of the animation panel with a proper timeline interface, track visualization, and separated concerns for track editing and export.

## Current State

- Animation panel has inline controls mixed together
- "New Animation" button required to start animating
- Track editing is inline and cramped
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

#### Add/Edit Track Panel
- **Mode**: Add new track or Edit existing track
- **Parameter selector** (Add mode): Dropdown of animatable parameters
- **Keyframe list**: Table showing all keyframes
  - Time (editable)
  - Value (editable)
  - Easing curve selector
  - Delete keyframe button
- **Add Keyframe button**: Adds keyframe at current time with current value
- **Close/Done button**

#### Export Animation Panel
- Moved from inline controls to separate panel
- **Resolution**: Width x Height inputs
- **Frame Rate**: FPS input
- **Format**: MP4/WebM/GIF selector
- **Quality settings**
- **Output path** (desktop only)
- **Export button**
- **Progress indicator** during export

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

### Phase 1: Layout Restructure
- Reorganize animation panel layout
- Move controls to top left/right sections
- Implement wider timeline scrubber
- Remove "New Animation" button

### Phase 2: Track Visualization
- Implement track bar rendering
- Add keyframe dot visualization
- Implement red position line through all tracks
- Add hover tooltip for keyframe values

### Phase 3: Track Interaction
- Click on timeline to jump
- Drag scrubber position
- Click on keyframe dot to edit
- Track Edit/Delete buttons

### Phase 4: Add/Edit Track Panel
- Create new panel
- Parameter selector for new tracks
- Keyframe list with editing
- Easing curve selection

### Phase 5: Export Animation Panel
- Extract export controls to separate panel
- Add export button to open panel
- Progress indicator

### Phase 6: Polish
- Keyboard shortcuts (if time permits)
- Visual refinements
- Edge case handling

## Files to Modify

- `src/ui/animation_panel.rs` - Main panel overhaul
- `src/ui/track_editor.rs` - May need updates or replacement
- `src/ui/mod.rs` - Register new panels
- `src/ui/workspace.rs` - Add new panels to dock system
- `src/animation/mod.rs` - Ensure animation always exists
- `locales/en.yml` - New UI strings

## Notes

- This is primarily a UI change
- Animation system backend should already support all needed functionality
- Focus on usability and visual clarity
