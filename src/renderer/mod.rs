pub mod compute_kernel;
pub mod render;
pub mod thumbnail;

pub use compute_kernel::{FlameRenderer, PathEntry};
pub use render::{render, NoProgress, RenderError, RenderJob, RenderOutput, RenderProgress};
pub use thumbnail::{render_thumbnail_async, THUMBNAIL_ITERATIONS, THUMBNAIL_ITERATIONS_PER_THREAD, THUMBNAIL_SIZE};

#[cfg(not(target_arch = "wasm32"))]
pub use thumbnail::render_thumbnail;
