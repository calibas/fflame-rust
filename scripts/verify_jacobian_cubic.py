"""Verification of every formula in src/variations/defs/jacobian_cubic.rs:
the constant Jacobian, the weighted-homogeneous reduction to (a,b), the
depressed fiber cubic, and the closed-form 3-branch inverse (numeric
3-preimage check). Requires sympy, mpmath, numpy.
Run: python scripts/verify_jacobian_cubic.py"""

# Part 1: symbolic identities.
# 1. det DF == -2 (constant)
# 2. weighted homogeneity (x,y,z) -> (l^-1 x, l y, l^2 z), outputs (R,Q,P) weights (-1,1,2)
# 3. reduction: a=xy, b=x^2 z, c=2-3a-b, V=(1+a)^2 b + a^2(3a+4)
#    a_out = R*Q == c*(a+3V),  b_out = R^2*P == c^2*(1+a)*V
# 4. det of reduced 2D map == 2c^2
# 5. skew product: R == x * c(a,b)
import sympy as sp

x, y, z, lam = sp.symbols('x y z lambda')

P = (1 + x*y)**3 * z + y**2 * (1 + x*y) * (4 + 3*x*y)
Q = y + 3*x*(1 + x*y)**2 * z + 3*x*y**2*(4 + 3*x*y)
R = 2*x - 3*x**2*y - x**3*z

F = sp.Matrix([P, Q, R])
J = F.jacobian([x, y, z])
detJ = sp.expand(J.det())
print("1. det DF =", detJ)

# 2. weights
subsw = {x: x/lam, y: lam*y, z: lam**2*z}
for name, expr, w in [("P", P, 2), ("Q", Q, 1), ("R", R, -1)]:
    scaled = sp.expand(expr.subs(subsw))
    ok = sp.simplify(scaled - lam**w * expr) == 0
    print(f"2. {name} weight {w}: {ok}")

# 3. reduction identities
a, b = sp.symbols('a b')
c = 2 - 3*a - b
V = (1 + a)**2 * b + a**2 * (3*a + 4)
a_out_claim = c * (a + 3*V)
b_out_claim = c**2 * (1 + a) * V

a_out_true = sp.expand(R * Q)
b_out_true = sp.expand(R**2 * P)
subs_ab = [(a, x*y), (b, x**2*z)]
print("3. a' = R*Q == c(a+3V):", sp.simplify(a_out_true - a_out_claim.subs(subs_ab)) == 0)
print("3. b' = R^2*P == c^2(1+a)V:", sp.simplify(b_out_true - b_out_claim.subs(subs_ab)) == 0)

# 5. skew product R == x*c
print("5. R == x*c(a,b):", sp.simplify(R - (x * c.subs(subs_ab))) == 0)

# 4. det of reduced map
G = sp.Matrix([a_out_claim, b_out_claim])
JG = G.jacobian([a, b])
detG = sp.factor(sp.expand(JG.det()))
print("4. det D(a',b') =", detG)

# Part 2: fiber cubic + numeric 3-preimage inversion.
a, b, A, B, u = sp.symbols('a b A B u')
c = 2 - 3*a - b
V = (1 + a)**2 * b + a**2 * (3*a + 4)
E1 = sp.expand(c * (a + 3*V) - A)
E2 = sp.expand(c**2 * (1 + a) * V - B)

cubic = (A**3*a**3 + 3*A**3*a**2 + 3*A**3*a + A**3 - A**2*a**3 - 3*A**2*a**2 - 2*A**2*a
         - 18*A*B*a**3 - 54*A*B*a**2 - 54*A*B*a - 18*A*B + 27*B**2*a**3 + 81*B**2*a**2
         + 81*B**2*a + 27*B**2 + 16*B*a**3 + 48*B*a**2 + 36*B*a)
cu = sp.expand(cubic.subs(a, u - 1))
print("cubic in u = 1+a:")
print(sp.collect(cu, u))

# numeric check: generic target
import mpmath as mp
mp.mp.dps = 30
x0, y0, z0 = mp.mpc('0.31','0.17'), mp.mpc('-0.42','0.23'), mp.mpc('0.11','-0.35')
def Fmap(x, y, z):
    P = (1 + x*y)**3 * z + y**2*(1 + x*y)*(4 + 3*x*y)
    Q = y + 3*x*(1 + x*y)**2 * z + 3*x*y**2*(4 + 3*x*y)
    R = 2*x - 3*x**2*y - x**3*z
    return P, Q, R
Pv, Qv, Rv = Fmap(x0, y0, z0)
Av, Bv = Rv*Qv, Rv**2*Pv     # reduced-map image of (a0,b0)
print("\ntarget (A,B) =", mp.nstr(Av, 8), mp.nstr(Bv, 8))

# roots of the cubic in a
coeffs_a = sp.Poly(cubic, a).all_coeffs()
import numpy as np
cnum = [complex(sp.N(cc.subs({A: complex(Av), B: complex(Bv)}))) for cc in coeffs_a]
roots = np.roots(cnum)
print("\nfiber roots a =", roots)

# for each root, recover b from E1 (quadratic in b), select by E2, lift to (x,y,z)
E1p = sp.Poly(E1, b)
E2f = sp.lambdify((a, b, A, B), E2, 'numpy')
E1c = [sp.lambdify((a, A, B), cc, 'numpy') for cc in E1p.all_coeffs()]
found = []
for ra in roots:
    qc = [f(complex(ra), complex(Av), complex(Bv)) for f in E1c]
    for rb in np.roots(qc):
        if abs(E2f(complex(ra), complex(rb), complex(Av), complex(Bv))) < 1e-9:
            # lift: x = R / c(a,b), y = a/x, z = b/x^2
            cval = 2 - 3*complex(ra) - complex(rb)
            xx = complex(Rv) / cval
            yy = complex(ra) / xx
            zz = complex(rb) / xx**2
            Pn, Qn, Rn = Fmap(mp.mpc(xx), mp.mpc(yy), mp.mpc(zz))
            err = abs(Pn - Pv) + abs(Qn - Qv) + abs(Rn - Rv)
            found.append((xx, yy, zz, float(err)))
print("\nfull preimages (x,y,z, |F(p)-target|):")
for f in found:
    print("  x=%.6g%+.6gj err=%.3g" % (f[0].real, f[0].imag, f[3]))
print("\ncount:", len(found))
