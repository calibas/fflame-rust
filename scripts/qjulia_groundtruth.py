"""Ground-truth quaternion Julia (Bourke's escape-time membership) for
c = -1 + 0.2i, in OUR convention q=(x,y,z,w) with w the scalar/real part,
c=(cx,cy,cz,cw)=(0.2, 0, 0, -1).

Renders three CPU images to output/ so we can see the true outline:
  1. complex plane  (i vs real, j=k=0)      -> the 2D Julia set for -1+0.2i
  2. real-slice z=0 (i vs j, k=0, real=-0.5) -> should be radially symmetric
  3. real-slice z=0 (i vs j, k=0, real= 0.0)
No deps beyond Pillow + numpy.
"""
import numpy as np
from PIL import Image

CX, CY, CZ, CW = 0.2, 0.0, 0.0, -1.0   # c = -1 + 0.2 i, our (i,j,k,real)
N = 600
RANGE = 1.6
ITERS = 40
BAILOUT2 = 16.0  # |q|^2 > 16  ->  escaped

def qmul(a, b):
    # a,b: (...,4) arrays, component order (x,y,z,w)=(i,j,k,real)
    ax, ay, az, aw = a[...,0], a[...,1], a[...,2], a[...,3]
    bx, by, bz, bw = b[...,0], b[...,1], b[...,2], b[...,3]
    return np.stack([
        aw*bx + ax*bw + ay*bz - az*by,
        aw*by - ax*bz + ay*bw + az*bx,
        aw*bz + ax*by - ay*bx + az*bw,
        aw*bw - ax*bx - ay*by - az*bz,
    ], axis=-1)

def render(axis_x, axis_y, fixed):
    """axis_x/axis_y in {'x','y','z','w'}; fixed = dict of the other two."""
    idx = {'x':0,'y':1,'z':2,'w':3}
    lin = np.linspace(-RANGE, RANGE, N)
    gx, gy = np.meshgrid(lin, lin)
    q = np.zeros((N, N, 4), dtype=np.float64)
    q[...,idx[axis_x]] = gx
    q[...,idx[axis_y]] = gy
    for k, v in fixed.items():
        q[...,idx[k]] = v
    c = np.array([CX, CY, CZ, CW])
    alive = np.ones((N, N), dtype=bool)
    escape_iter = np.full((N, N), ITERS, dtype=np.int32)
    for i in range(ITERS):
        q = qmul(q, q) + c
        mag2 = (q*q).sum(axis=-1)
        newly = alive & (mag2 > BAILOUT2)
        escape_iter[newly] = i
        alive[newly] = False
        q[~alive] = 0.0  # freeze escaped to avoid overflow
    # interior = never escaped -> white; exterior banded by escape speed
    img = np.where(alive, 255, (escape_iter.astype(np.float64)/ITERS*120).astype(np.uint8))
    return Image.fromarray(img.astype(np.uint8), 'L')

render('x','w', {'y':0.0,'z':0.0}).save('output/gt_complex_i_real.png')
render('x','y', {'z':0.0,'w':-0.5}).save('output/gt_slice_real_-0.5.png')
render('x','y', {'z':0.0,'w': 0.0}).save('output/gt_slice_real_0.0.png')
print("wrote 3 ground-truth images to output/")
