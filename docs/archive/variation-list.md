Here's all the variations in Apophysis (38 + random):
(https://github.com/xyrus02/apophysis-7x/blob/b39211fc2f29e177434009733181a1839a73bbfc/src/Forms/ScriptForm.pas#L3542C1-L3580C40)

LINEAR', 0
SINUSOIDAL', 1
SPHERICAL', 2
SWIRL', 3
HORSESHOE', 4
POLAR', 5
HANDKERCHIEF', 6
HEART', 7
DISC', 8
SPIRAL', 9
HYPERBOLIC', 10
DIAMOND', 11
EX', 12
JULIA', 13
BENT', 14
WAVES', 15
FISHEYE', 16
POPCORN', 17
EXPONENTIAL', 18
POWER', 19
COSINE', 20
RINGS', 21
FAN', 22
EYEFISH', 23
BUBBLE', 24
CYLINDER', 25
NOISE', 26
BLUR', 27
GAUSSIANBLUR', 28
RADIALBLUR', 29
RINGS2', 30
FAN2', 31
BLOB', 32
PDJ', 33
PERSPECTIVE', 34
JULIAN', 35
JULIASCOPE', 36
CURL', 37
RANDOM', -1

Here's the variations from Scott Draves (49):

Linear (Variation 0)
Sinusoidal (Variation 1)
Spherical (Variation 2)
Swirl (Variation 3)
Horseshoe (Variation 4)
Polar (Variation 5)
Handkerchief (Variation 6)
Heart (Variation 7)
Disc (Variation 8)
Spiral (Variation 9)
Hyperbolic (Variation 10)
Diamond (Variation 11)
Ex (Variation 12)
Julia (Variation 13)
Bent (Variation 14)
Waves (Variation 15) - dependent
Fisheye (Variation 16)
Popcorn (Variation 17) - dependent
Exponential (Variation 18)
Power (Variation 19)
Cosine (Variation 20)
Rings (Variation 21) - dependent
Fan (Variation 22) - dependent
Blob (Variation 23) - parametric
PDJ (Variation 24) - parametric
Fan2 (Variation 25) - parametric
Rings2 (Variation 26) - parametric
Eyefish (Variation 27)
Bubble (Variation 28)
Cylinder (Variation 29)
Perspective (Variation 30) - parametric
Noise (Variation 31)
JuliaN (Variation 32) - parametric
JuliaScope (Variation 33) - parametric
Blur (Variation 34)
Gaussian (Variation 35)
RadialBlur (Variation 36) - parametric
Pie (Variation 37) - parametric
Ngon (Variation 38) - parametric
Curl (Variation 39) - parametric
Rectangles (Variation 40) - parametric
Arch (Variation 41)
Tangent (Variation 42)
Square (Variation 43)
Rays (Variation 44)
Blade (Variation 45)
Secant (Variation 46)
Twintrian (Variation 47)
Cross (Variation 48)


Linear (Variation 0)
V0(x, y) = (x, y)
16
Sinusoidal (Variation 1)
V1(x, y) = (sin x, sin y)
Spherical (Variation 2)
V2(x, y) = 1
r2 · (x, y)
17
Swirl (Variation 3)
V3(x, y) = (x sin(r2) − y cos(r2), x cos(r2) + y sin(r2))
Horseshoe (Variation 4)
V4(x, y) = 1
r · ((x − y)(x + y), 2xy)
18
Polar (Variation 5)
V5(x, y) =
( θ
π , r − 1
)
Handkerchief (Variation 6)
V6(x, y) = r · (sin(θ + r), cos(θ − r))
19
Heart (Variation 7)
V7(x, y) = r · (sin(θr), − cos(θr))
Disc (Variation 8)
V8(x, y) = θ
π · (sin(πr), cos(πr))
20
Spiral (Variation 9)
V9(x, y) = 1
r (cos θ + sin r, sin θ − cos r)
Hyperbolic (Variation 10)
V10(x, y) =
( sin θ
r , r cos θ
)
21
Diamond (Variation 11)
V11(x, y) = (sin θ cos r, cos θ sin r)
Ex (Variation 12)
p0 = sin(θ + r), p1 = cos(θ − r)
V12(x, y) = r · (p3
0 + p3
1, p3
0 − p3
1)
22
Julia (Variation 13)
V13(x, y) = √r · (cos(θ/2 + Ω), sin(θ/2 + Ω)
(Note: The grid visualization for the julia variation only includes data for
Ω = 0.)
Bent (Variation 14)
V14(x, y) =



(x, y) x ≥ 0, y ≥ 0
(2x, y) x < 0, y ≥ 0
(x, y/2) x ≥ 0, y < 0
(2x, y/2) x < 0, y < 0
23
Waves (Variation 15) - dependent
V15(x, y) =
(
x + b sin
( y
c2
)
, y + e sin
( x
f 2
))
Fisheye (Variation 16)
Note the reversed order of x and y in the formula.
V16(x, y) = 2
r + 1 · (y, x)
24
Popcorn (Variation 17) - dependent
V17(x, y) = (x + c sin(tan 3y), y + f sin(tan 3x))
Exponential (Variation 18)
V18(x, y) = exp(x − 1) · (cos(πy), sin(πy))
25
Power (Variation 19)
V19(x, y) = rsin θ · (cos θ, sin θ)
Cosine (Variation 20)
V20(x, y) = (cos(πx) cosh(y), − sin(πx) sinh(y))
26
Rings (Variation 21) - dependent
V21(x, y) = ((r + c2) mod (2c2) − c2 + r(1 − c2)) · (cos θ, sin θ)
Fan (Variation 22) - dependent
t = πc2
V22(x, y) =
{ r · (cos(θ − t/2), sin(θ − t/2)) (θ + f ) mod t > t/2
r · (cos(θ + t/2), sin(θ + t/2)) (θ + f ) mod t ≤ t/2
27


Blob (Variation 23) - parametric
p1 = blob.high, p2 = blob.low, p3 = blob.waves
V23(x, y) = r ·
(
p2 + p1 − p2
2 (sin(p3θ) + 1)
)
· (cos θ, sin θ)
PDJ (Variation 24) - parametric
p1 = pdj.a, p2 = pdj.b, p3 = pdj.c, p4 = pdj.d
V24(x, y) = (sin(p1y) − cos(p2x), sin(p3x) − cos(p4y))
28
Fan2 (Variation 25) - parametric
Fan2 was created as a parametric alternative to Fan.
p1 = π(fan2.x)2, p2 = fan2.y
t = θ + p2 − p1trunc( 2θp2
p1
)
V25(x, y) =
{ r · (sin (θ − p1/2) , cos (θ − p1/2)) t > p1/2
r · (sin (θ + p1/2) , cos (θ + p1/2)) t ≤ p1/2
29
Rings2 (Variation 26) - parametric
Rings2 was created as a parametric alternative to Rings.
p = (rings2.val)2
t = r − 2ptrunc
( r + p
2p
)
+ r(1 − p)
V26(x, y) = t · (sin θ, cos θ)
Eyefish (Variation 27)
Eyefish was created to correct the order of x and y in Fisheye.
V27(x, y) = 2
r + 1 · (x, y)
30
Bubble (Variation 28)
V28(x, y) = 4
r2 + 4 · (x, y)
Cylinder (Variation 29)
V29(x, y) = (sin x, y)
31


  Perspective (Variation 30) - parametric
p1 = perspective.angle, p2 = perspective.dist
V30(x, y) = p2
p2 − y sin p1
· (x, y cos p1)
Noise (Variation 31)
V31(x, y) = Ψ1 · (x cos(2πΨ2), y sin(2πΨ2))
32
JuliaN (Variation 32) - parametric
p1 = juliaN.power, p2 = juliaN.dist
p3 = trunc(|p1|Ψ)
t = (φ + 2πp3)/p1
V32(x, y) = r p2
p1 · (cos t, sin t)
JuliaScope (Variation 33) - parametric
p1 = juliaScope.power, p2 = juliaScope.dist
p3 = trunc(|p1|Ψ)
t = (Λφ + 2πp3)/p1
V33(x, y) = r p2
p1 · (cos t, sin t)
33
Blur (Variation 34)
V34(x, y) = Ψ1 · (cos(2πΨ2), sin(2πΨ2))
Gaussian (Variation 35)
Summing 4 random numbers and subtracting 2 is an attempt at approximating
a Gaussian distribution.
V35(x, y) =
( 4∑
k=1
Ψk − 2
)
· (cos(2πΨ5), sin(2πΨ5))
34
RadialBlur (Variation 36) - parametric
p1 = (radialBlur.angle) · (π/2)
t1 = v36(
4∑
k=1
Ψk − 2), t2 = φ + t1 sin p1, t3 = t1 cos p1 − 1
V36(x, y) = 1
v36
· (r cos t2 + t3x, r sin t2 + t3y)
Pie (Variation 37) - parametric
p1 = pie.slices, p2 = pie.rotation, p3 = pie.thickness
t1 = trunc(Ψ1p1 + 0.5)
t2 = p2 + 2π
p1
(t1 + Ψ2p3)
V37(x, y) = Ψ3(cos t2, sin t2)
35
Ngon (Variation 38) - parametric
p1 = ngon.power, p2 = 2π/ngon.sides, p3 = ngon.corners, p4 = ngon.circle
t3 = φ − p2⌊φ/p2⌋
t4 =
{ t3 t3 > p2/2
t3 − p2 t3 ≤ p2/2
k =
p3
( 1
cos t4 − 1
)
+ p4
rp1
V38(x, y) = k · (x, y)
36
Curl (Variation 39) - parametric
p1 = curl.c1, p2 = curl.c2
t1 = 1 + p1x + p2(x2 − y2), t2 = p1y + 2p2xy
V39(x, y) = 1
t2
1 + t2
2
· (xt1 + yt2, yt1 − xt2)
Rectangles (Variation 40) - parametric
p1 = rectangles.x, p2 = rectangles.y
V40(x, y) = (2⌊x/p1⌋ + 1)p1 − x, (2⌊y/p2⌋ + 1)p2 − y)
37
Arch (Variation 41)
V41(x, y) = (sin(Ψπv41), sin2(Ψπv41)/ cos(Ψπv41))
Tangent (Variation 42)
V42(x, y) =
( sin x
cos y , tan y
)
38
Square (Variation 43)
V43(x, y) = (Ψ1 − 0.5, Ψ2 − 0.5)
Rays (Variation 44)
V44(x, y) = v44 tan(Ψπv44)
r2 · (cos x, sin y)
39
Blade (Variation 45)
V45(x, y) = x · (cos(Ψrv45) + sin(Ψrv45), cos(Ψrv45) − sin(Ψrv45))
Secant (Variation 46)
V46(x, y) =
(
x, 1
v46 cos(v46r)
)
40
Twintrian (Variation 47)
t = log10
(sin2(Ψrv47)) + cos(Ψrv47)
V47(x, y) = x · (t, t − π sin(Ψrv47))
Cross (Variation 48)
V47(x, y) = √1/(x2 − y2)2 · (x, y)