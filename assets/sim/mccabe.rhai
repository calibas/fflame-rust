// McCabe's multi-scale Turing patterns (Jonathan McCabe, "Cyclic
// Symmetric Multi-Scale Turing Patterns", 2010): at each scale an
// activator's average and a wider inhibitor's; the scale whose two agree
// best wins the cell and moves it toward its activator.
//
// The Rhai twin of the built-in `mccabe` model (src/sim/models.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, presets and
// working values, its spectral discs and per-scale warps, and the same
// two passes.
//
// The averages are the framework's pyramid (`pyr_sample` at the level
// `pyr_level` gives for a radius) or, under Exact discs, each scale's
// activator-minus-inhibitor from the spectral stage (`pyr_diff`, the
// discs `fn discs(p)` names). The first pass runs only while the
// variation radius is on: every scale's signed variation into the scratch,
// which the second gathers around the cell. Channels: x = the field, y =
// the scale that won, z = the step at which the winner last changed.
//
// A scale's table row starts at the parameter slot 8 + 5i (radius, ratio,
// step, weight, symmetry) and its warp at 52 + 5i (zoom, rotation, pan x,
// pan y, swirl): read by slot, `param(k)`, as the rows are indexed.

// WGSL's `round`: half to even, as the built-in rounds its counts.

const PARAMS = #{
    scales: #{
        value: 5.0, min: 1.0, max: 6.0, caption: "Scales",
        hint: "How many scales compete. Each is twice the radius of the last, so five scales span a factor of sixteen.",
    },
    base_radius: #{
        value: 1.0, min: 0.5, max: 8.0, caption: "Finest radius",
        hint: "Activator radius of the finest scale, in cells. Everything else is built from it.",
    },
    ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Inhibitor ratio",
        hint: "Inhibitor radius over activator radius, at every scale. The paper's figures use 2.",
    },
    amount: #{
        value: 0.05, min: 0.001, max: 0.2, caption: "Step (finest)",
        hint: "How far the finest scale moves a cell per step. Larger is faster and grainier.",
    },
    amount_min: #{
        value: 0.01, min: 0.001, max: 0.2, caption: "Step (coarsest)",
        hint: "The coarsest scale's step; scales in between interpolate. Coarse scales moving slowly is what lets fine detail live inside them.",
    },
    symmetry: #{
        value: 0.0, min: 0.0, max: 8.0, caption: "Symmetry",
        hint: "n-fold rotational symmetry about the centre — the paper's own contribution. 0 or 1 is none. Costs n times the reads, and a periodic field that is rotated about its centre no longer tiles, so the corners stay asymmetric.",
    },
    memory: #{
        value: 0.0, min: 0.0, max: 1.0, caption: "Colour memory",
        hint: "How far each cell's colour moves toward the winning scale's colour per step — Softology's colour bump. Small values give smooth blends of the scales that have been winning; 1 is the latest winner alone. Colour with Scale Memory. 0 is off. Switching it on or off restarts the run, and on it keeps two more slices of the field (about 130 MB at a 1080p grid).",
    },
    layout: #{
        value: 0, choices: ["Ladder", "Table"], caption: "Scales from",
        hint: "Ladder builds every scale from the finest radius, doubling. Table sets each scale's radius, ratio, step, weight and symmetry on its own, as Softology and Reusser do; switching to it fills it from the ladder, so nothing changes until you edit it.",
    },
    s0_radius: #{
        value: 1.0, min: 0.5, max: 256.0, caption: "Scale 1 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s0_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 1 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s0_amount: #{
        value: 0.05, min: -0.2, max: 0.2, caption: "Scale 1 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s0_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 1 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s0_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 1 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    s1_radius: #{
        value: 2.0, min: 0.5, max: 256.0, caption: "Scale 2 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s1_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 2 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s1_amount: #{
        value: 0.04, min: -0.2, max: 0.2, caption: "Scale 2 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s1_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 2 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s1_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 2 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    s2_radius: #{
        value: 4.0, min: 0.5, max: 256.0, caption: "Scale 3 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s2_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 3 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s2_amount: #{
        value: 0.03, min: -0.2, max: 0.2, caption: "Scale 3 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s2_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 3 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s2_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 3 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    s3_radius: #{
        value: 8.0, min: 0.5, max: 256.0, caption: "Scale 4 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s3_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 4 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s3_amount: #{
        value: 0.02, min: -0.2, max: 0.2, caption: "Scale 4 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s3_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 4 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s3_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 4 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    s4_radius: #{
        value: 16.0, min: 0.5, max: 256.0, caption: "Scale 5 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s4_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 5 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s4_amount: #{
        value: 0.01, min: -0.2, max: 0.2, caption: "Scale 5 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s4_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 5 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s4_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 5 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    s5_radius: #{
        value: 32.0, min: 0.5, max: 256.0, caption: "Scale 6 activator radius",
        hint: "Activator radius of this scale, in cells. The inhibitor's is this times the ratio. Past the pyramid's reach at the current grid (the panel warns) both averages clamp to its top level and the scale wins everywhere.",
    },
    s5_ratio: #{
        value: 2.0, min: 1.25, max: 4.0, caption: "Scale 6 inhibitor ratio",
        hint: "Inhibitor radius over activator radius for this scale.",
    },
    s5_amount: #{
        value: 0.01, min: -0.2, max: 0.2, caption: "Scale 6 step",
        hint: "How far this scale moves a cell when it wins. Negative inverts it: the cell moves away from the activator, as one of Reusser's default scales does.",
    },
    s5_weight: #{
        value: 1.0, min: -4.0, max: 4.0, caption: "Scale 6 weight",
        hint: "Softology's weight: both averages are multiplied by it before they are compared. Larger makes the scale's variation larger, so it wins less often; negative also flips its direction; 0 makes it win everywhere.",
    },
    s5_symmetry: #{
        value: 0.0, min: 0.0, max: 12.0, caption: "Scale 6 symmetry",
        hint: "n-fold rotational symmetry of this scale's averages about the centre; 0 or 1 is none. McCabe's fig. 14 puts 3-fold on the small scales and 9-fold on the large. Costs n times this scale's reads.",
    },
    averaging: #{
        value: 0, choices: ["Pyramid", "Pyramid, shifted grid", "Exact discs"], caption: "Averaging",
        hint: "How the averages are taken. Pyramid is the original: fast, but its fixed grid of texels leaves faint creases that patterns can lock onto, so a table whose coarse scales move fastest leans toward the grid axes. Shifted grid moves the pyramid's grid every few steps, so nothing locks onto it -- measured, that trades the axis lean for a slight diagonal one. Exact discs averages over true discs by FFT, as Reusser and Chau do: isotropic, and the references' own texture, cells with nested detail, at 1.7x the step and ~150 MB at 1080p. Shifted grid and Exact discs need the wrap-around boundary; a grid whose size has a prime factor above 64 keeps the pyramid.",
    },
    s0_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 1 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s1_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 2 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s2_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 3 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s3_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 4 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s4_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 5 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s5_stretch: #{
        value: 1.0, min: 1.0, max: 4.0, caption: "Scale 6 stretch",
        hint: "Stretches this scale's discs into ellipses of the same area, this many times longer than wide, so its pattern grains along the angle: the lean, on purpose. 1 is round. Exact discs averaging only; the pyramid's averages are round.",
    },
    s0_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 1 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    s1_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 2 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    s2_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 3 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    s3_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 4 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    s4_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 5 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    s5_angle: #{
        value: 0.0, min: 0.0, max: 180.0, caption: "Scale 6 angle",
        hint: "The direction of this scale's stretch, in degrees counter-clockwise from horizontal, so 90 is vertical. Nothing at stretch 1. Exact discs averaging only.",
    },
    variation: #{
        value: 0.0, min: 0.0, max: 4.0, caption: "Variation radius",
        hint: "Averages each scale's variation over a disc of this radius, in cells, before the scales are compared: the paper's variation around the pixel, Softology's variation radius. The winner is then decided by its neighbourhood rather than the cell alone, so each scale holds larger, smoother regions with less speckle. 0 is off. While on it costs a pass a step and two more field slices (about 130 MB on a 1080p grid).",
    },
    s0_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 1 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s0_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 1 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s0_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 1 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s0_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 1 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s0_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 1 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
    s1_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 2 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s1_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 2 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s1_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 2 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s1_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 2 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s1_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 2 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
    s2_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 3 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s2_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 3 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s2_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 3 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s2_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 3 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s2_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 3 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
    s3_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 4 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s3_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 4 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s3_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 4 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s3_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 4 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s3_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 4 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
    s4_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 5 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s4_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 5 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s4_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 5 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s4_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 5 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s4_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 5 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
    s5_warp_zoom: #{
        value: 1.0, min: 0.9, max: 1.1, caption: "Scale 6 warp zoom",
        hint: "This scale reads its averages through a zoom about the centre each step, so its structure drifts outward (above 1) or inward, as the field warp's zoom would move the whole field. 1 is none.",
    },
    s5_warp_rotation: #{
        value: 0.0, min: -0.25, max: 0.25, caption: "Scale 6 warp rotation",
        hint: "Radians a step this scale's reading turns about the centre, so its structure turns while the other scales stay. 0 is none.",
    },
    s5_warp_pan_x: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 6 warp pan x",
        hint: "Cells a step this scale's reading moves sideways, so its structure drifts across the others. 0 is none.",
    },
    s5_warp_pan_y: #{
        value: 0.0, min: -4.0, max: 4.0, caption: "Scale 6 warp pan y",
        hint: "Cells a step this scale's reading moves up or down. 0 is none.",
    },
    s5_warp_flow: #{
        value: 0.0, min: -0.5, max: 0.5, caption: "Scale 6 warp swirl",
        hint: "A swirl of this scale's reading: extra radians a step at the rim, none at the centre. Strong values make a vortex of this scale alone. 0 is none.",
    },
};
const INFO = #{
    title: "McCabe Multi-Scale",
    id: "mccabe",
    description: "Turing patterns at several scales at once, the finest structure nested inside the coarsest — the electron-microscope look — with optional rotational symmetry.",
    dt: 1.0,
    max_dt: 1.0,
    steps: 200,
    time_step: false,
    settles: false,
    measure: #{ param: "variation", slices: 2 },
    scale_warps: #{ rows_param: "scales", rows: 6 },
    presets: [
        #{
            name: "multiscale",
            title: "Multi-scale",
            params: #{ scales: 5.0, base_radius: 1.0, ratio: 2.0, amount: 0.05, amount_min: 0.01, symmetry: 0.0, layout: 0.0, memory: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "scale_mix",
            coloring_params: #{ scales: 5.0, value_scale: 0.5 },
        },
        #{
            name: "coarse",
            title: "Coarse (nested contours)",
            params: #{ scales: 5.0, base_radius: 3.0, ratio: 2.0, amount: 0.05, amount_min: 0.01, symmetry: 0.0, layout: 0.0, memory: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "memory",
            title: "Colour memory",
            params: #{ scales: 5.0, base_radius: 3.0, ratio: 2.0, amount: 0.05, amount_min: 0.01, symmetry: 0.0, memory: 0.05, layout: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "scale_memory",
            coloring_params: #{ scales: 5.0, value_scale: 0.5, brightness: 0.0 },
        },
        #{
            name: "rosette",
            title: "Five-fold rosette",
            params: #{ scales: 5.0, base_radius: 3.0, ratio: 2.0, amount: 0.05, amount_min: 0.01, symmetry: 5.0, layout: 0.0, memory: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "symmetry_mix",
            title: "Mixed symmetry",
            params: #{ layout: 1.0, memory: 0.0, scales: 5.0, s0_radius: 3.0, s0_ratio: 2.0, s0_amount: 0.05, s0_weight: 1.0, s0_symmetry: 3.0, s1_radius: 6.0, s1_ratio: 2.0, s1_amount: 0.04, s1_weight: 1.0, s1_symmetry: 3.0, s2_radius: 12.0, s2_ratio: 2.0, s2_amount: 0.03, s2_weight: 1.0, s2_symmetry: 3.0, s3_radius: 24.0, s3_ratio: 2.0, s3_amount: 0.02, s3_weight: 1.0, s3_symmetry: 9.0, s4_radius: 48.0, s4_ratio: 2.0, s4_amount: 0.01, s4_weight: 1.0, s4_symmetry: 9.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "channel",
            coloring_params: #{ channel: 0.0, scale: 0.5, offset: 0.5, wrap: 0.0 },
        },
        #{
            name: "uneven",
            title: "Uneven scales",
            params: #{ layout: 1.0, memory: 0.05, scales: 5.0, s0_radius: 1.0, s0_ratio: 2.0, s0_amount: 0.05, s0_weight: 1.0, s0_symmetry: 0.0, s1_radius: 3.0, s1_ratio: 2.0, s1_amount: 0.04, s1_weight: 1.0, s1_symmetry: 0.0, s2_radius: 10.0, s2_ratio: 2.0, s2_amount: 0.03, s2_weight: 1.0, s2_symmetry: 0.0, s3_radius: 20.0, s3_ratio: 2.0, s3_amount: 0.02, s3_weight: 1.0, s3_symmetry: 0.0, s4_radius: 45.0, s4_ratio: 2.0, s4_amount: 0.01, s4_weight: 1.0, s4_symmetry: 0.0 },
            steps: 200,
            boundary: "periodic",
            init: #{ noise: #{ amplitude: 1.0 } },
            coloring: "scale_memory",
            coloring_params: #{ scales: 5.0, value_scale: 0.5, brightness: 0.0 },
        },
    ],
};

fn round_even(x) {
    let r = floor(x + 0.5);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

// An average of the field over radius r at a position in cells, with the
// cyclic symmetry folded in: the mean over the n rotations of the position
// about the grid's centre.
fn mc_avg(r, x, y, sym, ctx) {
    let level = pyr_level(r);
    if sym < 2 {
        pyr_sample(level, x, y)
    } else {
        let gx = to_float(ctx.width);
        let gy = to_float(ctx.height);
        let cx = gx * 0.5;
        let cy = gy * 0.5;
        let acc = 0.0;
        for k in 0..sym {
            let a = 6.28318530718 * to_float(k) / to_float(sym);
            let dx = x - cx;
            let dy = y - cy;
            let qx = cos(a) * dx - sin(a) * dy + cx;
            let qy = sin(a) * dx + cos(a) * dy + cy;
            // A float periodic wrap, so a rotated corner reads the tile.
            qx = qx - gx * floor(qx / gx);
            qy = qy - gy * floor(qy / gy);
            acc = acc + pyr_sample(level, qx, qy);
        }
        acc / to_float(sym)
    }
}

// Exact discs: scale i's activator-minus-inhibitor at a position, folded
// as `mc_avg` folds. The rotations' mean of a - b is the means' difference.
fn mc_diff(i, x, y, sym, ctx) {
    if sym < 2 {
        pyr_diff(i, x, y)
    } else {
        let gx = to_float(ctx.width);
        let gy = to_float(ctx.height);
        let cx = gx * 0.5;
        let cy = gy * 0.5;
        let acc = 0.0;
        for k in 0..sym {
            let a = 6.28318530718 * to_float(k) / to_float(sym);
            let dx = x - cx;
            let dy = y - cy;
            let qx = cos(a) * dx - sin(a) * dy + cx;
            let qy = sin(a) * dx + cos(a) * dy + cy;
            qx = qx - gx * floor(qx / gx);
            qy = qy - gy * floor(qy / gy);
            acc = acc + pyr_diff(i, qx, qy);
        }
        acc / to_float(sym)
    }
}

// Scale i's step: the table's, or the ladder's, falling linearly from the
// finest scale to the coarsest (one scale gets the finest).
fn mc_amount(i, n, p) {
    if p.layout >= 0.5 {
        param(8 + 5 * i + 2)
    } else {
        let t = if n > 1 { to_float(i) / to_float(n - 1) } else { 0.0 };
        mix(p.amount, p.amount_min, t)
    }
}

// Scale i's signed variation at a position for the measure pass: w(a - b),
// the activator's average less the inhibitor's. Its size is what the
// winner is chosen by and its sign the step's direction.
fn mc_measure(i, x, y, p, ctx) {
    let sym = to_int(round_even(p.symmetry));
    let table = p.layout >= 0.5;
    if ctx.exact_discs == 1 {
        let w = 1.0;
        let sym_i = sym;
        if table {
            let o = 8 + 5 * i;
            w = param(o + 3);
            sym_i = to_int(round_even(param(o + 4)));
        }
        w * mc_diff(i, x, y, sym_i, ctx)
    } else if table {
        let o = 8 + 5 * i;
        let ra = max(param(o), 0.5);
        let rb = ra * param(o + 1);
        let w = param(o + 3);
        let sym_i = to_int(round_even(param(o + 4)));
        // Softology's weight: both averages times w, then compared.
        let act = w * mc_avg(ra, x, y, sym_i, ctx);
        let inh = w * mc_avg(rb, x, y, sym_i, ctx);
        act - inh
    } else {
        let ra = p.base_radius * to_float(1 << i);
        let rb = ra * p.ratio;
        mc_avg(ra, x, y, sym, ctx) - mc_avg(rb, x, y, sym, ctx)
    }
}

// Scale i's read position under its warp: the field warp's map -- zoom,
// rotation, pan and swirl about the centre -- applied to where the scale
// reads its averages, so its structure moves while the others stay. As a
// vector, x and y.
fn mc_scale_pos(i, x, y, ctx) {
    let px = x;
    let py = y;
    if ctx.scale_warped == 1 {
        let o = 52 + 5 * i;
        let zoom = max(param(o), 0.0001);
        let rot = param(o + 1);
        let pan_x = param(o + 2);
        let pan_y = param(o + 3);
        let flow = param(o + 4);
        if !(zoom == 1.0 && rot == 0.0 && flow == 0.0 && pan_x == 0.0 && pan_y == 0.0) {
            // Undo the pan and the zoom, then turn back by the rate plus
            // the swirl, none at the centre and `flow` at the rim.
            let gx = to_float(ctx.width);
            let gy = to_float(ctx.height);
            let cx = gx * 0.5;
            let cy = gy * 0.5;
            let dx = x - cx;
            let dy = y - cy;
            let rim = max(min(gx, gy) * 0.5, 1.0);
            let theta = rot + flow * (sqrt(dx * dx + dy * dy) / rim);
            let qx = (dx - pan_x) / zoom;
            let qy = (dy - pan_y) / zoom;
            let cs = cos(theta);
            let sn = sin(theta);
            px = cx + (cs * qx + sn * qy);
            py = cy + (-sn * qx + cs * qy);
        }
    }
    vec3(px, py, 0.0)
}

// The measure pass, run while the variation radius is on: every scale's
// signed variation at this cell into the scratch, scales 0-3 in the first
// slice and 4-5 in the second. The state goes through unchanged.
fn cell_step(cell, p, ctx) {
    let n = to_int(clamp(round_even(p.scales), 1.0, 6.0));
    let x = ctx.cell.re + 0.5;
    let y = ctx.cell.im + 0.5;
    let m = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    for i in 0..n {
        let q = mc_scale_pos(i, x, y, ctx);
        m[i] = mc_measure(i, q.x, q.y, p, ctx);
    }
    write_scratch(0, rgba(m[0], m[1], m[2], m[3]));
    write_scratch(1, rgba(m[4], m[5], m[6], m[7]));
    cell
}

// The step: the scale with the least variation wins and moves the cell by
// its step, toward the activator.
fn cell_step2(cell, p, ctx) {
    let n = to_int(clamp(round_even(p.scales), 1.0, 6.0));
    let x = ctx.cell.re + 0.5;
    let y = ctx.cell.im + 0.5;
    let best_var = 1.0e30;
    let best_dir = 0.0;
    let best_scale = 0.0;
    if ctx.scratch_on == 1 {
        // The variation radius: each scale's |variation| over an
        // antialiased disc around the cell, from what the measure pass
        // wrote; the direction is still the cell's own. Every scale is
        // weighted alike, so the disc's weight is never divided out; a
        // cell past an edge the boundary does not wrap counts for nothing.
        let rv = clamp(p.variation, 0.0, 4.0);
        let reach = to_int(floor(rv + 0.5));
        let acc0 = rgba(0.0, 0.0, 0.0, 0.0);
        let acc1 = rgba(0.0, 0.0, 0.0, 0.0);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let fx = to_float(dx);
                let fy = to_float(dy);
                let w = clamp(rv + 0.5 - sqrt(fx * fx + fy * fy), 0.0, 1.0);
                if w <= 0.0 || outside(dx, dy) {
                    continue;
                }
                acc0 = acc0 + w * abs(scratch(dx, dy, 0));
                acc1 = acc1 + w * abs(scratch(dx, dy, 1));
            }
        }
        let own0 = scratch(0, 0, 0);
        let own1 = scratch(0, 0, 1);
        let acc = [acc0.x, acc0.y, acc0.z, acc0.w, acc1.x, acc1.y, acc1.z, acc1.w];
        let own = [own0.x, own0.y, own0.z, own0.w, own1.x, own1.y, own1.z, own1.w];
        for i in 0..n {
            let v = acc[i];
            let d = own[i];
            if v < best_var {
                best_var = v;
                let amt = mc_amount(i, n, p);
                best_dir = if d > 0.0 { amt } else { -amt };
                best_scale = to_float(i);
            }
        }
    } else if ctx.scale_warped == 1 {
        // Per-scale warps: each scale measured at its own read position.
        for i in 0..n {
            let q = mc_scale_pos(i, x, y, ctx);
            let d = mc_measure(i, q.x, q.y, p, ctx);
            let v = abs(d);
            if v < best_var {
                best_var = v;
                let amt = mc_amount(i, n, p);
                best_dir = if d > 0.0 { amt } else { -amt };
                best_scale = to_float(i);
            }
        }
    } else {
        // The rule as it was before the measure pass, written out: the
        // built-in keeps this shape so a run without the variation radius
        // is the run it always was.
        let sym = to_int(round_even(p.symmetry));
        let table = p.layout >= 0.5;
        let exact = ctx.exact_discs == 1;
        for i in 0..n {
            let v = 0.0;
            let up = false;
            let amt = 0.0;
            if exact {
                let w = 1.0;
                let sym_i = sym;
                if table {
                    let o = 8 + 5 * i;
                    amt = param(o + 2);
                    w = param(o + 3);
                    sym_i = to_int(round_even(param(o + 4)));
                } else {
                    let t = if n > 1 { to_float(i) / to_float(n - 1) } else { 0.0 };
                    amt = mix(p.amount, p.amount_min, t);
                }
                let d = w * mc_diff(i, x, y, sym_i, ctx);
                v = abs(d);
                up = d > 0.0;
            } else if table {
                let o = 8 + 5 * i;
                let ra = max(param(o), 0.5);
                let rb = ra * param(o + 1);
                amt = param(o + 2);
                let w = param(o + 3);
                let sym_i = to_int(round_even(param(o + 4)));
                let act = w * mc_avg(ra, x, y, sym_i, ctx);
                let inh = w * mc_avg(rb, x, y, sym_i, ctx);
                v = abs(act - inh);
                up = act > inh;
            } else {
                let ra = p.base_radius * to_float(1 << i);
                let rb = ra * p.ratio;
                let act = mc_avg(ra, x, y, sym, ctx);
                let inh = mc_avg(rb, x, y, sym, ctx);
                v = abs(act - inh);
                up = act > inh;
                let t = if n > 1 { to_float(i) / to_float(n - 1) } else { 0.0 };
                amt = mix(p.amount, p.amount_min, t);
            }
            if v < best_var {
                best_var = v;
                best_dir = if up { amt } else { -amt };
                best_scale = to_float(i);
            }
        }
    }

    // Normalise by the previous step's measured range, then step.
    let span = max(ctx.field_max - ctx.field_min, 0.000001);
    let f = (cell.x - ctx.field_min) / span * 2.0 - 1.0;
    let age = if best_scale != cell.y { to_float(ctx.step) } else { cell.z };

    // Colour memory: each cell's per-scale weights moved toward the
    // winner's one-hot by the bump, which the palette is applied to at
    // display time. Off (no slices), the memory reads 0 and writes nothing.
    if ctx.memory_count > 0 {
        let b = clamp(p.memory, 0.0, 1.0);
        let i = to_int(best_scale);
        let e0 = rgba(if i == 0 { 1.0 } else { 0.0 }, if i == 1 { 1.0 } else { 0.0 }, if i == 2 { 1.0 } else { 0.0 }, if i == 3 { 1.0 } else { 0.0 });
        let e1 = rgba(if i == 4 { 1.0 } else { 0.0 }, if i == 5 { 1.0 } else { 0.0 }, if i == 6 { 1.0 } else { 0.0 }, if i == 7 { 1.0 } else { 0.0 });
        write_memory(0, mix(memory(0), e0, b));
        write_memory(1, mix(memory(1), e1, b));
    }
    rgba(f + best_dir, best_scale, age, 0.0)
}

// Uniform noise in [-1, 1]. The init shape is ignored: the rule needs a
// disordered start everywhere.
fn cell_seed(inside, noise, ctx) {
    rgba(noise * 2.0 - 1.0, 0.0, 0.0, 0.0)
}

// Each live scale's discs for the spectral stage, as the step uses them:
// the table's radius floored at half a cell, its lean (counter-clockwise
// on screen, as the app's other angles are; the grid's y runs down, so
// negated); the ladder's round, the radius doubling from the finest.
fn discs(p) {
    let n = to_int(min(max(p.scales.round(), 1.0), 6.0));
    let radius = [p.s0_radius, p.s1_radius, p.s2_radius, p.s3_radius, p.s4_radius, p.s5_radius];
    let ratio = [p.s0_ratio, p.s1_ratio, p.s2_ratio, p.s3_ratio, p.s4_ratio, p.s5_ratio];
    let stretch = [p.s0_stretch, p.s1_stretch, p.s2_stretch, p.s3_stretch, p.s4_stretch, p.s5_stretch];
    let angle = [p.s0_angle, p.s1_angle, p.s2_angle, p.s3_angle, p.s4_angle, p.s5_angle];
    let out = [];
    for i in 0..n {
        if p.layout >= 0.5 {
            let ra = max(radius[i], 0.5);
            // Rust's f32 `to_radians`: the f32 nearest pi/180, the product
            // exact in f64 and rounded as f32 rounds it.
            out.push(#{ ra: ra, rb: ra * ratio[i], stretch: min(max(stretch[i], 1.0), 4.0), angle: -(angle[i] * 0.01745329238474369) });
        } else {
            let ra = p.base_radius * to_float(1 << i);
            out.push(#{ ra: ra, rb: ra * p.ratio });
        }
    }
    out
}
