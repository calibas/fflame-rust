//! **One compute pass timed on the GPU, read back without waiting.**
//!
//! The flame governor's measure of how much of a frame its dispatch
//! fills (`App`'s batch fill, [RENDERER.md](../../docs/main/RENDERER.md),
//! *The frame-time governor*): under vsync the time between frames is the
//! refresh interval however little the GPU did, so only the GPU's own
//! clock says how much room a frame has left. The escape renderer paces
//! its chunks the same way (`escape::renderer`'s `TimestampPacer`).
//!
//! One measurement in flight at a time: a pass is timed, its two
//! timestamps resolved and copied in the same encoder; the copy is mapped
//! on the next call, once that encoder has been submitted; the result is
//! read whenever the map has landed. Passes in between are not timed.
//! Without `TIMESTAMP_QUERY` (some browsers) there is no timer at all.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use egui_wgpu::wgpu::{
    Buffer, BufferDescriptor, BufferUsages, CommandEncoder, ComputePassTimestampWrites, Device, Features, MapMode,
    QuerySet, QuerySetDescriptor, QueryType, Queue,
};

/// A timed pass: its GPU time, what it ran, and the caller's tag for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassTiming {
    pub ms: f64,
    /// Thread-iterations the pass dispatched.
    pub iterations: u64,
    /// The caller's tag when the pass was recorded: what the measurement
    /// is valid for.
    pub tag: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Nothing in flight: the next pass may be timed.
    Idle,
    /// The pass's timestamps are resolved and copied in an encoder the
    /// caller has not submitted yet: the map waits for the next call.
    Encoded,
    /// Map requested; waiting on the callback.
    Mapping,
}

pub struct PassTimer {
    query_set: QuerySet,
    resolve: Buffer,
    staging: Buffer,
    /// Nanoseconds a timestamp tick.
    period_ns: f32,
    phase: Phase,
    iterations: u64,
    tag: u64,
    /// The map's outcome, from its callback: 0 pending, 1 mapped, 2 failed.
    done: Arc<AtomicU32>,
}

impl PassTimer {
    /// Two timestamps: 16 bytes resolved, 16 read back.
    const BYTES: u64 = 16;

    /// None where the device cannot time passes.
    pub fn new(device: &Device, queue: &Queue) -> Option<Self> {
        if !device.features().contains(Features::TIMESTAMP_QUERY) {
            return None;
        }
        Some(Self {
            query_set: device.create_query_set(&QuerySetDescriptor {
                label: Some("Pass Timer"),
                ty: QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&BufferDescriptor {
                label: Some("Pass Timer Resolve"),
                size: Self::BYTES,
                usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            staging: device.create_buffer(&BufferDescriptor {
                label: Some("Pass Timer Staging"),
                size: Self::BYTES,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period_ns: queue.get_timestamp_period(),
            phase: Phase::Idle,
            iterations: 0,
            tag: 0,
            done: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Advance the measurement in flight -- map the copy the last call
    /// recorded (its encoder is submitted by now), or read a map that has
    /// landed -- and return a finished measurement.
    pub fn collect(&mut self) -> Option<PassTiming> {
        match self.phase {
            Phase::Idle => None,
            Phase::Encoded => {
                self.done.store(0, Ordering::Relaxed);
                let done = self.done.clone();
                self.staging.slice(..).map_async(MapMode::Read, move |r| {
                    done.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
                });
                self.phase = Phase::Mapping;
                None
            }
            Phase::Mapping => match self.done.load(Ordering::Relaxed) {
                0 => None,
                1 => {
                    let ticks = {
                        let view = self.staging.slice(..).get_mapped_range();
                        let start = u64::from_le_bytes(view[0..8].try_into().unwrap_or_default());
                        let end = u64::from_le_bytes(view[8..16].try_into().unwrap_or_default());
                        end.saturating_sub(start)
                    };
                    self.staging.unmap();
                    self.phase = Phase::Idle;
                    let ms = ticks as f64 * self.period_ns as f64 * 1e-6;
                    (ms > 0.0).then_some(PassTiming { ms, iterations: self.iterations, tag: self.tag })
                }
                _ => {
                    // The map failed: nothing is mapped, nothing to unmap.
                    self.phase = Phase::Idle;
                    None
                }
            },
        }
    }

    /// The timestamp writes for the next pass, when no measurement is in
    /// flight; the pass is then followed by [`Self::after_pass`].
    pub fn writes(&self) -> Option<ComputePassTimestampWrites<'_>> {
        (self.phase == Phase::Idle).then_some(ComputePassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        })
    }

    /// After a pass [`Self::writes`] timed: resolve and copy its
    /// timestamps in the same encoder.
    pub fn after_pass(&mut self, encoder: &mut CommandEncoder, iterations: u64, tag: u64) {
        encoder.resolve_query_set(&self.query_set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.staging, 0, Self::BYTES);
        self.iterations = iterations;
        self.tag = tag;
        self.phase = Phase::Encoded;
    }
}
