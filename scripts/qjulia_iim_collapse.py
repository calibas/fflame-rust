"""Does the inverse-iteration (IIM) walk for c=-1+0.2i stay full-4D, or does it
collapse onto the (real,i) complex plane? We iterate q -> random-branch sqrt(q-c)
from generic 4D seeds and watch the transverse magnitude sqrt(j^2+k^2).

If mean transverse -> 0, IIM degenerates to the 2D Julia embedded in 4D and CANNOT
reproduce Bourke's revolved 4D slices. Convention q=(x,y,z,w)=(i,j,k,real),
c=(0.2,0,0,-1)."""
import numpy as np

rng = np.random.default_rng(0)
C = np.array([0.2, 0.0, 0.0, -1.0])
NP = 20000
q = (rng.random((NP, 4)) * 2 - 1)  # generic 4D seeds

def qsqrt_random_branch(Q):
    # square roots of quaternion Q=(x,y,z,w): mag*(cos(th/2+{0,pi}) + nhat sin(...))
    mag = np.linalg.norm(Q, axis=1) + 1e-12
    vlen = np.linalg.norm(Q[:, :3], axis=1)
    nhat = np.where(vlen[:, None] > 1e-9, Q[:, :3] / np.maximum(vlen, 1e-9)[:, None],
                    np.array([1.0, 0, 0]))
    ang = np.arccos(np.clip(Q[:, 3] / mag, -1, 1))
    branch = rng.integers(0, 2, NP)               # 0 or 1 -> add 0 or 2pi before /2
    half = (ang + branch * 2 * np.pi) / 2.0
    rad = np.sqrt(mag)
    out = np.empty_like(Q)
    out[:, :3] = (rad * np.sin(half))[:, None] * nhat
    out[:, 3] = rad * np.cos(half)
    return out

print("iter   mean|transverse(j,k)|   mean|vec(i,j,k)|")
for it in range(41):
    trans = np.linalg.norm(q[:, 1:3], axis=1).mean()
    vec = np.linalg.norm(q[:, :3], axis=1).mean()
    if it % 5 == 0:
        print(f"{it:4d}      {trans:.5f}                {vec:.5f}")
    q = qsqrt_random_branch(q - C)
