# Local plugin examples

Two worked examples of local-only plugins — one variation, one effect.
Neither is loaded from here: copy the file into your plugins folder and
restart.

| | desktop | web |
|---|---|---|
| variations | `<app data>/plugins/variations/` | localStorage `plugins/variations/…` |
| effects | `<app data>/plugins/effects/` | localStorage `plugins/effects/…` |

On Windows the app data folder is
`%APPDATA%\fractal-flame\fractal_flame_wgpu\data\`.

## The file *name* is the identity

A plugin is registered under its **file name**, not the `name` inside
it. Two files cannot claim one name, and which one won would otherwise
depend on directory order.

## `plugin_example.json` — a variation

Deliberately equal to `linear` at its defaults (`scale = 1`,
`twist = 0`), so the render is byte-identical to a `linear` flame until
you change a parameter. That makes it a check as well as an example: if
the picture moves at default settings, something is wrong.

The format is the same JSON the API serves for a downloaded variation.
That is the whole design — one registration path, one compile path, one
set of ceilings — and it means submitting a plugin for curation is
sending the file you already have.

## `plugin_example_tint.json` — a color effect

`amount = 0` is a true no-op, for the same reason. It calls `luminance`,
`hsl_to_rgb` and `apply_blend` from the shared blend-mode library, so it
also demonstrates the `INCLUDE_BLEND_MODES` directive.

**The directive must be alone on its line.** Mentioning it inside prose
does not trigger a splice — this file's own header comment quotes it,
which is where that behaviour was first discovered to matter.

A shader that calls into the shared library *without* the directive is
refused at registration rather than left to fail at compile time naming
a function nobody wrote.

## What is refused

Names are never shadowed, in either direction (see the shared-resource
plan, §0 decision 3):

- a plugin may not take a built-in's name — shadowing `linear` would
  change what every shared flame renders;
- a download may not displace a plugin — that would replace your work
  with somebody else's without asking.

Refusals name the conflict, on startup, in the app and on stderr for
headless runs.

## One consequence to know about

A flame travels as variation **names**, never definitions. So a flame
using one of your plugins renders correctly here and nowhere else —
including for you on another device. Saving or uploading such a flame
says so.
