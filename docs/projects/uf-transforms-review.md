# Ultra Fractal transformations vs. our flame variations

Reviewed 2026-10-18 for the KIFS plan's item 8
([escape-kifs-and-variations.md](escape-kifs-and-variations.md)), at the
user's request once C (Rhai variations, their hooks) was done: what UF's
transformations cover, and which our variations already do. UF
transformations themselves stay unsupported (UF support is paused,
[escape-formula-language.md](escape-formula-language.md)); the maps
worth having are candidates for Rhai variations
([rhai-plugins.md](rhai-plugins.md)). The library was UF's public
formula download, read as text for measurement only and never bundled;
nothing here is UF code, only descriptions and the maths.

**What was read.** The 24 `.ulb` class libraries (UF's public formula database, downloaded to `output/uf-public/files/`, read as text only; nothing in them was executed) and our variation registry, `src/variations/defs/*.rs` (647 `VariationDef`s). For every UF class with a plausible counterpart I read our def's doc comment and WGSL body rather than matching on names.

**How transform classes were found.** I listed every `class` declaration in the 24 files with its parent. That is 3,284 declarations, matched case-insensitively because reb.ulb writes `Class`. I then resolved inheritance across files. An unqualified parent names a class in the same file, falling back to common.ulb, and names are compared case-insensitively. A class counts as a transform when its chain reaches `common.ulb:Transform`.

**Counts.** 192 classes reach `Transform`:

- the 4 bases: `Transform`, `UserTransform`, `UtilityTransform`, `ClipShape`;
- 174 descendants of `UserTransform`, one of which (`MMF_TrapTransformZold`) is an untitled abstract base;
- 8 descendants of `UtilityTransform`;
- 6 descendants of `ClipShape`.

187 of them have a `title`. They sit in 11 of the 24 files:

| File | Classes |
|---|---|
| jlb.ulb | 71 |
| thm.ulb | 31 |
| mmf.ulb | 24 |
| dmj5.ulb | 17 |
| common.ulb | 13 |
| reb.ulb | 12 |
| mt.ulb | 11 |
| om2.ulb | 7 |
| om.ulb | 4 |
| aho.ulb | 1 |
| ck.ulb | 1 |

The other 13 files (dpb, ea, eew, ejb, fwp, ilc, kcc5, mhb, phs, rds, rkb, sam, tma) contain no Transform descendants. jlb.ulb's `DualTransform`, `Distance` and `FT` families derive from `Generic`, not `Transform`, so they appear only as parameters. The earlier estimate of about 165 is low: all 192 are listed below.

**Conventions.**

- **Kind.**
  - `map` is a pure point-to-point map with parameters.
  - `map+` is a map that also needs random numbers, screen or view values (`#screenpixel`, `#width`, `#center`, `#magn`, ...), state carried across calls (per-iteration counters, `zold`, values captured in `Init`), an image file, or an open-ended class as a parameter (any UserTransform, Distance, Formula or TrapShape slot).
  - `utility` covers bases, clipping and solid marking, merging, location and handles helpers, coordinate conversion and identity.
  - A fixed-count loop inside one call (n fold steps, n attractor steps) still counts as `map`, because a variation can loop too.
  - A helper class that only picks a fixed formula from a menu counts as a parameter, so the transform stays `map`. This covers jlb.ulb's `FT` function plug-ins, `CD` curve plug-ins, the `PM`/`Combine` operators, and UserTransform slots marked `selectable = false`.
- **Ours.**
  - `equivalent` means the same map once the xform supplies its pre- and post-affine and its weighted variation sum (a flame xform outputs the sum of w_i * V_i(p)). `linear` below means "the xform's affine does it".
  - `close` means a variation (or that structural feature) reproduces part of the map, and the note says what differs.
  - `none` means nothing in the registry computes it. The nearest relative is given where it helps.
- **UF notation in the sources.** `|z|` is the squared modulus, and `abs()` of a complex number is taken per component. The formulas below write r for the modulus and avoid the pipe character.

---

## aho.ulb (1). Author unknown: no name in the file header or class comments

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| aho_ClipShapeTarget (1154) | Crosshairs | unknown | Marks points solid on concentric rings (inner to outer radius in steps) and on an optional crosshair around a centre; returns z unchanged | utility (clip) | - |

## ck.ulb (1)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| Image (703) | Image | unknown (other ck.ulb classes credit c.Kleinhuis; this one has no comment) | z + scale * luminance of an image sampled at the screen pixel (#screenpixel / #width, #height) | map+ (image file, screen coordinates) | none |

## common.ulb (13). The file header credits Damien M. Jones, Frederik Slijkerman, Jim Blue, David Makin, Ron Barnett "and others"

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| Transform (2288) | (none) | header (several) | Abstract base: Init/Iterate/IsSolid, returns z | utility (base) | - |
| UserTransform (2443) | (none) | header (several) | Abstract base for user-selectable point transforms | utility (base) | - |
| UtilityTransform (4046) | (none) | header (several) | Abstract base for internal helpers | utility (base) | - |
| ClipShape (4071) | (none) | header (several) | Abstract base for clip shapes (solid inside/outside, handles, aspect) | utility (base) | - |
| NullTransform (4161) | Null Transform | header (several) | Identity | utility | - |
| TransformMerge (4181) | Transform Merge | header (several) | Chains up to 10 user transforms in order; solid if any is | utility (merge) | - |
| TrapTransform (4401) | Trap Position | header (several) | (z - centre) * rotation / scale, then aspect and skew; per-iteration drift, rotation and skew steps; the centre can follow the first z | map+ (per-iteration state) | close: linear (the static case is an affine; the steps need per-call state) |
| Location (4508) | Location | header (several) | The Location tab as a transform: skew, stretch, rotation, magnification, centre | map | equivalent: linear |
| PixelCoordinate (4569) | Pixel Coordinate utility class | header (several) | Fractal coordinate to screen pixel (uses #width, #height, #center, #magn, #angle) | utility (coordinates) | - |
| FractalCoordinate (4644) | Fractal Coordinate utility class | Jim Blue (class comment) | Screen pixel to fractal coordinate | utility (coordinates) | - |
| Handles (4723) | Handles utility class | header (several) | Draws handle markers (points, lines, circles) as solid flags, sized in pixels | utility (handles) | - |
| NullClipShape (6521) | Null Clip Shape | header (several) | No clipping | utility (clip) | - |
| ClipShapeMerge (6541) | Clip Shape Merge | header (several) | Combines several clip shapes | utility (clip) | - |

## dmj5.ulb (17). Damien M. Jones (file header)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| DMJ_Clipping (4720) | Über Clipping | Damien M. Jones | Clipping framework: screen-relative option, offset, a ClipShape plus Handles; sets solid inside or outside; returns z | utility (clip) | - |
| DMJ_Distort (4848) | Distort (fBm and more) | Damien M. Jones | z += v * d * strength. d comes from a TrapShape (default fBm noise) through its own transform and transfer. v is radial from a centre or a fixed direction, and its angle can come from a second texture | map+ (TrapShape, Transfer, transform objects) | none (no noise-displacement variation) |
| DMJ_FastMosaic (5033) | Mosaic | Damien M. Jones | Voronoi mosaic. Moves z toward the nearest of N points produced by a Formula (default random), with tile scale and shift, edge stretching and solid edges or tiles | map+ (point set from a Formula class) | close: voron (hash-placed sites per grid cell and a lerp toward the nearest; no formula-made sites, edge modes or tile shift) |
| DMJ_FollowCurve (5238) | Follow Curve (BETA) | Damien M. Jones | z becomes (arc length along a user-drawn PolyCurve, signed distance from it) | map+ (curve object, handles) | none |
| DMJ_Inversion (5376) | General Inversion | Damien M. Jones | Inverts (z - c), rotated and aspect-scaled, in an ellipse, hypercross, flower (rose), lines or one-axis curve, with a general complex exponent | map | close: spherical (the ellipse curve with exponent -1 and aspect 1 is spherical plus affines; the other curves and exponents are new) |
| DMJ_Kaleidoscope (5488) | Kaleidoscope | Damien M. Jones | Polar angle reduced mod 2pi/n about a centre, in reflective, left, right or slice-only mode, with rotations before and after | map | none (a deterministic fold; multi_kaleidoscope and post_point_symmetry_wf give the same symmetry by plotting copies instead) |
| DMJ_LinearWave (5594) | Linear Wave | Damien M. Jones | In a frame rotated by the wave angle, z += strength * sin(x * f/pi + phase/pi) along a chosen displacement direction | map | close: waves2 (displacement perpendicular to the wave is waves2's single-axis term; adds the wave and displacement angles and the phase) |
| DMJ_LogSpiral (5665) | Log Spiral | Damien M. Jones | t = tightness * ln r + arg/2pi; output (floor(t) - arg/2pi) * a + i * frac(t) * d + offset (log-spiral strip coordinates) | map | close: log (log-polar followed by a shear and a fractional wrap) |
| DMJ_LowRes (5724) | Low Resolution | Damien M. Jones | Rounds x and y, in a rotated frame, to a grid with separate x and y resolutions | map | equivalent: blur_pixelize (scale 0 snaps to cell centres; the half-cell offset, per-axis resolution and rotation go in the xform's affines) |
| DMJ_PolarToRectangular (5773) | Polar to Rectangular | Damien M. Jones | Reads (x, y) as (r, theta) and returns r * e^(i theta) + centre | map | close: invpolar (axes swapped, angle in units of pi, radius offset by 1; the affines absorb all three) |
| DMJ_RectangularToPolar (5811) | Rectangular to Polar | Damien M. Jones | z becomes (modulus of z - c, arg(z - c)) | map | close: polar (swapped and scaled; ours measures the angle from the y axis, so its branch cut sits elsewhere) |
| DMJ_TransformRepeat (5849) | Repeat Transform | Damien M. Jones | Applies a user transform n times; each step can be damped (auto-scale), rotated (with a periodic rotation reset) and offset | map+ (UserTransform slot) | none |
| DMJ_TrapTiling (5943) | Trap Tiling | Damien M. Jones | Rectangular tiling (centred wrap by the spacing), polar tiling (angle * 4/pi and radius wrapped) or radial tiling (the polar wrap mapped back to the plane) | map | close: modulus (rectangular mode is modulus with limits = spacing/2 and period = spacing; the polar and radial modes are new) |
| DMJ_ScreenRelative (6018) | Screen Relative | Damien M. Jones | Replaces z with #screenpixel scaled to 0..1 when enabled | utility (coordinates) | - |
| DMJ_ClipShapeCircle (6077) | Circle | Damien M. Jones | Circle clip shape | utility (clip) | - |
| DMJ_ClipShapePolyCurve (6273) | PolyCurve Family | Damien M. Jones | PolyCurve clip shape | utility (clip) | - |
| DMJ_ClipShapeRectangle (6355) | Rectangle | Damien M. Jones | Rectangle clip shape | utility (clip) | - |

## jlb.ulb (71). Author unknown: the file header and class comments name no author (line 269 cites "Blue, James L., NIST" as a reference for a random-number routine)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| JLB_QuadMirror2 (2320) | QuadMirror2 new | unknown | Picks one block of an nx x ny screen grid and repeats it, mirrored horizontally and vertically, in screen-pixel space | map+ (#screenpixel, #screenmax) | none (nearest: modulus, which repeats without mirroring) |
| JLB_MoveCenter (3268) | MoveCenter | unknown | Translates the fractal so a chosen screen position becomes the centre; optional screen-rectangle clip | utility (location, clip) | - |
| JLB_TwoDistances (4611) | Two Distances | unknown | z becomes (Dx(z), Dy(z)) from two Distance plug-ins, optionally swapped or alternating per call | map+ (Distance classes, per-call state) | none |
| JLB_FTransform (4691) | Function - Menu | unknown | One complex function from a 60-entry menu: trig, hyperbolic and their inverses, sec/csc families, sqr, cube, sqrt, log, loglog, exp, abs, conj, flip, recip, ceil/floor/round/trunc, power, gd, logit/expit, gauss, softplus, sinc, decay, z/log z, capped identity | map | close: about 20 entries are our own variations (sin, cos, tan, sec, csc, cot, sinh, cosh, tanh, sech, csch, coth, exp, log, arcsinh, acosh, arctanh, acoth, acosech, arcsech, cpow); none for asin, acos, complex atan, gd, logit, expit, gauss, softplus, sinc, decay or the rounding entries |
| JLB_BubbleTransform (4827) | Bubble | unknown | Inside a superellipse of radius r0 about a centre, z becomes c2 + s * (r/r0)^p * (z - c) * rotation (a local magnifier or transfer), with linear, exponential or Gaussian fade outside | map | none (our bubble and fisheye are global radial maps) |
| JLB_Waves (5191) | Waves | unknown | z += a * F(phase + fx * x + fy * y) * e^(i * direction), optional function tweaks per axis, optional radial fade | map | close: waves2 (here one plane wave in any direction, with phase, a function choice and a fade; waves2 is two axis-aligned sine terms) |
| JLB_PolynomialUserTransform (5563) | Polynomial Transform | unknown | Complex polynomial of degree up to 6 (Horner form), or z^p | map | close: cpow (power mode only; nothing for the polynomial) |
| JLB_UT_Function (5666) | Function - Plug-in | unknown | f(z) from an FT function plug-in (the same set as the menu) | map | close: as Function - Menu |
| JLB_UT_Tweak (6861) | Tweak Transform | unknown | n steps of z = z + e1 + e2 * z (or replace z), or e1 + e2 * T(a * z), or round to n decimals | map (Transform mode takes a UserTransform) | close: linear (Linear mode is affine; Round mode is a grid snap like blur_pixelize) |
| JLB_UT_Tweak2 (6972) | Tweak2 | unknown | e1 + e2 * z, or e1 + e2 * T(z) | map | close: linear (Linear mode) |
| JLB_UT_Basic (8643) | Basic Transform | unknown | a1 + a2 * f(a3 * z), f from the function plug-in | map | close: our function variations plus affines (equivalent wherever f is one we have) |
| JLB_UT_Linear (8696) | Linear Transform | unknown | a + b * z, or a + b * T(z) | map | equivalent: linear (complex affine) |
| JLB_UT_Combo (11093) | Transform Combo | unknown | Up to three user transforms applied to x and to y separately (real parts), combined by + - * / ^ or composition | map+ (UserTransform classes) | none |
| JLB_UT_CD (12574) | X and Y Distances | unknown | z becomes (+/-Dx(z), +/-Dy(z)) from two Distance plug-ins | map+ (Distance classes) | none |
| JLB_UT_Formula (12838) | Formula Transform | unknown | One iteration of a Formula or FormulaX class, used as a map | map+ (Formula class) | none |
| JLB_UT_Curves (14760) | Two Curves | unknown | z becomes (Cx(z), Cy(z)), each the implicit polynomial of a named algebraic curve (about 80: Arachnea, Astroid, Bicorn, ...) | map | none |
| JLB_UT_XY_Signs (15750) | X+Y Signs | unknown | New (x, y) from +/-x, +/-y, +/-abs(x), +/-abs(y), with an optional swap | map | close: linear (the sign and swap modes are affine; the abs-fold modes have no match) |
| JLB_UT_DucksPlus (15820) | DucksPlus | unknown | Rotates z into the base sector of an n-fold rotation (round(arg/delta)), applies X+Y Signs, then f(z^p) (default log). After Monnier's Ducks | map | none |
| JLB_UT_Shape (15902) | Shape Transform | unknown | z + m * d * (z - c), d the signed distance to a shape plug-in, with decay inside and outside | map+ (DistancePlus class) | none |
| JLB_UT_Rotate (15985) | Rotate | unknown | Rotation about a centre | map | equivalent: linear |
| JLB_UT_Radial (16056) | Radial Transform | unknown | Radial expand or contract about a centre within radius r0, with an abrupt or smooth (power, tanh) taper | map | none (nearest: circus, which scales inside and outside the unit circle with a hard step) |
| JLB_UT_GnarlZ (16154) | Gnarl Z | unknown | n gnarl steps on x and y (x += h * f1(y^p + f2(a(y^q + f3(b y)))), y likewise) plus a damped complex-z gnarl term | map | none (nearest: popcorn2) |
| JLB_UT_OneOrTwoFunc (16422) | 1 or 2 Functions | unknown | f(a * z^p) (four placement options) alone, or combined with g(b * z^q) by + - * / ^, composition or alternation; optional reciprocal | map ("alternate" needs per-call state) | none (f + g is two variations summed in one xform; the other combinations are new) |
| JLB_UT_OneOrTwoFunc2 (16702) | 1 or 2 Functions #2 | unknown | f of a sign/argument option of z (+/-x +/- iy or +/-x +/- y), combined with an optionally flipped g of another option by + - * / | map | none |
| JLB_UT_OneOrTwoUT (16983) | 1 or 2 Transforms | unknown | T1(a * z^p) alone, or combined with T2(b * z^q) by + - * / ^ or composition | map+ (UserTransform classes) | none |
| JLB_UT_Distance (17209) | Distance | unknown | d * z, d * f(z) or f(d * z), d from a Distance plug-in | map+ (Distance class) | none |
| JLB_UT_OneFunction (17354) | 1 Function (Plug-in) | unknown | f(z) from an FT plug-in | map | close: as Function - Menu |
| JLB_UT_OneFunctionM (17417) | 1 Function (Menu) | unknown | f(z) from the function menu | map | close: as Function - Menu |
| JLB_UT_PowerPlusFunc (17551) | Power + Function | unknown | z^p + a * f(b * z) | map | close: cpow plus a function variation in one xform (the variation sum; a must be real) |
| JLB_UT_ZZZ (18306) | ZZZ | unknown | a * S1(z)^p combined with b * S2(z)^S2(z) (a z^z term), where S is a sign, swap or reciprocal option; combined by larger, smaller, sum, difference, product or a mean | map | none |
| JLB_UT_Discrete (18500) | Discrete | unknown | Compresses each coordinate by log base b, rounds (round, ceil, floor, trunc), expands again: a log-spaced grid snap | map | none (nearest: blur_pixelize, a linear grid) |
| JLB_UT_Sierpinski (18576) | Sierpinski +/- | unknown | n steps of z = factor * z, subtracting or adding delta per coordinate past the boundaries (Sierpinski-trap style); returns z - z_n | map (internal loop) | none |
| JLB_UT_Doodads (19054) | Doodads +/- | unknown | About 30 trap modes x 13 flavours of composite formulas in the modulus and angle of g, the modulus of z and so on, through six function plug-ins, then a power | map | none |
| JLB_UT_TriFunc (19335) | Trifunctional +/- | unknown | f(a * fx(+/-x) + i * b * fy(+/-y)), optional swap | map | none |
| JLB_UT_Harlequin (19434) | Harlequin +/- | unknown | f(p2 +/- (m * arg(z)^2 + p1), squared or not): a function of the angle only | map | none |
| JLB_UT_Martin (19510) | Martin +/- | unknown | n iterations of the Martin map (x = y - f2(x), y = p - x) starting from f1(z)/scale, then f3 | map (internal loop) | none |
| JLB_UT_Vine (19649) | Vine +/- | unknown | n steps of x += a * f(y), y += a * f(x), plus swapped and complex variants and an optional secondary function, between an initial and a final function | map (internal loop) | none (nearest: popcorn2) |
| JLB_UT_PointLine (19965) | Point and Line | unknown | Vectors from z to two points or lines, each through a function, combined (larger, smaller, sum, product, means, ...) | map | none |
| JLB_UT_TwoUT (20116) | Two Transforms | unknown | T2(T1(z)), or a combination of T1(z) and T2(z) | map+ (UserTransform classes) | none |
| JLB_UT_Null (20196) | None | unknown | Identity | utility | - |
| JLB_UT_Inverse (20210) | Inverse | unknown | c + (R/(z - c))^p | map | close: spherical (p = 1 is spherical plus a y flip; the complex power is extra) |
| JLB_UT_Moebius (20263) | Moebius | unknown | (a0 + a1 * Z)/(b0 + b1 * Z), Z = z or T(z), optionally combined with Z | map | equivalent: mobius (Z = z) |
| JLB_UT_Moebius2 (20367) | Moebius2 | unknown | (z - p)/(z + p), or (a z + b)/(c z + d) | map | equivalent: mobius |
| JLB_UT_TwoFunc (21152) | 2 Functions | unknown | (f(a x) op g(b y)) + i * (g(a x) op f(b y)) | map | none |
| JLB_UT_Sine (21391) | Sine +/- | unknown | Combines (ha * f3(ht * Im f1(z)), va * f3(vt * Re f2(z))) with a polar wave (r + ha * sin(ht * theta), theta + va * cos(vt * r)) | map | none |
| JLB_UT_GnarlXY (21625) | GnarlXY | unknown | n steps of x += h * f1(y + f2(a(y + f3(b y)))), y likewise | map (internal loop) | none (nearest: popcorn2) |
| JLB_UT_Ident (22268) | Ident | unknown | Identity | utility | - |
| JLB_UT_AB (22286) | Above/Below | unknown | a +/- b * z, picked (or tanh-blended) by whether a distance value is below a threshold | map | none |
| JLB_UT_2Powers (22807) | Two powers | unknown | z^p1 + a * z^p2 | map | close: two cpow variations in one xform (real a only) |
| JLB_NewtonUserTransform (28502) | Newton | unknown | One relaxed Newton step for z^p = r (standard: (2z^3 + 1)/(3z^2)) | map | none |
| JLB_SpiroGlyphicUserTransform (28566) | Spiroglyphic | unknown | The Spiroglyphics step from tma2.ufm: nested sin/cos iterations of swapped x and y with sign patterns, then initial and final functions | map (internal loop) | none |
| JLB_MoebiusUserTransform (28887) | Moebius | unknown | (a0 + a1 * v)/(b0 + b1 * v), or the same with F(a2 * v) and F(b2 * v) | map | equivalent: mobius (v mode) |
| JLB_NullUtilityTransform (29302) | (No change) | unknown | Identity | utility | - |
| JLB_NewtonUtilityTransform (29328) | Newton | unknown | The same Newton step, on the UtilityTransform base | map | none |
| JLB_SpiroGlyphicUtilityTransform (29387) | Spiroglyphic | unknown | The same Spiroglyphics step, on the UtilityTransform base | map | none |
| JLB_PowerUserTransform (29948) | Power | unknown | a1 + a2 * (a3 * z + a4)^p | map | equivalent: cpow (the complex factors before and after go in the xform's affines) |
| JLB_DoubleUserTransform (30002) | Double UserTransform | unknown | T2(T1(z)), or n gnarl steps driven by T2(T1(.)) on swapped axes | map+ (UserTransform classes) | none |
| JLB_BasicUserTransform (30095) | Basic | unknown | a1 + a2 * f(a3 * z + a4), f a UF built-in function | map | close: our function variations plus affines (equivalent where f is one we have; the default sqr is cpow with power 2) |
| JLB_NewtonPolyUserTransform (30155) | Newton Poly | unknown | One Newton, Halley-type or Laguerre step for a polynomial of degree up to 10, or for z^p, minus a constant or #pixel | map (the #pixel option is map+) | none |
| JLB_QuadMirror (30470) | QuadMirror | unknown | Older version of QuadMirror2: mirrored repetition of a screen block | map+ (#screenpixel) | none |
| JLB_LinearUserTransform (34327) | Linear | unknown | a + b * z | map | equivalent: linear |
| Tweak (34357) | Tweak | unknown | z + e1 + e2 * {z, F(a z), T(a z)}, or the same replacing z (UtilityTransform base) | map (Transform mode takes a UserTransform) | close: linear (Linear mode) |
| JLB_TwoUserTransforms (34451) | Two Transforms | unknown | T1(T2(z)), or T1(z) op T2(z) | map+ (UserTransform classes) | none |
| JLB_ComboUserTransform (34509) | Combo | unknown | F1 op F2 or a composition, each a built-in function or a user transform | map (function slots; the transform slots are map+) | none (F1 + F2 is two variations summed in one xform) |
| JLB_RotateUserTransform (34604) | Rotate | unknown | Rotation | map | equivalent: linear |
| JLB_OverUnderUserTransform (34646) | OverUnder | unknown | One of two transforms (or a tanh blend) by whether a test value of z exceeds a threshold | map+ (UserTransform classes) | none |
| JLB_ComboTransform (35328) | Combo Transform | unknown | T(a z), T(a z op #pixel), or T1(a z) op T2(b z) | map+ (UserTransform, #pixel) | none |
| JLB_Combo2Transform (36407) | Combo T | unknown | Per-axis T1/T2/T3 combinations, as in Transform Combo | map+ (UserTransform classes) | none |
| JLB_UT_XY (37411) | X and Y Transforms | unknown | Signed real or imaginary parts of Tx(z) and Ty(z), added to z or replacing it | map+ (UserTransform classes) | none |
| JLB_ComboFunction (37481) | ComboF | unknown | Menu functions applied to x and y as reals, combined as in Transform Combo | map | none |
| JLB_FuncsUserTransform (37713) | Funcs | unknown | x' = a * Fx(b * argx(z)), y' = a * Fy(b * argy(z)); the arg comes from a menu (modulus, x+y, x*y, max, angle, ...); n steps, added or replacing | map+ (UserTransform slots) | none |

## mmf.ulb (24). David Makin (file header)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| MMF_TrapTransformZold (8370) | (none) | David Makin | Abstract base that keeps zold and zolder for trap transforms | utility (base) | - |
| MMF_TrapTransformInterpolation (8439) | Interpolation Transform | David Makin | Fits a * zold^p1 + b * zold^p2 to recent orbit values and returns a function of the fit (or of the predicted z): an orbit-history trap transform | map+ (zold and zolder across calls) | none |
| MMF_TransformMod (22370) | Transform Modulus | David Makin | (scale * z) % modulus per axis, then a following transform | map+ (Transform slot) | close: modulus (a per-axis remainder; the following transform is a separate step) |
| MMF_GeneralTransform (22417) | General Transform | David Makin | ((f((z + o) * s + o1) + o2) * s1 + o3), f a built-in function | map | close: our function variations plus affines |
| MMF_SelfRotation (23430) | Self Rotation | David Makin | z * (z/r)^d: rotates z by d times its own angle and keeps the modulus | map | none (cpow also raises the modulus) |
| MMF_RunningSum (23469) | Running Sum | David Makin | Running sum of the sequence | map+ (state across calls) | none |
| MMF_RunningProduct (23512) | Running Product | David Makin | Running product of the sequence | map+ (state across calls) | none |
| MMF_PowerFunctions (23569) | Power Functions | David Makin | f(z)^p | map | close: cpow (f = ident; any other f needs a function variation first) |
| MMF_SimpleMatrixTransform (23600) | Simple Matrix Transform | David Makin | 2x2 real matrix | map | equivalent: linear |
| MMF_OffsetMatrixTransform (23640) | Offset Matrix Transform | David Makin | Offset, then a 2x2 matrix | map | equivalent: linear |
| MMF_CrossProduct (23687) | Cross Product | David Makin | 2D cross products of z with zold or zolder, or of their differences | map+ (zold and zolder across calls) | none |
| MMF_UserFormulaTransform (23741) | User Formula Transform | David Makin | Sum of up to 10 terms c_k * f_k(z)^p_k | map | close: the xform's variation sum (each term needs a function and a power; real weights only) |
| MMF_TransformAdd (24009) | Transform Add | David Makin | T1(z) + T2(z) | map+ (Transform classes) | close: structural, two variations summed in one xform |
| MMF_TransformSubtract (24051) | Transform Subtract | David Makin | T1(z) - T2(z) | map+ (Transform classes) | close: structural, the variation sum with a negative weight |
| MMF_TransformMultiply (24098) | Transform Multiply | David Makin | T1(z) * T2(z) | map+ (Transform classes) | none |
| MMF_TransformDivide (24142) | Transform Divide | David Makin | T1(z) / T2(z) | map+ (Transform classes) | none |
| MMF_TransformSum (24193) | Transform Sum | David Makin | Sum of up to 10 c_k * T_k(z), complex c_k | map+ (Transform classes) | close: structural, the variation sum (real weights) |
| MMF_TransformProduct (24434) | Transform Product | David Makin | Product of up to 10 T_k(z)^p_k | map+ (Transform classes) | none |
| MMF_TransformPowerFunctions (24675) | Transform Power Functions | David Makin | f(T(z))^p | map+ (Transform class) | none |
| MMF_FormulaTransform (24717) | Any Formula | David Makin | One iteration of any main formula, used as the transform | map+ (Formula class) | none |
| MMF_Constant (24942) | Constant Value | David Makin | Returns a constant | map | equivalent: linear (an affine with a zero matrix) |
| MMF_Scale (24970) | Simple Scale | David Makin | s * z (complex) | map | equivalent: linear |
| MMF_Linear (24998) | Linear | David Makin | s * z + c | map | equivalent: linear |
| MMF_GammaTransform (25032) | Gamma Transform | David Makin | Complex Gamma(z) (Math.ComplexGamma) | map | none (our `gamma` is lgamma of the modulus with the angle kept: a different map) |

## mt.ulb (11). Mark Townsend (file header and class comments)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| MT_TextureTransform (1026) | Texture Position | Mark Townsend | Centre, scale, rotation, skew and aspect (Trap Position without the steps) | map | equivalent: linear |
| MT_RadialWaves (3124) | Radial Waves | Mark Townsend | n iterations of r += a * sin(t * theta), theta += a2 * cos(t2 * r) about a centre | map (internal loop) | none |
| MT_PopcornTransform (3180) | Popcorn | Mark Townsend | n steps of Pickover's popcorn: x -= h * f1(y + f2(a y)), y -= h * f3(x + f4(a' x)), default sin and tan | map (internal loop) | close: popcorn2 (one step of x + c * sin(tan(c y)): no "+ y" inside the sine, fixed functions) |
| MT_BarnsleyTransform (3248) | Barnsley | Mark Townsend | n iterations of Barnsley's z = (z -/+ 1) * c, chosen by a half-plane test | map (internal loop) | none |
| MT_GravitationalLens (3287) | Gravitational Lens | Mark Townsend | r becomes r - a^2/r about a centre (point-mass lens), angle kept | map | equivalent: linear + spherical in one xform (weights 1 and -a^2, centre via the affines) |
| MT_CrazySpiral (3322) | Crazy Spiral | Mark Townsend | theta += a * f(r) + amp * sin(b * theta) | map | close: swirl / swirl3 (f = sqr or f = log with amp 0; the angular sine term is extra) |
| MT_WheelTransform (3369) | Wheel | Mark Townsend | Inside an annulus, theta += f(distort * d/width), d the distance to the nearer rim | map | none (nearest: whorl) |
| MT_GnarlTransform (3428) | Gnarl | Mark Townsend | n steps of x -= h * f1(y + f2(a(y + f3(b y)))), y += h * f1(x + ...) | map (internal loop) | none (nearest: popcorn2) |
| MT_GnarlyTransform (3496) | Gnarly Transformation | Mark Townsend | Moves z by strength * (z - the end point of n steps of a Martin, Popcorn, Vine or Gnarl map) | map (internal loop) | none |
| MT_Scale (3614) | Scale | Mark Townsend | z / scale | map | equivalent: linear |
| MT_TrapShapeTransform (3640) | Trap Shape | Mark Townsend | z + strength * e^(i * 2 * pi * sqrt(2) * t), t a trap-shape value (default Perlin texture), or the shape's transformed point | map+ (TrapShape class) | none |

## om.ulb (4). Author unknown: no name in the file

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| OM_FunctionsTransform (17844) | Functions Transform | unknown | f3(f2(f1(z))) from three large menus (about 150 composite functions: sinc/cosc, exsec, Cayley-transform compositions, sec + cosec sums, ...), then p1 * z^pow + p2 | map | close: its simple entries are our function variations; nothing for the composites |
| OM_Moebius (18224) | Moebius | unknown | ((p1 * f(z) + p2)/(p3 * f(z) + p4))^pow | map | close: mobius (f = ident and pow = 1) |
| OM_MunsterMap (18263) | Munster Map | unknown | e^(i p2) * z^pow * B(z) + p3, B a Blaschke-type factor ((z - p1)/(1 - p1 z) and three variants) | map | none |
| OM_Polylog (18310) | Polylog | unknown | Li_(-n)(f(z)) for n = 1..10 (rational Eulerian-number forms), then p1 * z + p2 | map | none |

## om2.ulb (7). Author unknown: the comments only thank Barnett, Owens, Klein, Apollonius and others

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| OM_KleinTransform (1139) | Klein Transform | unknown | If z lies inside one of 3 (or 7) circles of a Klein-group packing, inverts it in that circle; then p1 * f(z) + p2 | map | none (nearest: klein_group, which draws a group's limit set by random generator choice) |
| OM_ApolloKleinTransform (7592) | Apollo Klein Transform | unknown | The 3-circle Klein inversion step and the 4-circle Apollonian step (3 circles plus the inner Soddy circle), in either order; circle centres pass through function menus | map | none (nearest: apollonian_gasket, apollony) |
| OM_ApolloKleinTransform2 (7979) | Apollo Klein Transform 2 | unknown | The 7-circle Klein step plus the Apollonian step | map | none |
| OM_ApolloKleinTransform3 (8364) | Apollo Klein Transform 3 | unknown | The 3-circle Klein step, then a Möbius (z + a)/(f z + 1 + a f) picked by a logistic-map index; all four entries are identical as written | map (the per-call state has no effect) | none |
| OM_ApolloTransform (8460) | Apollo Transform | unknown | The 4-circle Apollonian inversion step, then p1 * f(z) + p2 | map | none |
| OM_KleinTransform2 (9013) | Klein Transform 2 | unknown | The 3-circle and 7-circle Klein inversion steps, in either order | map | none |
| OM_ApolloTransform2 (10068) | Apollo Transform 2 | unknown | The Apollonian step, then the same identical-entry Möbius selection | map | none |

## reb.ulb (12). Ron Barnett (file header)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| TrapTransformMod (88353) | Trap Position Mod | Ron Barnett | Trap Position with aspect steps; drift, rotation, skew and aspect steps can be randomised | map+ (random numbers, per-iteration state) | close: linear (static case) |
| TobysMorph (88549) | Toby's Morph | Ron Barnett (after Toby Marshall's Morph) | + - * / ^ combinations of function-of-shifted-power terms f((z - t)^e), optionally of the modulus, arg, Re or Im of z | map | none |
| Monnier_HyperbolicTiling (89179) | Monnier Hyperbolic Tiling | Ron Barnett (after Samuel Monnier) | Folds a Poincaré-disc point into the fundamental domain of the {n,k} tiling by repeated rotation and circle inversion, then maps it (optional correction); solid outside the disc | map (internal loop; "One Tile" mode uses #screenpixel) | none (hypertile1/2 scatter copies across a {p,q} tiling: the opposite direction) |
| REB_attract_formula_tx (89389) | Attractor Transformations | Ron Barnett | n iterations of a 2D attractor map started at the point (Chip, CosMartin, Hopalong, New Hopalong, Pickover, Quadruptwo, Threeply, Latoocarfian, Zito, 100 Sprott quadratics); returns the end point or z plus a scaled displacement | map (internal loop) | close: clifford_js (one step of a Clifford map, about the Latoocarfian form; the other attractors and the iteration count are new) |
| REB_HyperbolicMapping (90140) | Hyperbolic Mapping | Ron Barnett | z / sqrt(R^2 - r^2) (bottom view) or z / sqrt(R^2 + r^2) (top view) inside radius R, solid outside, scaled by 4/#magn | map | close: hemisphere (the top view is hemisphere's xy, with R in the affines; the bottom view and the clip are new) |
| REB_Inversions (90203) | Circle Inversions | Ron Barnett | Refraction (Snell's law) through a circle configuration built by an InvertSphere object; uses #magn and #angle | map+ (helper object, view values) | none |
| REB_Mobius (90379) | Kleinian Inversions | Ron Barnett | The same refraction through the circles of a Kleinian (InvertMobius) configuration | map+ (helper object, view values) | none |
| Seven_Klein_Group (90558) | Seven Klein Group | Ron Barnett (modified from Morgan Owens' Fractint formula) | n iterations of "invert in whichever of 7 hexagonally packed circles contains z" | map (internal loop) | none (nearest: klein_group) |
| Push (90633) | Push | Ron Barnett | Inside the region x^2 + s * y^2 - p * y < scale - 1 about a touch point, scales (z - c) by (1 + d)/scale: a dent pushed into film | map | none (nearest: circus) |
| AppolonianGasket (90715) | Appolonian Gasket | Ron Barnett (modified from Morgan Owens' Fractint formula) | n iterations of inversion in whichever of 4 circles (3 given plus the inner Soddy circle) contains z | map (internal loop) | none (nearest: apollonian_gasket) |
| REB_Hyperparaboloid (90848) | Hyperparaboloid | Ron Barnett | Ray-casts the pixel onto the saddle x^2 - y^2 = 10z under a 3D rotation and translation and returns (u, v) texture coordinates; solid on a miss; uses #center and #magn | map+ (view values) | none |
| REB_IFSApollo (91052) | IFS Apollo | Ron Barnett | One of the three Apollony IFS maps. UF's RNG restarts from a fixed seed on every call, so every point gets the same branch, picked by the iteration count | map | close: apollony (the same three maps; ours draws a branch per sample, which is the actual IFS) |

## thm.ulb (31). Thomas Michels (file header)

| Class (line) | Title | Author | Computes | Kind | Ours |
|---|---|---|---|---|---|
| THMSwitchFormulaTransform (975) | TswFormulaTransform | Thomas Michels | One iteration, or n with a bailout, of a formula plug-in used as a transform | map+ (Formula class, state) | none |
| THMLambdaTransform (1117) | THMLambdaMapping | Thomas Michels | tz = tz * g(z) * (1 - g(z)), accumulated across calls starting from 0.5; returns f(tz) * p1 + p2 | map+ (state across calls) | none |
| THMFunTransform (1163) | THMFunMapping | Thomas Michels | f1(f2(z^p)) * p1 + c * p3, c the sequence's first value | map+ (Init value; a pure map when p3 = 0) | close: cpow, then a function variation (p3 = 0; f2 defaults to log) |
| THMTalisTransform (1210) | THMTalisTransform | Thomas Michels | f(log(abs(x) + i * abs(y))) * e^(i * p1 * arg z) | map | none |
| THMConjugationFold (2640) | ConjugationFold | Thomas Michels | Rotate and offset, then conj(z) where Im z > 0 (folds y to -abs(y)) | map | none (we have no abs-fold variation) |
| THMInvEllipseFold (2678) | InverseEllipseFold | Thomas Michels | Rotate, scale to an ellipse, apply a power, then Kali's inverse circle fold (where r^2 < k: w * k / r^2), then m * w + o | map | none (nearest: spherical) |
| THMKalisInvCircleFold (2740) | KalisInvCircleFold | Thomas Michels | Where r^2 < R: z * R / r^2; then m * z + o | map | none (nearest: spherical) |
| THMAbsFnSum (2782) | THMAbsFnSum | Thomas Michels | abs(f1(z) + f2(z)) per component, after rotation and scale | map | none |
| THMAbsReflection (2825) | Abs Reflection | Thomas Michels | z = abs(z); z = 2a - z; z = abs(z); z = 2b - z | map | none |
| THMKalisIterationFold (2857) | KalisIterationFold | Thomas Michels | n steps of z = t - abs(z - t) per axis, then x = -x, with t rescaled each step (Kali) | map (internal loop) | none |
| THMCircleFoldQ (3072) | CircleFoldQ | Thomas Michels | Optional pre-transform; where r > R: R^2 * z / r^2 (circle inversion of the outside); m * z + o | map (optional UserTransform slot) | none (nearest: spherical) |
| THMAsdamMTeleBrot (3196) | AsdamMTeleBrot | Thomas Michels | z - t * z / (f(x) + abs(y)); the default f = abs makes the denominator the L1 norm | map | none |
| THMTriangleFold (3452) | TriangleFold | Thomas Michels | Abs fold into the first quadrant; inside x + y < k swap x and y, outside invert (z / r^2) | map | none |
| THMTriangleAbs (3490) | TriangleAbs | Thomas Michels | Abs fold; inside x + y < k negate both coordinates | map | none |
| THMLinearpull (3602) | Linearpull | Thomas Michels | Meant to shift points with abs(x) > w and abs(y) > h by -/+2w and -/+2h; as written the shift uses uninitialised locals (0), so it is the identity | map | none |
| THMMirrorFold (3970) | MirrorFold | Thomas Michels | Reflects across y = -x and y = x into a wedge, optionally recursing through an extra transform k times (the counter resets only at Init) | map | none |
| THMElem90InverseFoldIn (4036) | Elem90InversFoldIn | Thomas Michels | Where r^2 > R: z = R/z; then m * z + o | map | none |
| THMElem90CircleFoldIn (4068) | Elem90CircleFoldIn | Thomas Michels | Where r > R: R^2 * z / r^2; then m * z + o | map | none |
| THMOuterEllipseFold (4154) | OuterEllipseFold | Thomas Michels | As InverseEllipseFold, but inverting the outside (r^2 > k) | map | none |
| THMKalisOuterCircleFold (4216) | KalisOuterCircleFold | Thomas Michels | Where r^2 > R: z * R / r^2; then m * z + o | map | none |
| THMRotationFold (4249) | RotationFold | Thomas Michels | Points with arg < 0 are reflected into the upper half-plane (y becomes abs(y)), then (z + o) * rotation | map | none |
| THMCircleFold (4337) | CircleFold | Thomas Michels | z = 2 - abs(z - 2) per component, then the Mandelbox sphere fold (r < R: z/R; r < 1: z / r^2) | map | none (nearest: dc_mandelbox2D and glsl_mandelbox2D, which are colour samplers, not maps) |
| THMBallFold (4369) | BallFold | Thomas Michels | abs(z) / r^2: Kali's ball fold, a Kaliset step without the constant | map | none (glsl_kaliset is a colour sampler) |
| THMBoxFold (4392) | BoxFold | Thomas Michels | Rotate and offset, the Mandelbox box fold per axis (x = abs(x + a) - abs(x - a) - x), then f(scale * z) | map | none |
| THMRadiusFold (4445) | RadiusFold | Thomas Michels | Where r > R: z = R/z; plus an offset | map | none |
| THMLogz (5252) | z=fn(log(-z)/log(z)) | Thomas Michels | f(scale * log(-w)/log(w)), w the rotated and offset z | map | none |
| THMImagFold (5330) | ImagFold | Thomas Michels | Rotate and offset, then (fx * x, fy * abs(y)) | map | none |
| THMRealFold (5434) | RealFold | Thomas Michels | Rotate and offset, then (fx * abs(x), fy * y) | map | none |
| THMSierpFold (5955) | SierpFold | Thomas Michels | One Sierpinski fold step: scale by f, and subtract f/2 from y if y > b, otherwise from x if x > b | map | none |
| THMknightyFold (6654) | KnightyFold | Thomas Michels | Reflections across y = -x and y = x (Knighty's Sierpinski folds), then f(z) | map | none |
| THMSaMMoebiusTransform (6702) | SaMMoebiusTransform | Thomas Michels | (a * z^p + b)/(c * z^p + d) | map | close: mobius (p = 1 is mobius; the power is extra) |

---

## Summary

**Kinds (192 classes):** 126 `map`, 44 `map+`, 22 `utility`. The utilities are 5 abstract bases, 7 clip classes, 4 identities, 3 coordinate converters, and one each for merging, handles and location.

**Coverage of the 170 maps (`map` plus `map+`):**

| | All 170 | The 126 pure maps | The 44 `map+` |
|---|---|---|---|
| equivalent | 18 (11%) | 18 | 0 |
| close | 40 (24%) | 32 | 8 |
| none | 112 (66%) | 76 | 36 |

Counting pure maps only, 18/126 (14%) are equivalent and 50/126 (40%) are equivalent or close.

The 18 equivalents are thin:

- 12 are plain affines (Location, the Rotate/Scale/Linear/Matrix/Texture Position classes, Constant), which the xform's own coefficients cover;
- 3 are Möbius (JLB Moebius, Moebius2, the old Moebius) = `mobius`;
- 1 is a complex power (JLB Power) = `cpow`;
- 1 is a grid snap (DMJ Low Resolution) = `blur_pixelize` at scale 0;
- 1 is a sum of two variations (MT Gravitational Lens) = `linear` + `spherical` in one xform.

Several "close" entries are structural: the transform-sum composers (Transform Add, Subtract, Sum) and the f + g combinators are what a flame xform's weighted variation sum already computes, but with real weights only. Products, quotients, powers of two maps, and compositions buried inside a function argument have no flame equivalent.

**`none` maps worth writing as Rhai variations** (new geometry, not variants of what we have):

1. **Circle-inversion folds for Kleinian and Apollonian packings.** Seven Klein Group, Appolonian Gasket, and om2's Klein/Apollo family (9 classes) all "invert in whichever circle contains z", n times. This is the deterministic, escape-time dual of our random-generator group variations (klein_group, apollonian_gasket, schottky_group). One variation with a circle-set mode covers all nine.
2. **A 2D fold toolkit** (thm.ulb): Mandelbox box fold, ball/sphere fold, Kali's inner and outer circle folds, the abs folds (Real/Imag/Conjugation/Rotation fold), triangle fold, Knighty/Mirror diagonal folds, Sierpinski fold, Kali iteration fold. We have no deterministic fold variation at all, and these are the 2D building blocks the KIFS plan needs.
3. **Monnier Hyperbolic Tiling.** Folds the Poincaré disc into the {n,k} fundamental domain; hypertile goes the other way.
4. **DMJ Kaleidoscope.** A reflective angular fold mod 2pi/n. Our symmetry variations (multi_kaleidoscope, post_point_symmetry_wf) plot copies instead, and our `kaleidoscope` is a half-plane mirror warp.
5. **DMJ General Inversion.** Inversion in hypercross, rose ("flower"), line and one-axis curves, with a general exponent; only the circle case is `spherical`.
6. **Newton / Newton Poly.** Newton, Halley and Laguerre steps of a polynomial: rational maps with basin structure. Nothing like it is in the registry (`chladni`'s Newton steps target a nodal line).
7. **OM Polylog and Munster Map.** Rational polylogarithm Li_(-n) maps and Blaschke-product disc maps, two conformal families we lack.
8. **MMF Gamma Transform.** Complex Gamma(z). Our `gamma` is lgamma of the modulus, a different map.
9. **MMF Self Rotation.** z * (z/r)^d, angle multiplication at a fixed modulus. A one-liner and genuinely new.
10. **The attractor and popcorn maps we lack**, from REB Attractor Transformations and mt.ulb: Hopalong, Martin/CosMartin, Chip, Quadruptwo, Threeply, Zito and the Sprott quadratics, plus Pickover's original popcorn/gnarl with sin(y + tan(a y)). Each is a one-step map that would sit beside clifford_js, henon and lozi; popcorn2 drops the "+ y".
11. **DMJ Distort.** fBm/Perlin displacement of the plane. We have no noise-displacement variation, though the WGSL Perlin used by dc_perlin already exists.
12. **A local lens family**: JLB Bubble, JLB Radial Transform, REB Push, MT Wheel. They magnify or twist inside a region with a smooth fade, which is a localized `circus` or `whorl`.

Lower value but new: JLB Two Curves (algebraic-curve polynomials as coordinates), ZZZ (z^z terms), MT Radial Waves / JLB Sine (iterated polar waves), JLB Discrete (log-spaced snap), and DMJ Log Spiral / Trap Tiling's polar modes.

**UF features with no direct counterpart in our model:**

- **The solid flag.** The 7 clip classes and Handles exist only to set it, and several maps also set it (Hyperbolic Mapping, Monnier, Hyperparaboloid, Mosaic). A flame variation can only move a point; the crop-style variations park or drop it instead.
- **Class-valued parameters.** UF transforms nest other transforms, Distance, Formula and TrapShape objects. 32 of the 44 `map+` take such a slot (15 of them in jlb.ulb); the other 12 need state, the screen or an image. Our structural answers are the variation sum (add), pre/post variation chains (compose), and xaos and final transforms. There is no product, quotient or power of two maps.
- **State across calls.** Per-iteration steps (Trap Position, Trap Position Mod), orbit history (MMF's zold-based trap transforms, Running Sum/Product, Cross Product), and accumulators (THMLambdaMapping) assume UF's per-pixel orbit. A flame's "sequence" is the chaos-game orbit, so these would mean something else there.
- **Screen and view values** (#screenpixel, #width, #height, #center, #magn, #angle): QuadMirror(2), MoveCenter, Image, Hyperparaboloid, Circle/Kleinian Inversions, and Hyperbolic Mapping's default scale.
- **UF's built-in `func` parameters.** A menu of about 40 complex functions appears all over jlb.ulb and om.ulb. Our variations each fix one function. A shared function-menu helper for Rhai variations would cover most of the function-combinator transforms at once.

**Surprises found while reading:**

- **IFS Apollo is not an IFS as written.** reb.ulb's IFS Apollo restarts its RNG from a fixed seed every call, so every point takes the same branch. Its three maps are exactly our `apollony`.
- **Two "random" choices are moot.** om2.ulb's Apollo Klein Transform 3 and Apollo Transform 2 pick among four identical Möbius entries, and their logistic index g * (1 - g), without the factor 4, decays toward 0 anyway.
- **thm.ulb's Linearpull is the identity as written.** Its shift uses uninitialised locals.
- **MirrorFold's recursion runs only on the first k calls of a sequence.** Its counter decrements each call and resets only in Init.
- **Several of our names mislead.** Our `kaleidoscope` (Will Evans) is not an angular fold, and our `gamma` is not Gamma. The glsl_*/dc_* Mandelbox and Kaliset variations colour sampled pixels rather than mapping the input, so none of them is an equivalent of thm.ulb's folds.
