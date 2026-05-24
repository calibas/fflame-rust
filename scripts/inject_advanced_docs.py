"""Apply doc-comments + per-param descriptions to advanced.rs.

One-off script for the variations-bulk-metadata project. Idempotent
(skips silently if the doc-comment is already present)."""
import re
import textwrap

DOC = {
    "POLAR": "Switches to polar coordinates: the X output becomes the angle (scaled to [-1, 1]), the Y output becomes the radius minus 1. Unwraps circular patterns into horizontal stripes.",
    "HANDKERCHIEF": "Twists radial waves so the pattern looks like a knotted handkerchief - concentric folds that ripple in toward the center.",
    "HEART": "Folds the plane along a heart-shaped curve. The output silhouette traces a cardioid; classic Apophysis effect.",
    "DISC": "Wraps the plane onto a disc, with the angle controlling the radial position and the radius controlling the ripples. Creates a hypnotic sunburst pattern.",
    "SPIRAL": "Combines an inverse-radius scaling with sine/cosine of both angle and radius. The result spirals inward in a logarithmic pattern.",
    "HYPERBOLIC": "Inverts X by the squared radius while leaving Y alone. Stretches things horizontally near the origin and squashes them outside.",
    "DIAMOND": "Maps the plane into a rotated diamond shape using sine and cosine of the polar angle and radius. Produces sharp diagonal symmetry.",
    "EX": "Cubes two sinusoidal functions of angle and radius and blends them. Creates a complex three-armed star.",
    "JULIA": "Randomly picks one of the two branches of the complex square root, then applies it. Each iteration jumps to one half of the Julia-set folding; over time the attractor fills out.",
    "BENT": "Doubles the X coordinate when X is negative, halves the Y coordinate when Y is negative. A simple asymmetric pinch.",
    "WAVES": "Adds sine-wave displacement to each coordinate, using the affine matrix's own b/c/d/f fields as wave parameters. Inherits its frequency and amplitude from the transform itself rather than from extra sliders.",
    "JULIAN": "Generalized Julia variation with a chosen integer power. Splits the angle into `power` equally-spaced branches and randomly picks one each iteration. With power = 2 it reduces to the classic Julia.",
    "BLOB": "Wraps the plane around the origin, with the radius pulsing between a high and low value as the angle rotates. Produces a wavy, bumpy boundary.",
    "EYEFISH": "An anti-fisheye that pulls everything toward the unit circle. Inverse of the classic fisheye warp.",
    "BUBBLE": "Maps the plane onto a sphere - far points shrink toward the equator, near points spread across the surface.",
    "CYLINDER": "Wraps the X coordinate around a cylinder (sine), passes Y through unchanged. In 3D, adds a cosine of X as the Z coordinate so the plane really wraps into a cylindrical sheet.",
    "NOISE": "Multiplies the point by a random radius in a random direction. Adds a textured, noisy spray to the rendered shape.",
    "BLUR": "Replaces the input with a uniformly random point inside the unit disc - the position is ignored. Useful for adding a soft glow or particle haze.",
    "GAUSSIAN_BLUR": "Like Blur but the random radius follows a bell curve (sum of four uniforms). Produces a softer, more concentrated haze.",
    "POLAR2": "Variant of Polar with log-radius output. Compresses large distances and expands small ones; good for revealing distant structure.",
    "CROSS": "Divides each coordinate by the absolute difference of squared coordinates. Produces a sharp diagonal cross pattern.",
    "LOONIE": "Inside the unit circle, inflates points outward; outside, leaves them alone. Creates a coin shape with a sharp edge at radius 1.",
    "SCRY": "Pulls every point toward the origin with a strength that drops off with distance. Produces a magnifying-glass / scrying-orb effect.",
    "FOCI": "Maps the plane through a hyperbolic curve based on exponentials. Produces two focal points that warp the surrounding space.",
    "ELLIPTIC": "Conformal map onto an elliptic-coordinate grid. Useful for mathematical-looking, symmetric patterns.",
    "WAVES2": "Like Waves, but the sine wave frequencies and amplitudes are exposed as sliders instead of being baked into the affine. Independent control over each axis.",
    "LOG": "Polar log transform - the output X is the logarithm of the squared distance, the output Y is the angle. Produces a spiral log-scale view.",
    "ESCHER": "Conformal log-spiral mapping inspired by M. C. Escher's prints. Tunes between pure scaling and pure rotation via the beta angle.",
    "BIPOLAR": "Maps to bipolar coordinates (a pair of orthogonal coordinate systems centered on two points). Good for two-focus symmetric flames.",
    "LAZYSUSAN": "Inside a unit disc the points rotate and twist; outside the disc they're pushed away from the center. Produces a layered, plate-like swirl.",
    "RINGS2": "Carves the plane into concentric ring bands at the chosen spacing. Each ring inverts the radial position within its band.",
    "FAN2": "Slices the plane into pie wedges and offsets each wedge alternately - even wedges go one way, odd wedges the other. Configurable wedge width and rotation.",
    "PDJ": "Peter de Jong attractor - four sine/cosine coefficients drive the output. Famous for producing intricate chaotic attractor shapes.",
    "CURL": "Multiplies the input by a complex polynomial (1 + c1*z + c2*z^2) and normalises. Adds a soft swirling distortion.",
    "RECTANGLES": "Tiles the plane into rectangles, mirroring the coordinates within each tile. Produces a checkered, blocky output.",
    "SPLITS": "Pushes positive-X points and negative-X points apart by `x`, and same for Y. Creates a gap down the middle along each axis.",
    "NGON": "Bends the plane into an N-sided polygon outline. Configurable side count, corner sharpness, and how circle-vs-polygon the shape feels.",
    "AUGER": "Drills a corkscrew distortion into the plane - sine waves on both axes coupled together. Produces twisting, augur-like patterns.",
    "CPOW": "Raises the complex point to a complex power (real + imaginary parts of the exponent both adjustable). Produces logarithmic spirals with `power` arms.",
}

PARAM_DOC = {
    ("JULIAN", "power"): "Number of branches the output is split into. Higher = more arms; negative values flip the rotation direction.",
    ("JULIAN", "dist"): "Stretches or compresses each arm radially. 1.0 is balanced; larger pushes arms outward, smaller pulls them in.",
    ("BLOB", "high"): "Outer radius - how far the bumps reach at their peaks.",
    ("BLOB", "low"): "Inner radius - how close the bumps recede in the troughs.",
    ("BLOB", "waves"): "How many bumps go around the perimeter. More waves = finer-grained edge.",
    ("WAVES2", "freqx"): "Horizontal ripple frequency. More = tighter waves across the X axis.",
    ("WAVES2", "scalex"): "Horizontal ripple amplitude. How far points get pushed sideways.",
    ("WAVES2", "freqy"): "Vertical ripple frequency.",
    ("WAVES2", "scaley"): "Vertical ripple amplitude.",
    ("WAVES2", "freqz"): "Depth ripple frequency (3D mode only).",
    ("WAVES2", "scalez"): "Depth ripple amplitude (3D mode only).",
    ("LOG", "base"): "Logarithm base. Default `e` (natural log); larger compresses the output, smaller stretches it out.",
    ("ESCHER", "beta"): "Balance between scaling and rotation. At 0 degrees the map is pure scaling; near +/-90 degrees it's pure rotation. Sweep this to get spiraling effects.",
    ("BIPOLAR", "shift"): "Vertical offset on the output. Slides the bipolar pattern up or down.",
    ("LAZYSUSAN", "spin"): "How far points inside the unit disc rotate.",
    ("LAZYSUSAN", "space"): "Gap added to points outside the disc - pushes the outer region away from center.",
    ("LAZYSUSAN", "twist"): "Extra rotation that fades with distance. Adds a twisting motion to the inside.",
    ("LAZYSUSAN", "x"): "Horizontal offset of the rotation center.",
    ("LAZYSUSAN", "y"): "Vertical offset of the rotation center.",
    ("RINGS2", "val"): "Ring spacing. Smaller packs more rings closer together; larger spreads them out.",
    ("FAN2", "x"): "Wedge width. Controls how many sectors the fan is split into.",
    ("FAN2", "y"): "Rotation offset. Spins the whole fan around the origin.",
    ("PDJ", "a"): "Coefficient on the first sine - shapes the X output curve.",
    ("PDJ", "b"): "Coefficient on the first cosine - shapes the X output curve.",
    ("PDJ", "c"): "Coefficient on the second sine - shapes the Y output curve.",
    ("PDJ", "d"): "Coefficient on the second cosine - shapes the Y output curve.",
    ("CURL", "c1"): "Linear twist strength. Stronger = tighter curl around the center.",
    ("CURL", "c2"): "Quadratic twist strength. Adds a second-order curl that grows away from the origin.",
    ("RECTANGLES", "x"): "Width of each rectangular tile.",
    ("RECTANGLES", "y"): "Height of each rectangular tile.",
    ("SPLITS", "x"): "Horizontal gap. Pushes positive-X and negative-X points apart by this amount.",
    ("SPLITS", "y"): "Vertical gap. Pushes positive-Y and negative-Y points apart by this amount.",
    ("NGON", "sides"): "Number of sides of the polygon (e.g. 5 = pentagon, 6 = hexagon).",
    ("NGON", "power"): "Radial exponent. Stretches or compresses the polygon shape outward.",
    ("NGON", "circle"): "Blend between polygon and circle. 0 = pure circle, higher = sharper corners.",
    ("NGON", "corners"): "Horizontal output offset. Useful for tiling the polygon outward.",
    ("AUGER", "freq"): "Ripple frequency. How many waves go across the surface.",
    ("AUGER", "weight"): "How strongly the waves displace points. 0 = no effect.",
    ("AUGER", "scale"): "Cross-coupling between X and Y waves. Tunes the diagonal texture.",
    ("AUGER", "sym"): "Blend back toward the input. 0 = full displacement, 1 = no displacement.",
    ("CPOW", "r"): "Real component of the complex exponent. Controls scaling and how tightly the spiral winds.",
    ("CPOW", "i"): "Imaginary component of the complex exponent. Controls how much the spiral rotates.",
    ("CPOW", "power"): "Number of branches in the result. Like JuliaN's `power` - more = more arms.",
}

PATH = "src/variations/defs/advanced.rs"
with open(PATH, "rb") as f:
    src = f.read().decode("utf-8")

# PASS 1: insert doc comments before each `pub static <NAME>:` line.
# Idempotent: skip if a `///` doc-comment line already directly precedes the
# pub static (don't double-insert).
inserted = 0
already = 0
skipped = 0
for name, body in DOC.items():
    target = f"\npub static {name}: VariationDef = VariationDef {{"
    if target not in src:
        print(f"  WARN: no match for {name}")
        skipped += 1
        continue
    idx = src.find(target)
    # Look backward from idx for the most recent non-blank line.
    prefix = src[:idx]
    last_nl = prefix.rfind("\n")
    if last_nl != -1:
        prev_line = src[last_nl + 1:idx + 1].strip()
        if prev_line.startswith("///"):
            already += 1
            continue
    lines = []
    for paragraph in body.split("\n"):
        wrapped = textwrap.fill(paragraph, width=72) if paragraph.strip() else ""
        for line in wrapped.split("\n"):
            lines.append(f"/// {line}".rstrip())
    doc = "\n".join(lines) + "\n///\n/// # Authors\n/// - Scott Draves"
    src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
    inserted += 1
print(f"  pass1: inserted {inserted}, already {already}, skipped {skipped}")

# PASS 2: inject per-param descriptions
param_inserted = 0
param_already = 0
for (static_name, param_name), desc in PARAM_DOC.items():
    start_pattern = f"pub static {static_name}: VariationDef"
    start_idx = src.find(start_pattern)
    if start_idx == -1:
        print(f"  WARN: no static {static_name}")
        continue
    next_static = src.find("\npub static ", start_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)
    block = src[start_idx:end_idx]
    pdef_pat = re.compile(
        r'(VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?)description:\s*None,',
        re.DOTALL,
    )
    new_block, n = pdef_pat.subn(
        lambda m: m.group(1) + f'description: Some("{desc}"),',
        block,
        count=1,
    )
    if n == 0:
        # Already done?
        already_pat = re.compile(
            r'VariationParamDef\s*\{[^}]*?name:\s*"' + re.escape(param_name) + r'"[^}]*?description:\s*Some\(',
            re.DOTALL,
        )
        if already_pat.search(block):
            param_already += 1
        else:
            print(f"  WARN: no param {static_name}.{param_name}")
        continue
    src = src[:start_idx] + new_block + src[end_idx:]
    param_inserted += 1
print(f"  pass2: injected {param_inserted}, already {param_already}")

with open(PATH, "wb") as f:
    f.write(src.encode("utf-8"))
