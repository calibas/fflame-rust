pub mod compute_kernel;
pub mod render;

#[cfg(not(target_arch = "wasm32"))]
pub mod thumbnail;

pub use compute_kernel::{FlameRenderer, PathEntry};
pub use render::{render, NoProgress, RenderError, RenderJob, RenderOutput, RenderProgress};

#[cfg(not(target_arch = "wasm32"))]
pub use thumbnail::render_thumbnail;
