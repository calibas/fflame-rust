# Overwrite Mode FSM Integration

## Status: Planned

## Overview

Move the existing overwrite/preview mode logic into the `RenderModeFSM` state machine for centralized rendering mode management.

## Current State

The FSM in `src/app/render_mode.rs` already has:
- `RenderModeState::Overwrite` variant (unused)
- `pre_overwrite_snapshot` field (unused)
- `enter_overwrite()` / `exit_overwrite()` methods (unused)
- `should_use_overwrite()` helper (unused)

Current overwrite mode logic is scattered in `src/app/mod.rs`:
- `use_overwrite_next_frame: bool` field on App
- `last_param_change_time` tracking for 100ms overwrite window
- Manual overwrite mode management in render loop

## Goal

Consolidate all overwrite mode logic into the FSM:
1. Remove `use_overwrite_next_frame` from App
2. Remove `last_param_change_time` from App
3. Have ConfigManager or UI signal FSM when parameters change
4. FSM manages overwrite timing internally
5. App queries `render_mode.should_use_overwrite()` for renderer

## Benefits

- Single source of truth for rendering mode state
- Cleaner App struct (fewer fields)
- Testable state transitions
- Consistent pattern with animation mode

## Implementation Notes

### Challenges

1. **Timing**: Overwrite mode has a 100ms window after parameter changes. FSM would need to track time or receive timing signals.

2. **Integration Point**: Need to decide where overwrite mode is triggered:
   - Option A: ConfigManager calls FSM on parameter change
   - Option B: App calls FSM after processing UI changes
   - Option C: FSM receives timestamp and manages window internally

3. **Interaction with Animation**: Animation mode takes priority over overwrite mode (already handled in FSM).

### Suggested Approach

Option B seems cleanest:
```rust
// In App render loop, after processing config changes:
if config_manager.had_changes_this_frame() {
    render_mode.enter_overwrite(current_time);
}

// FSM internally tracks:
// - Entry timestamp
// - 100ms window expiration
// - Auto-exit when window expires

// Renderer queries:
let use_overwrite = render_mode.should_use_overwrite(current_time);
```

## Dependencies

- None (FSM infrastructure already exists)

## Priority

Low - Current implementation works fine, this is a code organization improvement.
