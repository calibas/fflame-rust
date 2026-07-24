# Hyperbolic Camera — Extras

A camera in hyperbolic space has many degrees of freedom that simply don't exist in Euclidean space because changing your "view" is not just a matter of translating and rotating. Hyperbolic geometry has intrinsic effects that can be exposed as camera operations.

Status legend: ✅ implemented in `hyperbolic_camera` · 🔜 planned (see Pathing & Navigation) · 💤 deferred.

Terminology convention used throughout the app: **Euclidean** names a *geometry* (the flat space a group can act in — the Space enums), **Equidistant** names a *projection* (the azimuthal equidistant chart, radius = true distance). Each term appears in exactly one role.

---

## Implemented

### 1. Change the camera's model (projection) ✅

The same hyperbolic scene can be viewed through several models:

* Poincaré disk/ball ✅
* Klein disk ✅
* Hyperboloid model ✅ (Gans)
* Upper half-plane ✅
* Equidistant ✅ · Band ✅ · Equal-Area ✅ (hyperbolic Lambert azimuthal, r = 2 sinh(d/2))
* Gyrovector coordinates — nothing to add: Möbius gyrovector coordinates *are* the Poincaré ball

The interpolation slider (`Poincaré <---> Klein`, geodesics gradually straightening while angles gradually distort) **is the Hyperbolic Zoom param**: the azimuthal charts turned out to be one projective family R = k·2ρ/(1 + a·ρ²), and `h_zoom` slides `a` continuously through Klein (1), Poincaré (0), Gans (−1) and beyond. Out of scope: morphs to the non-azimuthal charts (Half-Plane, Band) — the boundary changes topology.

### 2. Change curvature ✅

`curvature` param: K = −0.01 … −100 instead of fixed K = −1. Realized as the radial rescale d′ = d/√(−K) about the observer, applied after the isometry. The camera experiences the world as more or less negatively curved:

* horizons moving inward/outward
* objects shrinking faster/slower
* parallel lines diverging differently
* different amounts of exponential area growth

This is impossible in Euclidean space because there is only one flat geometry.

### 3. Hyperbolic zoom ✅

`h_zoom` param: moves the ideal boundary instead of the field of view — continuously changes visual infinity and how much of hyperbolic infinity is compressed into the image, while the scale at the observer stays fixed. The viewer reveals more or less of the infinite plane without changing position. Past Gans, visual infinity sits at *finite* hyperbolic distance; content behind it is clipped by default, or wrapped into an inverted exterior corona with the `zoom_wrap` toggle (incorrect as a projection, striking as an effect).

### 9 + 14. Constant-distance shells / view by hyperbolic radius ✅

`shell_min` / `shell_max` params: hide everything outside the hyperbolic annulus (2D) or ball shell (3D) around the observer, measured in experienced distance (after isometry and curvature). The visible volume becomes a true hyperbolic ball or shell. Because

```
Volume(r) ~ e^(2r)   (H²)
Volume(r) ~ e^(3r)   (H³)
```

sweeping the shell outward tours the tiling's generations — dramatically more content per shell. (Fading rather than hard clipping is deferred with #10 — it needs density writing, a different feature axis.)

### 11. Curvature-compensated view ✅

"Let the user choose which property to preserve" — this is exactly the chart menu, now complete:

* preserve **angles** → Poincaré
* preserve **geodesic straightness** → Klein
* preserve **radial distances** → Equidistant
* preserve **areas** → Equal-Area

No projection preserves all of these simultaneously, so the out-model picker *is* the choice.

---

## Pathing & Navigation 🔜

**Priority: implement in the animation system first; Fly Mode later.** These features are about *paths and persistent camera state*, not per-frame projection — they belong to the system that owns time. The animation tracks already interpolate camera params; what's missing is composing isometries along a path instead of interpolating parameters independently.

Shared prerequisite: the camera's isometries are currently boosts + rotations (hyperbolic + elliptic). **Parabolic** isometries — the ones fixing a single ideal point — are the missing third class, and items 4, 5, and 12 all want them.

### 4. Ideal-boundary steering

Hyperbolic space has a natural boundary at infinity. A camera could target "look toward this point at infinity" rather than "look toward XYZ" — orientation specified by asymptotic directions. This doesn't exist in Euclidean space because infinity isn't a finite boundary.

### 5. Horocycle alignment

Instead of aligning to planes, align to horocycles or horospheres: camera frame tangent to a horosphere, normal toward an ideal point. Navigation modes impossible in Euclidean geometry.

### 6. Parallel transport orientation

In Euclidean space, moving the camera without rotating leaves its orientation unchanged. In curved space, transported orientation depends on the path. Offer standard vs parallel-transported orientation; moving around a loop can rotate the camera even though you never manually rotated it — a very direct manifestation of curvature. *Falls out naturally from an animation-path integrator that composes isometries incrementally.*

### 7. Holonomy playback

After following a path A → B → C → A, the camera has acquired a rotation. Visualizing accumulated holonomy could be a camera feature. *With #6 implemented, this is just "don't reset the rotation."*

### 8. Geodesic locking

Movement follows geodesics instead of straight Euclidean directions: follow a geodesic, orbit around one, maintain fixed hyperbolic distance from it. *Note: animating tx/ty/tz linearly already moves along a geodesic through the origin (rapidity = distance); this item is about locking motion to arbitrary geodesics.*

### 12. Ideal-point orbit

Orbit around an ideal point instead of a finite one — motion becomes asymptotic rather than circular. No Euclidean equivalent. *This is precisely the parabolic isometry flow.*

### 13. Isometry interpolation

Interpolate through the isometry group SO⁺(3,1) rather than position + quaternion — smoother motion because interpolation follows geodesics in the space of camera poses rather than linear paths in coordinates. *The natural keyframe-interpolation mode for animation tracks.*

### Geodesic flow mode

Advance the camera along the geodesic flow determined by its current position and direction, producing trajectories with the characteristic chaotic mixing associated with hyperbolic geometry.

### Fundamental-domain hopping

When visualizing a quotient space (a hyperbolic manifold or tessellation), jump the camera by deck transformations to equivalent copies of the scene. The local view remains identical while the global context changes — a capability with no analogue in ordinary Euclidean space.

---

## Deferred 💤

### 10. Exponential-distance scaling

Render according to hyperbolic distance: brightness = e^(−d), fog = tanh(d), size = sech(d) — reveals the intrinsic metric. *This is DC-coloring / density-writing by hyperbolic distance: a different feature axis that competes with the source variations' own coloring; wants its own design pass (would also give shell fading, see #9).*

### 15. FOV based on angle defect

Define field of view intrinsically: how many geodesics span the screen, total angle defect represented, fraction of the ideal boundary visible. *A UI/metrics concept more than a transform.*

### Boundary morphing

Warp the rendering by applying a Möbius transformation to the ideal boundary. *Mostly already the tx/ty/rot controls — disk Möbius transformations are the isometries — except for the parabolic class (see Pathing & Navigation prerequisite).*

### Curvature lens

Different parts of the image behave as though they have different negative curvatures — a spatially varying version of the Curvature param. *Needs a real design (what varies K, and where).*

### Distance remapping

Replace the displayed distance function d with a nonlinear mapping such as log(1+d) or d², changing how the exponential expansion of space is perceived. *Natural generalization of the Curvature dial (which is the linear map d/√−K) to arbitrary f(d).*

---

The key distinction is that in Euclidean space, every camera pose is essentially determined by **position + orientation + projection**. In hyperbolic space, the underlying geometry itself provides additional intrinsic structures — ideal points, geodesic flow, horospheres, holonomy, and multiple natural conformal or projective models — that can become meaningful, user-controllable aspects of the camera. These aren't merely visual effects; they reflect genuine geometric features that don't exist in flat space.
