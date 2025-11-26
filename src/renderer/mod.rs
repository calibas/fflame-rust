pub mod compute_kernel;

#[cfg(not(target_arch = "wasm32"))]
pub mod thumbnail;

pub use compute_kernel::FlameRenderer;

#[cfg(not(target_arch = "wasm32"))]
pub use thumbnail::render_thumbnail;
