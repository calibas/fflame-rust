//! Cameras, unified (docs/projects/camera-unification.md).
//!
//! The app has seven cameras -- the flame's 2D view and its 3D camera,
//! the escape plane, mode D's solid camera, the escape terrain's, the
//! simulation's view and its terrain's -- each with its own fields,
//! units and zero, which the files, the JWF exchange, the animation
//! tracks and the scripts all address. They stay. This module is the one
//! place that knows what they mean, so the viewport's gestures, the View
//! panel and the fly mode can treat them alike:
//!
//! - [`quat`]: the orientation every 3D turn composes in, gimbal-free;
//! - [`chain`]: the one Euler chain every 3D camera stores, and the way
//!   back from a rotation to its angles;
//! - [`view3d`]: how each 3D camera's stored angles enter that chain;
//! - [`gesture`]: the viewport's camera gestures, as config edits;
//! - [`fly`]: one fly mode for every 3D camera -- look about the eye, fly
//!   along its axes.
//!
//! A camera operation is a [`CameraEdit`]: config writes and the history
//! entry they go under. The caller applies it through the ConfigManager,
//! so undo, animation and scripts see the same writes they always did.

pub mod chain;
pub mod fly;
pub mod gesture;
pub mod quat;
pub mod view3d;

use crate::config::{ConfigPath, ConfigValue};

/// Config writes a camera operation makes, and how they enter the
/// history: as one parameter edit (`history: None`, coalescing as that
/// parameter's edits do) or as a named batch.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraEdit {
    pub changes: Vec<(ConfigPath, ConfigValue)>,
    pub history: Option<&'static str>,
}

impl CameraEdit {
    /// One parameter, edited as that parameter.
    pub fn param(path: ConfigPath, value: ConfigValue) -> Self {
        CameraEdit { changes: vec![(path, value)], history: None }
    }

    /// Several, as one named entry.
    pub fn batch(changes: Vec<(ConfigPath, ConfigValue)>, history: &'static str) -> Self {
        CameraEdit { changes, history: Some(history) }
    }

    /// Make the writes: through `update_param` for one parameter's edit,
    /// `update_batch` for a named one -- and silently while an animation
    /// plays, as `update_param` already was and `update_batch` was not
    /// (a gesture during playback filled the undo history).
    pub fn apply(self, config_manager: &mut crate::config::ConfigManager) {
        if config_manager.is_animation_mode() {
            for (path, value) in self.changes {
                let _ = config_manager.update_param_silent(path, value);
            }
            return;
        }
        match self.history {
            None => {
                for (path, value) in self.changes {
                    let _ = config_manager.update_param(path, value);
                }
            }
            Some(history) => {
                let _ = config_manager.update_batch(self.changes, history.to_string());
            }
        }
    }
}

/// The picture a gesture is made on, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    pub fn new(width: f64, height: f64) -> Self {
        Viewport { width: width.max(1.0), height: height.max(1.0) }
    }
}
