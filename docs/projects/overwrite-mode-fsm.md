# Overwrite Mode FSM Integration

## Status: Partial - Animation mode integrated, Overwrite mode pending

## Overview

Move the existing overwrite/preview mode logic into the `RenderModeFSM` state machine for centralized rendering mode management.

## Current State

### FSM Infrastructure (Complete)

The FSM in `src/app/render_mode.rs` is fully implemented with:
- `RenderModeState` enum: `Normal`, `Animating`, `Overwrite`
- `RenderModeFSM` struct with state transitions
- `enter_animation()` / `exit_animation()` - **actively used**
- `enter_overwrite()` / `exit_overwrite()` - implemented but unused
- `should_use_overwrite()` / `should_apply_silent()` helpers
- Full test coverage (7 tests)

### Animation Mode (Integrated ✅)

Animation mode is fully integrated with the FSM:
- `AnimationController` calls `render_mode.enter_animation()` / `exit_animation()`
- Pre-animation snapshot saved for undo on exit
- `should_apply_silent()` used to suppress individual undo entries during playback

### Overwrite Mode (Not Integrated)

Current overwrite mode logic remains in `src/app/mod.rs`:
- `use_overwrite_next_frame: bool` - tracks overwrite state
- `last_param_change_time: Option<Instant>` - tracks 100ms window

This is just 2 fields managing a simple timer, so the scattered state problem is minimal.

## Goal

Consolidate overwrite mode logic into the FSM:
1. Remove `use_overwrite_next_frame` from App
2. Remove `last_param_change_time` from App
3. Have App signal FSM when parameters change
4. FSM manages overwrite timing internally
5. App queries `render_mode.should_use_overwrite()` for renderer

## Benefits

- Single source of truth for rendering mode state
- Cleaner App struct (2 fewer fields)
- Consistent pattern with animation mode
- All rendering mode logic in one testable module

## Implementation Approach

The FSM needs to manage the 100ms overwrite window internally:

```rust
// Add to RenderModeFSM
overwrite_entry_time: Option<Instant>,

pub fn enter_overwrite(&mut self, current_time: Instant) -> TransitionResult {
    match self.state {
        RenderModeState::Animating => TransitionResult::NoChange,
        RenderModeState::Overwrite => {
            // Already in overwrite, just refresh the timer
            self.overwrite_entry_time = Some(current_time);
            TransitionResult::NoChange
        }
        RenderModeState::Normal => {
            self.state = RenderModeState::Overwrite;
            self.overwrite_entry_time = Some(current_time);
            TransitionResult::Transitioned
        }
    }
}

/// Check if overwrite window has expired and auto-exit if so
pub fn update(&mut self, current_time: Instant) -> TransitionResult {
    if self.state == RenderModeState::Overwrite {
        if let Some(entry_time) = self.overwrite_entry_time {
            if current_time.duration_since(entry_time).as_millis() >= 100 {
                return self.exit_overwrite();
            }
        }
    }
    TransitionResult::NoChange
}
```

### App Integration

```rust
// In App render loop, after processing config changes:
if config_manager.had_changes_this_frame() {
    render_mode.enter_overwrite(now);
}

// Each frame, check for auto-exit
if let TransitionResult::Transitioned = render_mode.update(now) {
    // Overwrite window expired, trigger iteration counter reset
    self.reset_iteration_counter();
}

// Renderer queries:
let use_overwrite = render_mode.should_use_overwrite();
```

## Files to Modify

- `src/app/render_mode.rs` - Add timing to overwrite mode
- `src/app/mod.rs` - Remove `use_overwrite_next_frame`, `last_param_change_time`; use FSM instead

## Priority

Low - Current implementation works correctly. This is an architectural improvement for code organization, not a bug fix or feature.

## Dependencies

None - FSM infrastructure already exists and is tested.
