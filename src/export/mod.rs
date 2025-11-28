//! High-resolution export with optimized tiled rendering
//!
//! This module implements single-pass tiled rendering where iterations run once
//! and samples are routed to appropriate tile buffers based on screen coordinates.

mod tiled;
mod renderer;

pub use tiled::*;
pub use renderer::*;
