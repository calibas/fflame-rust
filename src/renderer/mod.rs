#[cfg(any(feature = "engine-escape", feature = "engine-sim"))]
pub mod average;
pub mod coat;
pub mod compute_kernel;
pub mod effect_chain;
pub mod histogram;
pub mod render;
pub mod shade_pass;
pub mod sticky;
pub mod dof_pass;
pub mod density_stats;
pub mod thumbnail;
pub mod upscale;
#[cfg(test)]
pub(crate) mod density_gates;
#[cfg(test)]
mod orbit_gates;
#[cfg(test)]
mod param_cost;
#[cfg(all(test, not(target_arch = "wasm32"), feature = "web-app", feature = "engine-flame", feature = "engine-escape", feature = "engine-sim"))]
mod calibration;

pub use compute_kernel::{FlameRenderer, TargetingState};
pub use histogram::{compute_histogram_async, DensityHistogram, HISTOGRAM_BINS};
pub use render::{render, render_with, NoProgress, RenderEngines, RenderError, RenderJob, RenderOutput, RenderProgress};
pub use thumbnail::{render_thumbnail_async, THUMBNAIL_ITERATIONS, THUMBNAIL_ITERATIONS_PER_THREAD, THUMBNAIL_SIZE};

#[cfg(not(target_arch = "wasm32"))]
pub use thumbnail::render_thumbnail;
