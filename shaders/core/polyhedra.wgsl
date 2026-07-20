// Polyhedron radial-support helpers — shared by the `polyhedron`
// (surface projection) and `polyhedron_volume` (solid occluder)
// variations. Included once by the shader builder when either is
// active, so the two can coexist in one flame without duplicate
// symbols.
//
// Every solid is normalized to CIRCUMRADIUS 1 and centered at the
// origin. The radial support function r(dir) — the distance from the
// center to the surface along unit direction `dir` — is
// min over faces of d_i / (n_i · dir), which for these symmetric
// solids reduces to abs()/max() closed forms (verified to 4e-16
// against brute-force face enumeration by scratchpad polyhedra_gen).
// The star tetrahedron is the union of two point-reflected tetrahedra
// (star-shaped about the center), so its radial is the max of theirs.
//
// Shape ids (must match the defs' enum order):
//   0 Tetrahedron        1 Cube               2 Octahedron
//   3 Dodecahedron       4 Icosahedron        5 Star Tetrahedron
//   6 Cuboctahedron      7 Rhombic Dodecahedron
//   8 Truncated Octahedron

fn polyhedra_radial_tetra(dir: vec3<f32>) -> f32 {
    // Vertices {(1,1,1),(1,-1,-1),(-1,1,-1),(-1,-1,1)}/sqrt3; face
    // normals are the negated vertex directions, inradius 1/3.
    let s = 0.57735026919;
    let m = max(
        max(-(dir.x + dir.y + dir.z), -(dir.x - dir.y - dir.z)),
        max(-(-dir.x + dir.y - dir.z), -(-dir.x - dir.y + dir.z)),
    ) * s;
    return 0.33333333333 / max(m, 1e-6);
}

fn polyhedra_radial(dir: vec3<f32>, shape: u32) -> f32 {
    let a = abs(dir);
    let phi = 1.61803398875;
    var r = 1.0;
    switch shape {
        case 0u: {
            r = polyhedra_radial_tetra(dir);
        }
        case 1u: { // cube: 6 axis faces, d = 1/sqrt3
            r = 0.57735026919 / max(max(a.x, a.y), max(a.z, 1e-6));
        }
        case 2u: { // octahedron: 8 diagonal faces, d*sqrt3 = 1
            r = 1.0 / max(a.x + a.y + a.z, 1e-6);
        }
        case 3u: { // dodecahedron: 12 faces along icosa vertex dirs (0,±1,±phi)
            let m = max(a.y + phi * a.z, max(a.z + phi * a.x, a.x + phi * a.y)) * 0.52573111212;
            r = 0.79465447230 / max(m, 1e-6);
        }
        case 4u: { // icosahedron: 8 diagonal + 12 cyclic (0,±phi,±1/phi) faces
            let m1 = a.x + a.y + a.z;
            let m2 = max(phi * a.y + a.z / phi, max(phi * a.z + a.x / phi, phi * a.x + a.y / phi));
            r = 0.79465447230 / max(max(m1, m2) * 0.57735026919, 1e-6);
        }
        case 5u: { // star tetrahedron (stella octangula): union of two tetras
            r = max(polyhedra_radial_tetra(dir), polyhedra_radial_tetra(-dir));
        }
        case 6u: { // cuboctahedron: 6 square (axis) + 8 triangle (diagonal) faces
            let m1 = max(max(a.x, a.y), a.z);
            let m2 = (a.x + a.y + a.z) * 0.57735026919;
            r = min(0.70710678119 / max(m1, 1e-6), 0.81649658093 / max(m2, 1e-6));
        }
        case 7u: { // rhombic dodecahedron: 12 faces along (±1,±1,0) perms
            let m = max(a.x + a.y, max(a.y + a.z, a.z + a.x)) * 0.70710678119;
            r = 0.70710678119 / max(m, 1e-6);
        }
        case 8u: { // truncated octahedron: 6 square (axis) + 8 hex (diagonal) faces
            let m1 = max(max(a.x, a.y), a.z);
            let m2 = (a.x + a.y + a.z) * 0.57735026919;
            r = min(0.89442719100 / max(m1, 1e-6), 0.77459666924 / max(m2, 1e-6));
        }
        default: { // fallback: unit sphere
            r = 1.0;
        }
    }
    return r;
}

// Map a world-space direction into the solid's local frame. The solid
// is rotated by intrinsic Euler angles Rz(rz)·Ry(ry)·Rx(rx) (degrees),
// so sampling its radial function along world `dir` evaluates along
// R⁻¹·dir = Rx(−rx)·Ry(−ry)·Rz(−rz)·dir.
fn polyhedra_inverse_rotate(dir: vec3<f32>, rx: f32, ry: f32, rz: f32) -> vec3<f32> {
    let d2r = 0.01745329252;
    var d = dir;
    let cz = cos(-rz * d2r);
    let sz = sin(-rz * d2r);
    d = vec3<f32>(cz * d.x - sz * d.y, sz * d.x + cz * d.y, d.z);
    let cy = cos(-ry * d2r);
    let sy = sin(-ry * d2r);
    d = vec3<f32>(cy * d.x + sy * d.z, d.y, -sy * d.x + cy * d.z);
    let cx = cos(-rx * d2r);
    let sx = sin(-rx * d2r);
    d = vec3<f32>(d.x, cx * d.y - sx * d.z, sx * d.y + cx * d.z);
    return d;
}
