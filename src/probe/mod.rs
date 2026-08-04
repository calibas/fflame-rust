//! Numerical probe: evaluate every variation's shader math directly and
//! record a comparable fingerprint.
//!
//! # What this is for
//!
//! `npolar` rendered differently on macOS because Metal runs shaders
//! with fast-math, where `atan2(0, 0)` returns π/4 instead of a signed
//! zero. It was found by bisecting a visual difference, and
//! `CLAUDE.md` records that **616 `atan2` call sites across 86 files
//! remain unaudited**. A visual test can tell you a picture changed; it
//! cannot tell you which variation, at which input, computed what.
//!
//! This evaluates each variation over a fixed set of inputs and writes
//! one compact report. Diff the report between platforms — or between
//! builds — and a divergence names the variation and the input.
//!
//! # Why the comparison is coarse on purpose
//!
//! Exact float comparison across vendors is useless: different GPUs
//! legitimately differ in `sin`/`cos`/`exp` implementations, FMA
//! contraction, and reassociation, all permitted by the spec. Hashing
//! raw `f32` would flag all 646 variations on any other GPU, which is
//! the same non-signal as cross-platform golden images.
//!
//! The bugs actually being hunted are **categorical**, not last-ulp:
//!
//! | | IEEE | Metal fast-math |
//! |---|---|---|
//! | `x != x` for NaN | true | false |
//! | `Inf / Inf` | NaN | 1.0 |
//! | `atan2(0, 0)` | ±0 / ±π | π/4 |
//!
//! So each output is reduced to a [`Class`] — finite/NaN/Inf, sign, and
//! a magnitude bucket — plus a value quantised well above ulp noise.
//! That survives a vendor change and still separates π/4 from zero.
//!
//! # What it does not cover
//!
//! The accumulate and tonemap passes, and how variations compose. The
//! existing visual regression suite covers those; this covers the math.

pub mod batch;
pub mod classify;
pub mod flame;
pub mod inputs;
pub mod report;
#[cfg(not(target_arch = "wasm32"))]
pub mod run;
pub mod shader;
pub mod sweep;

pub use batch::{builtin_targets, plan_batches, Batch, Target};
pub use classify::{summarise, Class, Sample};
pub use flame::build_probe_flame;
pub use inputs::{probe_inputs, Point};
pub use report::{compare, Divergence, Entry, Meta, Report, Timings, SCHEMA};
