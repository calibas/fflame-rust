> **Planning note (2026-09-01).** This is the seed document for the third
> fractal family. The detailed plan it led to lives in four companion
> documents, which supersede it where they differ:
> [simulation-fractals.md](simulation-fractals.md) (master plan and
> decision record), [simulation-pipeline.md](simulation-pipeline.md)
> (GPU design), [simulation-integration.md](simulation-integration.md)
> (file-by-file checklist) and [simulation-catalog.md](simulation-catalog.md)
> (every model with sources). Two things changed in planning: the mode is
> named **Simulation**, not Field, because `src/escape/fields.rs` already
> uses "field" for escape's neighbour-free mode B (the opposite meaning);
> and the Reusser quotation below is not verbatim -- his kernels are
> analytic in the frequency domain inside an FFT pipeline. No code exists
> yet; the NumPy prototypes are in `output/sim_proto/`.

I've been working on a plan for a whole other category of fractal rendering beyond flame IFS and escape-time/orbit-trapping. 

I want to implement things like DLA, simulated crystal growth and dendrite formation, plus Jonathan McCabe's Belousov–Zhabotinsky reaction type dynamics and Turing patterns.

Here's what I've got so far for the plan.

## 4. Multi-scale Turing patterns and BZ — NOT escape-time

Both requests are whole-image feedback simulations. They cannot be
escape formulas, and the reason is architectural rather than a matter
of effort.

Our escape engine dispatches one thread per pixel that iterates a map
using only its own c and z. It never reads a neighbour. McCabe's
multi-scale Turing algorithm, per
[Reusser's WebGPU implementation](https://rreusser.github.io/notebooks/multiscale-turing-patterns/),
does the opposite at every step:

    Convolve the field with Gaussian or circular kernels at different
    radii to compute activator and inhibitor values … **requires
    whole-image convolutions each step. Each pixel depends on distant
    neighbours through FFT-based convolution — the receptive field spans
    the entire domain.**

The step is: convolve at k scales, pick per pixel the scale with the
smallest |activator − inhibitor|, move the field in that scale's
direction, blend colour toward that scale's colour
([McCabe's paper](https://handandmachine.org/classes/computational_fabrication/wp-content/uploads/2024/10/Cyclic_Symmetric_Multi-Scale_Turing_Patterns.pdf)).

Belousov–Zhabotinsky is the same shape: a reaction-diffusion field
(or the [hodgepodge-machine cellular
automaton](https://www.hermetic.ch/pca/bz.htm)) updated from a
neighbourhood each step, plus a per-step resample of the domain for
the "expanding space" variant.

**What they would actually be:** a THIRD render mode beside flames and
escape — init noise → loop[multi-scale blur → select → update] →
tonemap — whose nearest relative in this codebase is the effects
chain, which already ping-pongs full-resolution textures. Not
src/escape/.

Rough shape, if it is ever taken on:

    a field texture pair (ping-pong), f16 is what the reference implementation uses;
    k blur scales per step — separable Gaussians or a mip pyramid, NOT an FFT, since we have no FFT on the GPU and the reference notes analytic kernels evaluated in-shader;
    a select-and-update pass;
    a driver that runs hundreds of steps and hands the field to the existing tonemap tail.

**Measure before building.** The one number that decides feasibility
is the cost of k full-resolution blurs per step at 1080p, times the
step count needed for the pattern to settle. That is a half-day
prototype and it is the same discipline the NTT investigation used —
which is the reason that project was parked with evidence rather than
built on a guess.

Can you review and suggest any other fractal types to add, or modifications to the plan? No need to dig too deep just yet, I want to see what kind of fractals are available, and what to call this family of fractals.


Solid section. Three things: the name, the gaps in the catalogue, and a few architectural changes I'd make before the prototype.

**What to call it**

"Field" mode, alongside Flame and Escape. Everything here is a scalar/vector field updated by *local* rules — stencil, convolution, or agent deposit — and the local-rule/global-emergence contrast is exactly what distinguishes it from the other two. Academic umbrellas ("pattern formation," "morphogenesis," "dissipative structures") are accurate but nobody will type them into a search box. Internally there are two compute shapes hiding under it, and the plan currently conflates them:

- **Relaxation / dense**: every cell updates every step (reaction-diffusion, CA, phase-field). GPU-native, ping-pong.
- **Growth / front-driven**: only the boundary changes (DLA, Eden, invasion percolation). Serial by nature; needs reformulation to be worth a GPU.

**Plan modifications**

1. Dendrites and crystal growth belong in the dense bucket, not with DLA. Kobayashi's phase-field model (1993) is a two-field PDE — phase + temperature with an anisotropy term — and produces the classic six-fold snowflake dendrites from a 3×3 stencil. Gravner–Griffeath's snowflake CA is the other strong option. Either is trivially GPU-parallel; particle DLA is not.

2. For DLA itself, two GPU-friendly routes: (a) the dielectric breakdown model — Jacobi-relax ∇²φ = 0 on a ping-pong texture, grow with probability ∝ |∇φ|^η. One knob η unifies Eden (0), DLA (1), and dendritic/tip-splitting (>1). (b) Massively parallel walkers — one thread per walker, stick on occupied neighbour, race conditions are benign. Note that (b) is structurally *the chaos game*: a point buffer stepping stochastically and depositing into an accumulation buffer. It might be a flame "variation" plus a sticky test rather than a new engine.

3. Replace "k separable Gaussians per step" with a mip pyramid sampled at fractional LOD. One downsample chain per step, then each scale is a single trilinear texture fetch per pixel — O(1) regardless of radius, which matters because McCabe's radii run to 100+ px. Summed-area tables give exact box kernels at similar cost. This largely dissolves the feasibility question before you measure. Also, "no FFT on the GPU" is a current fact, not a constraint: a Stockham radix-2 FFT in WGSL is ~log₂N dispatches and Reusser did it in WebGL. Park it, don't rule it out.

4. Promote the "expanding space" resample to a first-class **domain warp** stage — per-step affine (zoom, rotate, translate) or flow-field advection. It gets you the zooming BZ, McCabe's cyclic-symmetric variant (rotate-and-average), and a large family of demoscene looks for free, and it reuses your affine machinery.

5. The real architecture is then a tiny op graph, not a "Turing mode": `stencil` (3×3/5×5 local update), `pyramid` (wide blur), `warp`, `deposit` (agents → field), `tonemap`. Nearly everything below composes from those four.

6. Format: `rgba16float` is renderable and filterable everywhere in WebGPU; pack multiple fields into channels (u/v for Gray–Scott, phase/temperature for Kobayashi). Storage writes to r16float aren't universal, so a fragment-pass ping-pong like the effects chain is the safe choice. f16 is fine for RD; phase-field gradients may want f32.

7. Reproducibility: seed the init noise and save it with params, but accept that chaotic dynamics won't be bit-identical across GPUs. Same problem your fixed-point deep-zoom work exists to solve, and here it isn't worth solving — save the final field if a shared render must match.

8. Prototype Gray–Scott first, not McCabe. It's a 3×3 Laplacian with two parameters, Pearson's parameter map is a ready-made preset library, and it validates driver + ping-pong + tonemap in a day. Then add the pyramid.

One flag: the bolded text you attribute to Reusser doesn't read like his notebook prose to me. Worth verifying it's a quote and not your gloss before it lands in a doc.

**Gaps in the catalogue** (all grid-native unless noted)

- Reaction-diffusion: Gray–Scott, FitzHugh–Nagumo, Oregonator (the actual BZ chemistry), Brusselator, Schnakenberg.
- Fourth-order PDEs: Swift–Hohenberg (stripes/hex), Cahn–Hilliard (spinodal decomposition). Two Laplacian passes each.
- Continuous CA: Lenia and SmoothLife — kernel convolution + growth function, same compute shape as McCabe (his concentric rings are essentially Lenia kernels). Very high payoff per line of code.
- Discrete CA: cyclic CA (Griffeath — BZ-like spirals from an integer rule), spatial rock-paper-scissors, Abelian sandpile (the identity element is a genuine fractal; toppling is parallel), 1D Wolfram rules rendered as spacetime (Rule 90 *is* Sierpiński, a nice bridge to your IFS family).
- Growth: Eden, ballistic deposition, Hele-Shaw / Saffman–Taylor fingering, Lichtenberg figures (DBM with high η), invasion percolation, critical-percolation clusters (D = 91/48), Ising at T_c via checkerboard Metropolis.
- Agent-based: Physarum (Jones 2010) — sense/turn/move/deposit into a trail field, then diffuse+decay. Hybrid of your point-buffer and field ops, and the single most popular "field" look in the demoscene.

If you want a triage rule: build the four ops, then Gray–Scott, Lenia, McCabe, Physarum, Kobayashi, DBM — in that order each one adds exactly one new primitive.