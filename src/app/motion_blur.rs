//! Motion blur in the viewport (docs/projects/motion-blur.md): while an
//! animation plays with it on, a frame is the average of its moments
//! (`animation::export::moment_configs`, the export's own, so a moment
//! in the app is the export's).
//!
//! The escape-time engine renders the moments in turn, each to
//! settlement, folding each into its accumulation; the viewport shows
//! the last finished frame meanwhile (`present_accumulation`), and the
//! frame settles -- so paced playback moves on -- after its last moment.
//! A simulation walks its grid to each moment's step count in turn, the
//! same way; one the timeline does not step is recoloured at each moment.
//! A flame shares its frame's samples among the moments within the frame.

use crate::animation::AnimationController;
use crate::config::FractalConfig;

/// Whether the viewport blurs: playing, with the animation's blur on.
/// Paused or scrubbing the picture is the sharp frame at the playhead,
/// which is what editing wants (D4).
pub(super) fn blurring(controller: &AnimationController) -> bool {
    controller.is_playing() && controller.animation.as_ref().is_some_and(|a| a.motion_blur.active())
}

/// A frame being built from its moments, one at a time.
pub(crate) struct Moments {
    /// The frame's moments.
    pub moments: Vec<FractalConfig>,
    /// The moment rendering now; those before it are folded in.
    pub next: usize,
    /// A simulation frame whose step count the timeline committed
    /// backward -- a loop's wrap, a turnaround, a scrub
    /// (`sim::timeline_target_applies`): its first moment runs back to
    /// its step count instead of being held where the field is.
    pub rewind: bool,
}

impl Moments {
    /// The moment being drawn now.
    pub fn current(&self) -> &FractalConfig {
        &self.moments[self.next]
    }
}
