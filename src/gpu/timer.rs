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

/// A frame's GPU time: the UI's paint, then the engine's work after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameTiming {
    /// The caller's number for the frame.
    pub frame: u64,
    pub ui_ms: f64,
    pub engine_ms: f64,
}

/// **A whole frame timed on the GPU** (docs/projects/quality-governor.md,
/// phase 1), where [`PassTimer`] times one pass. Three marks: the frame's
/// start, the UI painted, the engine's work done. Each mark is an empty
/// compute pass with an end-of-pass timestamp, which lands once all the
/// work queued before it has finished (Vulkan writes pass timestamps at
/// `BOTTOM_OF_PIPE`), so a span is the GPU's time to get through its
/// part, gaps between submissions included: a gap means the GPU waited on
/// the CPU, which the caller's own clock shows.
///
/// A few frames in flight; a frame that finds no slot free is not timed.
pub struct FrameTimer {
    slots: Vec<FrameSlot>,
    /// The slot this frame's marks go to, from [`Self::begin`] to
    /// [`Self::end`].
    current: Option<usize>,
    /// Nanoseconds a timestamp tick.
    period_ns: f32,
}

struct FrameSlot {
    query_set: QuerySet,
    resolve: Buffer,
    staging: Buffer,
    phase: FramePhase,
    frame: u64,
    /// The map's outcome, from its callback: 0 pending, 1 mapped, 2 failed.
    done: Arc<AtomicU32>,
}

#[derive(Clone, Copy, PartialEq)]
enum FramePhase {
    Idle,
    /// Marks being recorded; the frame has not ended.
    Recording,
    /// Ended: resolved and copied in an encoder the caller submits before
    /// the next [`FrameTimer::collect`].
    Encoded,
    Mapping,
}

impl FrameTimer {
    /// Frames timed at once.
    const SLOTS: usize = 4;
    /// Three timestamps: 24 bytes.
    const BYTES: u64 = 24;

    /// None where the device cannot time passes.
    pub fn new(device: &Device, queue: &Queue) -> Option<Self> {
        if !device.features().contains(Features::TIMESTAMP_QUERY) {
            return None;
        }
        let slots = (0..Self::SLOTS)
            .map(|_| FrameSlot {
                query_set: device.create_query_set(&QuerySetDescriptor {
                    label: Some("Frame Timer"),
                    ty: QueryType::Timestamp,
                    count: 3,
                }),
                resolve: device.create_buffer(&BufferDescriptor {
                    label: Some("Frame Timer Resolve"),
                    size: Self::BYTES,
                    usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                staging: device.create_buffer(&BufferDescriptor {
                    label: Some("Frame Timer Staging"),
                    size: Self::BYTES,
                    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                phase: FramePhase::Idle,
                frame: 0,
                done: Arc::new(AtomicU32::new(0)),
            })
            .collect();
        Some(Self { slots, current: None, period_ns: queue.get_timestamp_period() })
    }

    /// The frame's first mark, at the start of its first encoder. A frame
    /// that never ended (its encoder dropped on an early return) gives its
    /// slot back.
    pub fn begin(&mut self, encoder: &mut CommandEncoder, frame: u64) {
        for s in &mut self.slots {
            if s.phase == FramePhase::Recording {
                s.phase = FramePhase::Idle;
            }
        }
        self.current = self.slots.iter().position(|s| s.phase == FramePhase::Idle);
        if let Some(i) = self.current {
            self.slots[i].phase = FramePhase::Recording;
            self.slots[i].frame = frame;
            self.mark(encoder, 0);
        }
    }

    /// The UI is painted: the second mark, at the end of its encoder.
    pub fn ui_done(&mut self, encoder: &mut CommandEncoder) {
        self.mark(encoder, 1);
    }

    /// The engine's work is recorded: the last mark, then the resolve and
    /// the copy, in the frame's last encoder.
    pub fn end(&mut self, encoder: &mut CommandEncoder) {
        let Some(i) = self.current.take() else { return };
        self.mark_slot(encoder, i, 2);
        let s = &mut self.slots[i];
        encoder.resolve_query_set(&s.query_set, 0..3, &s.resolve, 0);
        encoder.copy_buffer_to_buffer(&s.resolve, 0, &s.staging, 0, Self::BYTES);
        s.phase = FramePhase::Encoded;
    }

    fn mark(&mut self, encoder: &mut CommandEncoder, index: u32) {
        if let Some(i) = self.current {
            self.mark_slot(encoder, i, index);
        }
    }

    fn mark_slot(&self, encoder: &mut CommandEncoder, slot: usize, index: u32) {
        encoder.begin_compute_pass(&egui_wgpu::wgpu::ComputePassDescriptor {
            label: Some("Frame Timer Mark"),
            timestamp_writes: Some(ComputePassTimestampWrites {
                query_set: &self.slots[slot].query_set,
                beginning_of_pass_write_index: None,
                end_of_pass_write_index: Some(index),
            }),
        });
    }

    /// After the frame's submissions: map the slots ended since the last
    /// call, and return the frames whose maps have landed.
    pub fn collect(&mut self) -> Vec<FrameTiming> {
        let period_ms = self.period_ns as f64 * 1e-6;
        let mut out = Vec::new();
        for s in &mut self.slots {
            match s.phase {
                FramePhase::Encoded => {
                    s.done.store(0, Ordering::Relaxed);
                    let done = s.done.clone();
                    s.staging.slice(..).map_async(MapMode::Read, move |r| {
                        done.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
                    });
                    s.phase = FramePhase::Mapping;
                }
                FramePhase::Mapping => match s.done.load(Ordering::Relaxed) {
                    0 => {}
                    1 => {
                        let t: [u64; 3] = {
                            let view = s.staging.slice(..).get_mapped_range();
                            let at = |k: usize| u64::from_le_bytes(view[k * 8..k * 8 + 8].try_into().unwrap_or_default());
                            [at(0), at(1), at(2)]
                        };
                        s.staging.unmap();
                        s.phase = FramePhase::Idle;
                        // Out of order means a mark never ran: not a timing.
                        if t[0] > 0 && t[0] <= t[1] && t[1] <= t[2] {
                            out.push(FrameTiming {
                                frame: s.frame,
                                ui_ms: (t[1] - t[0]) as f64 * period_ms,
                                engine_ms: (t[2] - t[1]) as f64 * period_ms,
                            });
                        }
                    }
                    _ => s.phase = FramePhase::Idle,
                },
                FramePhase::Idle | FramePhase::Recording => {}
            }
        }
        out.sort_by_key(|t| t.frame);
        out
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "web-app"))]
mod tests {
    use super::*;
    use egui_wgpu::wgpu::PollType;

    /// Frames marked around work the GPU does come back, each once, with
    /// the engine's span covering that work; a frame whose encoder was
    /// dropped before it ended gives its slot back.
    #[test]
    fn a_frame_comes_back_with_its_spans() {
        let Ok((device, queue)) = pollster::block_on(crate::app::export::headless_device()) else {
            eprintln!("no GPU adapter: skipped");
            return;
        };
        let Some(mut timer) = FrameTimer::new(&device, &queue) else {
            eprintln!("no TIMESTAMP_QUERY: skipped");
            return;
        };
        let work = device.create_buffer(&BufferDescriptor {
            label: Some("work"),
            size: 64 << 20,
            usage: BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // An early return: the frame began, its encoder never submitted.
        {
            let mut dropped = device.create_command_encoder(&Default::default());
            timer.begin(&mut dropped, 0);
        }
        let mut timings = Vec::new();
        for frame in 1..=(FrameTimer::SLOTS as u64) {
            let mut ui = device.create_command_encoder(&Default::default());
            timer.begin(&mut ui, frame);
            timer.ui_done(&mut ui);
            queue.submit(std::iter::once(ui.finish()));
            let mut engine = device.create_command_encoder(&Default::default());
            for _ in 0..16 {
                engine.clear_buffer(&work, 0, None);
            }
            timer.end(&mut engine);
            queue.submit(std::iter::once(engine.finish()));
            timings.extend(timer.collect());
        }
        for _ in 0..10 {
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            timings.extend(timer.collect());
        }
        let frames: Vec<u64> = timings.iter().map(|t| t.frame).collect();
        assert_eq!(frames, (1..=FrameTimer::SLOTS as u64).collect::<Vec<_>>(), "every frame once, the dropped one in none");
        for t in &timings {
            assert!(t.engine_ms > 0.0 && t.engine_ms > t.ui_ms, "the engine's span holds the work: {t:?}");
        }
    }
}
