# Angle audit: what is left

The policy (UI.md, *Angles*): **every angle a user sees is in degrees**;
storage keeps its units and the control converts. On 2026-11-22 every
variation file was read and each parameter used as an angle classified by
the units its shader reads (`radians`, `degrees`, `turns`, a `rate`, or
`other`: a factor or an offset). The clear-cut ones were converted (75:
radians, degrees declared as plain floats, turns, and `target`'s
even/odd, which said degrees and read radians). Effects were already
degrees throughout. This page holds the rest, for a decision each.

## Decisions

1. **`time` parameters** (23): most feed several rates at once or mix an
   angle with a linear scroll. Recommendation: leave them as numbers.
2. **Curve-parameter ranges** (16; tmin/tmax and friends): radians, but
   some presets use them linearly, and spirograph's run to 10,000.
   Recommendation: leave as numbers, or convert only polarplot2d/3d's
   (plainly the sampled angle).
3. **Rates** (17): an angle per unit radius, per e-fold, per step.
   Recommendation: leave as numbers (a rate in degrees per unit is
   possible but unusual).
4. **Angles in units of pi** (`other`: `v*pi/2`, `v*pi`, `pi/4 + v*pi/8`,
   ...): converting needs a factor kind (and an offset for some).
   Recommendation: convert the pure multiples with a factor if wanted;
   leave the offsets and the ones scaled by another parameter.
5. **Medium/low confidence** rows: each needs a look at the code.

## Fixed on the way

- Eleven hints that contradicted their code (ripple's phase, bubbleT3D's
  angle of hole, fan2's y, lazyTravis's spins, superShape3d's rho/phi,
  wedge_sph's swirl, truchet's rotation, log_db's fix period,
  glsl_kaliset's time, funnel's effect, quasiconformal's spiral): reworded
  to what the code does.
- `pre_bwraps` / `post_bwraps` read their twists in degrees; Apophysis 7x
  (`varPreBwraps.pas`, `varPostBwraps.pas`) feeds them to `SinCos` as
  radians. Now radians, as Apophysis: a `.flame` import renders as it does
  there, and a value saved by this app before 2026-11-22 is read as
  radians (57x the turn it had).

## The rows left

## Degrees, medium or low confidence

- `heart_wf.shift_t` (unlimited_float 0.0 -10.0 10.0): t is in degrees (clamped at t_max = 60) and also enters the polynomial (-t*t + 40t + 1200), so shift_t is a curve parameter measured in degrees

## Other: an angle with a factor or an offset (high confidence)

- `bipolar.shift` (Spec::float(0.0, -2.0, 2.0).free()): angle offset = -shift*pi/2 radians added to a half-angle that is then wrapped to [-pi/2, pi/2]; output new_y = (2/pi)*y, so shift=1 moves the output by one unit (half its period)
- `scry2.star` (unlimited_float 0.0 -10.0 10.0): angle = -star * pi/2 (quarter turns, negated); hint says 'scaled by -pi/2 internally': consistent
- `poincare3D.a` (unlimited_float 0.0 -10.0 10.0): angle = a * pi/2 (quarter turns); hint 'in units of pi/2': consistent
- `poincare3D.b` (unlimited_float 0.0 -10.0 10.0): angle = b * pi/2 (quarter turns); hint consistent
- `xheart_blur_wf.angle` (unlimited_float 0.0 -10.0 10.0): angle = pi/4 + angle * pi/8; hint 'scaled by pi/8 and offset from pi/4': consistent
- `cpow3_wf.a` (unlimited_float 0.1 -10.0 10.0): angle = a * pi/2 (quarter turns); hint 'scaled by pi/2': consistent
- `asteria.alpha` (unlimited_float 0.0 -10.0 10.0): angle = alpha * pi (half-turns); hint '(x pi)': consistent
- `bCollide.a` (unlimited_float 0.0 -10.0 10.0): angle = a * pi / num: a is a fraction of the wedge width pi/num (a=1 = one full wedge)
- `eCollide.a` (unlimited_float 0.0 -10.0 10.0): angle = a * pi / num: fraction of the wedge width
- `cut_yuebing.p1` (unlimited_float 2.0 -50.0 50.0): angle = p1/2 rad. Hint 'Fringe-frequency phase: duration = sin(p1/2)·p2' matches.
- `log_db.fix_period` (Spec::float(1.0, 0.0, 10.0).free()): angle offset in units of π rad (half-turns): k·π·fix_period, k random int. HINT CONTRADICTS CODE: hint says '0 = no shift' but fix_period <= 0 is replaced by π (full shift).
- `harmonograph_js.p3` (int 0.0 0.0 360.0): stored in degrees; phase = (p3 + 90)·π/180 (+90° added internally).
- `harmonograph_js.p4` (int 0.0 0.0 360.0): stored in degrees; phase = (p4 + 90)·π/180 (+90° added internally).
- `cpow2.a` (Spec::float(0.0, -10.0, 10.0).free()): angle = a·π/2 (quarter-turns): a = 1 is 90°. Hint matches.
- `yin_yang.ang1` (Spec::float(0.0, -2.0, 2.0).free()): angle = pi * ang1 (half-turns: 1 unit = 180 deg). Hint agrees. Needs a half-turn unit (or turns with a factor of 0.5).
- `yin_yang.ang2` (Spec::float(0.0, -2.0, 2.0).free()): angle = pi * ang2 (half-turns), same as ang1. Hint agrees.
- `lazyjess.corner` (int 1.0 1.0 50.0): angle = (corner - 1) * 2pi/n: an integer corner index, not a free angle. Probably keep as an index.
- `spligon.i` (unlimited_float 1.0 -10.0 10.0): angle offset = -pi * i / (2 * sides) = -i quarter-sectors (sector = 2pi/sides). Scale depends on sides, so no fixed degree conversion.
- `collideoscope.a` (unlimited_float 0.2 -10.0 10.0): offset = pi * a / num: a is a fraction of one sector (sector = pi/num). Scale depends on num, so no fixed degree conversion.
- `xheart.angle` (unlimited_float 0.0 -10.0 10.0): a = pi/4 + v * pi/8 (1 unit = 22.5 deg, plus a 45 deg offset). Hint agrees.
- `hypertile.n` (int 1.0 0.0 50.0): a = n * 2pi/p: an integer tile index, not a free angle. Keep as an index.
- `hypertile3D.n` (int 0.0 0.0 50.0): a = n * 2pi/p: an integer tile index. Keep as an index.
- `bipolar2.shift` (unlimited_float 0.0 -2.0 2.0): Formula: angle = -shift*pi/2 radians (one unit = -90 deg), added to an atan2 output and then wrapped to [-pi/2, pi/2].
- `target_sp.twist` (unlimited_float 0.0 -5.0 5.0): Formula: angle = twist*pi (one unit = a half-turn, 180 deg), added to atan2. Matches the hint "in half-turns".
- `perspective.angle` (unlimited_float 0.62 -10.0 10.0): Formula: angle*pi/2 radians (one unit = 90 deg). Matches the hint "units of pi/2". Despite the name, it is not in degrees.
- `line.delta` (unlimited_float 0.0 -10.0 10.0): Formula: delta*pi radians (one unit = 180 deg). Matches the hint "in units of pi".
- `line.phi` (unlimited_float 0.0 -10.0 10.0): Formula: phi*pi radians (one unit = 180 deg). Matches the hint.
- `sunflower.angle` (unlimited_float 180.0 0.0 360.0): Formula: floret step = angle*(3-sqrt5) DEGREES, i.e. t = (i+1)*angle*(3-sqrt5)*pi/180. The default 180 gives a 137.5-degree golden angle. The stored value is a degree "seed" scaled by 0.7639, not the real step angle.
- `superShape3d.rho` (unlimited_float 9.9 -100.0 100.0): Formula: max angle = rho*2/pi radians (rho1 uniform in [0, rho*2/pi]). HINT CONTRADICTS: it says "Angular range (radians) ... default 9.9 (~3pi)", but the real range is 9.9*2/pi = 6.30 rad (about 2pi).
- `superShape3d.phi` (unlimited_float 2.5 -100.0 100.0): Formula: half-range = phi*2/pi radians (phi1 uniform in [-phi*2/pi, phi*2/pi]). HINT CONTRADICTS: it says "Angular range (radians) ... 2.5 (~0.8pi)", but the real half-range is 1.59 rad.
- `vibration2.phase` (unlimited_float 0.0 -10.0 10.0): Formula: phase term = 2*pi*phase/freq radians (cycles divided by freq). The hint ("in cycles; converted as 2pi*phase/freq") matches.
- `vibration2.phase2` (unlimited_float 0.0 -10.0 10.0): Formula: 2*pi*phase2/freq2 radians.
- `loonie2.star` (unlimited_float 0.0 -10.0 10.0): Formula: angle = -star*pi/2 (quarter turns, negated). The hint ("scaled by -pi/2 internally") matches.
- `xtrb.xtrb_a` (unlimited_float 1.0 -10.0 10.0): Formula: interior angle B = 0.047 + xtrb_a radians. The hint matches.
- `xtrb.xtrb_b` (unlimited_float 1.0 -10.0 10.0): Formula: interior angle C = 0.047 + xtrb_b radians. The hint matches.

## Other, medium or low confidence

- `fan2.x` (Spec::float(1.0, 0.01, 5.0).free()): sector width in radians = pi * x^2 (flam3 fan2 convention); an angular size, not a direction; human decision whether to show it in degrees (non-linear in x)
- `cone.radius2` (unlimited_float 1.0 -10.0 10.0): per-branch angle step = pi * radius2 (half-turns), before the xwave/ywave multipliers; radius1 is a multiplier on the angle (not an angle). Hint 'Outer-radius multiplier on the random branch offset'
- `sphere_nja.stretch` (unlimited_float 1.0 -10.0 10.0): inverse rate: units of radius per radian (angle = r/stretch - pi/2); probably not one to show in degrees
- `ripple.phase` (unlimited_float 0.0 -10.0 10.0): p = phase*2pi - pi radians (turns with a half-turn offset), used both as a cosine phase (times velocity) and, as in JWF, as the lerp factor. Hint says '0..1 interpolation factor: 0 = first position, 1 = second' but the lerp factor is phase*2pi - pi (-pi at 0). Needs a human decision; velocity is a multiplier on this angle (not an angle itself)
- `scry2.circle` (unlimited_float 0.0 -10.0 10.0): angle = circle * pi/2 used as a sin/cos blend (0 = polygon, 1 = circle, as the hint says); an angle-coded mix factor, likely better left unitless
- `fdisc.rshift` (unlimited_float 1.0 -10.0 10.0): offset of rshift*pi radians on the angle, but the sum is used as a RADIUS (pr), never fed to trig; hint calls it 'Angular offset added to atan2/pi'
- `elliptic2.a3` (unlimited_float 0.0 -10.0 10.0): -π/2·a3 is added to the x OUTPUT coordinate (beside v·atan2), never fed to trig. Likely a translation, not an angle: human decision.
- `lazyTravis.spin_in` (unlimited_float 1.0 -10.0 10.0): Not a true angle: an offset along a square perimeter of length 8s, 4*v*s, so 1 unit = HALF a lap of the square (180 deg of travel, not uniform in polar angle). HINT CONTRADICTS CODE: it says one unit = one full lap.
- `lazyTravis.spin_out` (unlimited_float 0.5 -10.0 10.0): Same as spin_in: 1 unit = half a lap of the square perimeter. Hint says "same scaling as spin_in" (i.e. inherits the wrong full-lap claim).
- `ortho.in_p` (unlimited_float 0.0 -10.0 10.0): Multiplier of a point-dependent angle theta = atan2(1, ro); never a fixed angle. Probably leave unitless.
- `ortho.out_p` (unlimited_float 0.0 -10.0 10.0): Same as in_p: a multiple of a per-point angle. Probably leave unitless.
- `pow_block.root` (unlimited_float 1.0 -10.0 10.0): Branch offset = root * 2pi * k * numerator / denominator (k random integer): a turns-style multiplier on the branch step, with 1 = the natural root spacing. Hint says "x 2pi". Probably keep unitless.
- `hofstadter3d.slice_z` (float 0.25 -2.0 2.0): phi = 2pi * slice_z / size in 2D render mode, but in 3D it is a z coordinate (slab centre) and it also drives coupling (axis = Coupling). Dual role: a position, not a pure angle. Probably do not convert. slice_thickness has the same dual role (phase spread 2pi * thickness / size in 2D).
- `popcorn2_3D.popcorn2_3D_c` (unlimited_float 3.0 -10.0 10.0): Mainly a frequency (tan(c*coord)). The only direct angle use is tan(c) in the 3D z term, where it is read as radians. Recommend NOT converting.
- `post_heat.theta_phase` (unlimited_float 0.0 -10.0 10.0): Formula: phase = theta_phase/theta_period radians added inside sin(2pi*r/period + ...). Its size in radians depends on the period.
- `post_heat.phi_phase` (unlimited_float 0.0 -10.0 10.0): Formula: phi_phase/phi_period radians inside sin. 3D shader only (the 2D shader ignores it).
- `post_heat.r_phase` (unlimited_float 0.0 -10.0 10.0): Formula: r_phase/r_period radians: the phase of the radial ripple.
- `pRose3D.offset` (unlimited_float 0.0 -2.0 2.0): Formula: phase = offset*2pi/k radians (one unit = one petal cycle). 3D only. Matches the hint "+ offset*cycle".
- `funnel.effect` (int 8.0 -100.0 100.0): NOT AN ANGLE: effect*pi is added to sec(x), a ratio, never fed to a trig function. HINT MISMATCH: the hint calls it an "Additive angular offset". Do not convert; only the hint needs fixing.
- `surface_group.bend_im` (unlimited_float 0.0 -1.0 1.0): Imaginary part of the complex half translation length c. Each generator twists (loxodromic rotation) by 2*bend_im radians. Used only through cos/sin. The hint does not call it an angle. Human decision.
- `vibration2.fm` (unlimited_float 0.0 -10.0 10.0): Formula: phase offset = fm*cos(2pi*d*fmfreq)/freq radians. A phase-modulation depth divided by freq.
- `vibration2.f2m` (unlimited_float 0.0 -10.0 10.0): Formula: f2m*cos(...)/freq2 radians.
- `loonie2.circle` (unlimited_float 0.0 -10.0 10.0): Formula: angle = circle*pi/2. It is fed to sin/cos, but only as a polygon/circle blend (the hint says 0 = polygon, 1 = circle). Effectively an interpolation factor; probably not an angle to show in degrees.

## Radians, medium or low confidence

- `juliascope3Db.wave1` (unlimited_float 1.0 -10.0 10.0): constant argument of sin; only used in 3D with mode on.
- `juliascope3Db.wave2` (unlimited_float 1.0 -10.0 10.0): constant argument of cos; only used in 3D with mode on.
- `disc2.twist` (Spec::float(0.5, -10.0, 10.0).free()): radians, but beyond ±2π the raw value also scales the amplitude, so a degree UI must keep the full unwrapped value.
- `post_heat.theta_amp` (unlimited_float 0.0 -10.0 10.0): Amplitude of an angle perturbation added to atan2 (radians, multiplied by the variation weight w). Since it is an amplitude, this needs a human decision.
- `post_heat.phi_amp` (unlimited_float 0.0 -10.0 10.0): Amplitude added to the polar angle acos(z/r), multiplied by w. 3D only.
- `bsplit.x` (unlimited_float 0.0 -10.0 10.0): Shift added to the input coordinate p.x, which the body then treats as radians (sin/tan). Label "X shift": it is really a coordinate offset. Human decision; probably keep as a length.
- `bsplit.y` (unlimited_float 0.0 -10.0 10.0): Also used linearly in the Y output, so it is not purely an angle. Probably keep as a length.
- `vibration2.dm` (unlimited_float 0.0 -10.0 10.0): Depth of the angle modulation added to dir, so it is an angle amplitude in radians.
- `vibration2.tm` (unlimited_float 0.0 -10.0 10.0): Depth of the angle modulation added to angle, in radians.
- `vibration2.d2m` (unlimited_float 0.0 -10.0 10.0): Same as dm.
- `vibration2.t2m` (unlimited_float 0.0 -10.0 10.0): Same as tm.
- `waveblur_wf.phase` (unlimited_float 0.0 -10.0 10.0): Added to an acos() angle in radians, but the sum becomes a RADIUS (divided by count*pi) and is never fed to trig. Human decision.

## Curve-parameter ranges (tmin/tmax, umin/umax, vmin/vmax)

- `spirograph3D.tmin` (unlimited_float 0.0 -1000.0 1000.0): lower bound of the curve parameter t, in radians (also colour = fract(t/2pi)); range is many turns, a parameter range rather than a direction: human decision
- `spirograph3D.tmax` (unlimited_float 1000.0 -10000.0 10000.0): upper bound of t in radians; same caveat as tmin
- `spirograph.tmin` (unlimited_float -1.0 -100.0 100.0): Lower bound of the curve parameter t, which is used directly as an angle. The +-100 range covers many turns.
- `spirograph.tmax` (unlimited_float 1.0 -100.0 100.0): Upper bound of the curve parameter t; see tmin.
- `lissajous.tmin` (unlimited_float -3.14159265 -100.0 100.0): Curve parameter, scaled by the frequencies a and b inside sin. Also used linearly in the c*t drift term. Default is -pi.
- `lissajous.tmax` (unlimited_float 3.14159265 -100.0 100.0): Default is pi; see tmin.
- `parplot2d_wf.umin` (unlimited_float 0.0 -100.0 100.0): Surface parameter range. Most of the 47 presets feed u and v to cos/sin, but some use them linearly (ids 22, 33, 36, 37). Needs a human decision on whether a parameter range counts as an angle.
- `parplot2d_wf.umax` (unlimited_float 6.283185307179586 -100.0 100.0): Default is 2pi (JWF). Same caveat as umin.
- `parplot2d_wf.vmin` (unlimited_float 0.0 -100.0 100.0): Same caveat as umin.
- `parplot2d_wf.vmax` (unlimited_float 6.283185307179586 -100.0 100.0): Default is 2pi. Same caveat as umin.
- `polarplot2d_wf.tmin` (unlimited_float -3.0 -100.0 100.0): The sampled polar angle; the hint says "angle parameter". Some radius presets also use t linearly (r = t*a).
- `polarplot2d_wf.tmax` (unlimited_float 2.0 -100.0 100.0): See tmin.
- `polarplot3d_wf.tmin` (unlimited_float -3.141592653589793 -100.0 100.0): Azimuth angle. Default is -pi.
- `polarplot3d_wf.tmax` (unlimited_float 3.141592653589793 -100.0 100.0): Default is pi.
- `polarplot3d_wf.umin` (unlimited_float 0.0 -100.0 100.0): Polar angle in spherical mode (the default). In cylindrical mode it is a length instead (3D L219 `z = u`). Needs a human decision.
- `polarplot3d_wf.umax` (unlimited_float 3.141592653589793 -100.0 100.0): Default is pi; see umin.

## Rates: an angle per unit of something (high confidence)

- `lazysusan.twist` (Spec::float(0.0, -5.0, 5.0).free()): radians per unit of (1 - r): twist radians at the centre, 0 at r = 1. (spin is Spec::angle and converts by PI/180: fine)
- `swirl3.shift` (unlimited_float 0.5 -10.0 10.0): radians per unit of ln(radius)
- `bSwirl.in_p` (unlimited_float 0.0 -10.0 10.0): radians x tau (angle = in_p / tau)
- `bSwirl.out_p` (unlimited_float 0.0 -10.0 10.0): radians per unit of tau
- `eSwirl.in_p` (unlimited_float 1.2 -10.0 10.0): radians x mu (angle = in_p / mu)
- `eSwirl.out_p` (unlimited_float 0.2 -10.0 10.0): radians per unit of mu
- `curliecue2.speed` (float 0.5 0.0 1.0): turns per iteration (angular velocity of the walker). Hint 'fraction of 2π per iteration' matches.
- `wedge.swirl` (unlimited_float 0.0 -30.0 30.0): radians per unit radius.
- `fourth.twist` (unlimited_float 0.0 -10.0 10.0): Radians per unit of distance inside the threshold (w - r).
- `wedge_sph.swirl` (unlimited_float 0.3 -10.0 10.0): Radians times 1//p/ (radians per unit inverse radius). HINT CONTRADICTS: it says "Extra rotation that grows with distance", but the rotation grows toward the centre (1/r).

## Rates, medium or low confidence

- `whorl.inside` (unlimited_float 0.1 -10.0 10.0): angle = inside / (w - r): radians times distance (radians at unit distance from the weight radius); point-dependent. Probably keep unitless.
- `whorl.outside` (unlimited_float 0.2 -10.0 10.0): Same as inside.
- `disc3d.pi` (Spec::float(3.14159265358979, -10.0, 10.0).free()): Radians per unit radius in sin(pi*r), and also divides the atan2 result. It works like a frequency constant (default pi). Hint calls it a "phase constant", which it is not. Recommend leaving it unconverted.
- `quasiconformal.spiral` (unlimited_float 0.0 -4.0 4.0): Radians per e-fold of radius in both modes. HINT MISMATCH: the hint says Field mode is "turns per e-fold of radius", but there is no 2pi factor, so it is radians per e-fold.
- `helix.frequency` (Spec::float(1.0, -10.0, 10.0).free()): Turns per unit z: really the helix's frequency. Recommend leaving it. 3D only.
- `helicoid.frequency` (Spec::float(1.0, -10.0, 10.0).free()): Twist rate in turns per unit z: the rotation angle is 2pi*frequency*z. 3D only. Matches the hint.
- `pre_disc3d.pi` (unlimited_float 3.141592653589793 -10.0 10.0): Dual use: radians per unit radius in a = pi*r, and the divisor of atan2 (normalises the angle). A tunable "pi" constant; showing it in degrees would make no sense. Probably leave as is.

## Animation `time` parameters

- `cut_metaballs.time` (unlimited_float 0.0 -100.0 100.0): animation 'time' used as a phase, factor 1. Human decision: do 'time' params count as angles?
- `cut_kaleido.time` (unlimited_float 0.0 -100.0 100.0): centre rotation = 2π·0.02·time/n rad (n-dependent) plus a ring-frequency phase time/5 rad; no single unit. Animation time param.
- `cut_spiral.time` (unlimited_float 0.0 -100.0 100.0): per-step rotation by (time - π/2) rad, scaled 1.1; stored units radians (offset -π/2). Hint 'Rotation angle per spiral step' matches.
- `cut_swarp.time` (unlimited_float 0.0 -100.0 100.0): phase in radians with factors 1 and 2, and also a plain additive offset to v0. Animation time param; probably leave unconverted.
- `cut_pattern.time` (unlimited_float 0.9 -100.0 100.0): animation phase, factor 1.
- `cut_sqsplits.time` (unlimited_float 0.9 -100.0 100.0): split rotation in turns ≈ 0.7·time (a = atan2/6.3 is ~turns); time 1 ≈ 252°. Hint 'Phase of the radial split rotation'.
- `cut_fractal.time` (unlimited_float 0.0 -100.0 100.0): radians, factor 1, with fixed offsets 1.5 / 1.8 rad.
- `cut_circdes.time` (unlimited_float 0.0 -100.0 100.0): animation phase, factor 1.
- `cut_snowflake.time` (unlimited_float 0.0 -100.0 100.0): phase = 0.4·time rad.
- `dc_mandelbox2D.time` (unlimited_float 0.0 -10000.0 10000.0): phase = 0.1·time rad. Animation time param.
- `glsl_acrilic.time` (unlimited_float 0.0 -10000.0 10000.0): phase in radians with factor 1 and factor 1/p2. Animation time param.
- `glsl_circlesblue.time` (unlimited_float 1.0 -10000.0 10000.0): mixed: several phase rates plus a linear scroll. Animation time param.
- `glsl_circuits.time` (unlimited_float 0.0 -10000.0 10000.0): rotation angle = π·t with t ≈ 2·rate + 0.01·rate·time (piecewise folded); 'rate' also feeds the angle. Animation time param.
- `glsl_starsfield.time` (unlimited_float 0.0 -10000.0 10000.0): rotation by time rad, but also a period-1 layer phase and hash input. Animation time param.
- `glsl_grid3D.time` (unlimited_float 200.0 1.0 1000.0): rotation = 0.021·time rad plus a linear camera move. Animation time param.
- `glsl_mandelbox2D.time` (unlimited_float 0.0 -10000.0 10000.0): phase = 0.1·time rad. Animation time param.
- `glsl_kaliset.time` (unlimited_float 200.0 1.0 1000.0): two different angle rates (7.2/603 and 7.2/407.4 rad per unit) plus zoom. Hint says 'time/603 · 2π' but code uses 7.2, not 2π (minor hint mismatch).
- `glsl_kaliset2.time` (unlimited_float 85.5 -10000.0 10000.0): phase = 0.03·time rad. Animation time param.
- `glsl_apollonian.time` (unlimited_float 0.0 -10000.0 10000.0): several rates (0.05, 0.1, 0.3 rad per unit). Animation time param.
- `glsl_kaleidoscopic.time` (unlimited_float 0.0 -10000.0 10000.0): rotation = 0.1·time rad (only when radial = 1). Hint 'angle += 0.1·time' matches.
- `glsl_kaleidocomplex.time` (unlimited_float 0.0 -10000.0 10000.0): complex multiply by (sin t, cos t) = rotation by (π/2 - time) rad; stored units radians.
- `glsl_squares.time` (unlimited_float 85.5 -10000.0 10000.0): radians in trig but also a period-1 zoom phase. Animation time param.
- `glsl_hoshi.time` (unlimited_float 10.0 0.0001 10000.0): mixed rates and a 10/time term; not a usable angle. Animation time param.

## Turns, medium or low confidence

- `cpow3_wf.spread` (unlimited_float 1.0 -10.0 10.0): a count of full-turn branches (also 0.5/spread in cos(ai*inv_spread)); behaves like a branch count, probably not one to show in degrees
- `rhodonea.cycle_offset` (unlimited_float 0.0 -10.0 10.0): Turns added to atan2, then multiplied by `cycles` (the rose rotates by cycle_offset*2pi*cycles in t). Matches the hint "fractions of 2pi".
