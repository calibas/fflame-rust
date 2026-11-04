/// The app's window and its surface; WASI (wasm/python) has no window.
#[cfg(not(target_os = "wasi"))]
pub mod device;
pub mod buffers;
pub mod pipelines;
pub mod timer;
