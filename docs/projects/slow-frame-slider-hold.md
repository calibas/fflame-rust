# Holding a drag while frames are slow

**Status 2026-10-12:** planned, not started. Options 1 and 2 are built:
the settings known to be slow are listed per registry and their controls
commit on release or are dropdowns (`ui::slow_controls`; UI.md, *Slow
settings*). This is option 3: catch the slow settings nobody listed.

## The problem

A slider takes its value from the pointer every frame. When a change
makes the next frame slow, the whole UI waits for that frame. Meanwhile
the pointer moves on, and the next frame's value jumps. The user sees
nothing change, keeps dragging, and the setting runs to its maximum
before the picture has caught up once.

The marked lists cover what was measured or reasoned out. They miss:
- a plugin or API-downloaded variation with an iteration parameter;
- an effect parameter;
- a setting that is only slow in combination, such as a deep zoom with a
  high iteration cap;
- whatever is added later without anyone timing it.

## The idea

When a frame during a drag is slow, hold the rest of that drag. The
control keeps moving and showing its value, but the value reaches the
config only when the drag ends: `on_release` behaviour, switched on by
measurement rather than by a list.

- **Per drag, and sticky.** A drag starts live. Once one of its frames
  takes longer than a threshold, it holds until release. It never flips
  back mid-drag; flipping would make the drag's feel depend on timing
  noise.
- **The first slow frame still happens.** The drag has to see one to
  know. The overshoot shrinks from many jumps to one.
- **Frames stay fast while held.** The config does not change, so the
  renderer keeps drawing the old picture, and the UI keeps up with the
  pointer. That is the point: the user sees where the slider is.

## Where it lives

In the slider wrappers, not in each panel. `VkbSlider` and
`VkbDragValue` (`src/ui/mod.rs`) wrap egui's widgets for the virtual
keyboard, and about 110 controls already use them. The hold goes inside
their `Widget::ui`:
- keep the in-drag value in egui memory, keyed by the widget's id, as
  `slow_controls::on_release` does;
- leave `*value` alone while held;
- on release, write `*value` and mark the response changed.

Panels then commit as they already do on `changed()`.

The app supplies the signal. Each frame it writes the previous frame's
duration into egui memory, through a small `slow_controls::frame_was_slow`
/ `set_last_frame_ms` pair. The wrapper reads it while a drag is in
progress. The threshold is about 100 ms: four or more frames at 60 Hz, a
frame a user feels.

## The tricky parts

1. **The roughly 150 raw egui controls.** 64 `egui::Slider::new` and 88
   `egui::DragValue::new` in `src/ui` and `src/config` bypass the
   wrappers. Each has to be either converted to the wrapper or judged
   exempt:
   - exempt where the value is not a render setting, such as export
     sizes, a seed field or settings dialogs;
   - exempt where it is the viewport's own gesture.

   `ConfigSlider` (`src/config/slider.rs`) is a third route and needs the
   same treatment. This audit is most of the work. A source-scanning test
   should hold the line afterwards: a raw slider in a panel needs an
   `// exempt:` reason, the way `render_mode.rs` guards its single writer.
2. **What "a slow frame" measures.** The app's frame time includes shader
   compiles (seconds, once), exports freezing the viewport, and window
   resizes.
   - A compile triggered by the drag itself is a fair reason to hold;
     raising a variation's weight from 0 compiles once.
   - An unrelated hitch should not hold a drag. Count only frames that
     follow a change committed by this drag. In practice that means the
     first frame after the drag's first commit, then every frame after
     it.
3. **Governed engines hide the cost.** The flame governor shortens
   dispatches to hold the frame time, and the escape renderer sizes its
   chunks to it. So a slow setting there shows up as a slow picture, not
   a slow frame. The hold does not fire, and need not: the UI stays
   responsive. The traps are where the frame itself is slow:
   - the first frame after a change, before the governor or chunk sizer
     adapts;
   - the governor turned off;
   - simulation steps;
   - effect chains.
4. **Undo coalescing.** A held drag commits once, one undo step, where a
   live drag coalesced many. That is better; nothing to change.
5. **Animation and scripted changes** do not go through widgets, so they
   are unaffected. Timeline scrubbing is a drag on the track editor's
   widgets; decide whether it should hold, probably not, since it is
   playback.
6. **Touch.** On the web's compact layout a drag is a touch. The same
   rule works, but a phone's frames are slower, so it will hold more
   often. Measure the threshold there before shipping.

## Steps

1. The signal: the app records the last frame's duration and whether a
   committed change preceded it; `slow_controls` exposes it.
2. The hold in `VkbSlider` and `VkbDragValue`, with an egui headless test
   like `a_drag_commits_once_when_it_ends`: a simulated slow frame
   mid-drag holds the remainder; a fast drag stays live.
3. The audit: convert or exempt every raw control, then add the
   source-scanning test.
4. Feedback: while held, a small "release to apply" note beside the
   control. Without it, a frozen picture under a moving slider looks
   broken.
5. Measure on the web and on a slow GPU before turning it on everywhere.
   Keep a Rendering-menu switch to turn it off.

## Not this

- **Throttling** (apply at most every N ms) keeps the UI waiting on each
  slow frame, so the user still cannot see the slider: the trap remains.
- **A coarse preview while dragging** helps only where a cheaper picture
  exists, which a per-iteration loop count does not have.
