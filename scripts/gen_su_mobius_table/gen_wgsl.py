"""Generates shaders/core/su_mobius.wgsl — the shared SL(2,C) Mobius-group
library: the baked SU(n)/SO(5) generator tables, su_group_range (the
su_mobius baked-group index map), and the SuMat/conjugator/Poincare-H3
helpers used by every Feature::NeedsMobiusLib variation.

Sources: SU(2) 6-group and SU(3) reduced are Bagula's matrices (Programing4
notebooks); SU(5)/SO(5) are our reductions of the generalized Gell-Mann set
via the tensor t = [[1,1,1,-1,-1],[0,1,-1,i,-i]] with trace-plugging; SU(4)
is our baked reduction (su4_baked.json). Run from the repo root:

    python scripts/gen_su_mobius_table/gen_wgsl.py

Output must be committed together with any table change; group offsets are
append-only (existing offsets must never move).
"""
import os
_HERE = os.path.dirname(os.path.abspath(__file__))
_ROOT = os.path.dirname(os.path.dirname(_HERE))
import cmath, math, io, json
I=1j; s3=math.sqrt(3)
def inv(M):
    a,b,c,d=M[0][0],M[0][1],M[1][0],M[1][1]; det=a*d-b*c; return [[d/det,-b/det],[-c/det,a/det]]
def norm(M):
    a,b,c,d=M[0][0],M[0][1],M[1][0],M[1][1]; sd=cmath.sqrt(a*d-b*c); return [[a/sd,b/sd],[c/sd,d/sd]]
def det(m): return m[0][0]*m[1][1]-m[0][1]*m[1][0]
su2=[[[2,1],[-1,0]],[[I,0],[2,-I]],[[2,I],[I,0]],[[0,1],[-1,2]],[[I,0],[2,-I]],[[0,I],[I,2]]]
su2=su2+[inv(m) for m in su2]
u0=[[[0,I],[I,2*I]],[[2,1],[-1,0]],[[1,1],[1,2]],[[2,2+I],[2+I,2+2*I]],[[0,1],[-1,0]],[[0,I],[I,-2+2*I]],[[0,-1],[1,2]],[[-1/s3,(-1-2*I)/s3],[(-1-2*I)/s3,(-4*I)/s3]]]
su3=u0+[inv(m) for m in u0]
def zeros(): return [[0j]*5 for _ in range(5)]
def sym5(i,j):
    m=zeros(); m[i-1][j-1]=1; m[j-1][i-1]=1; return m
def asym5(i,j,sg):
    m=zeros(); m[i-1][j-1]=sg*I; m[j-1][i-1]=-sg*I; return m
def dg5(v,sc=1.0):
    m=zeros()
    for k in range(5): m[k][k]=v[k]/sc
    return m
S5={1:sym5(1,2),2:sym5(1,3),3:sym5(1,4),4:sym5(1,5),5:sym5(2,3),6:sym5(2,4),7:sym5(3,4),8:sym5(2,5),9:sym5(3,5),10:sym5(4,5),
   11:asym5(1,2,1),12:asym5(1,3,-1),13:asym5(1,4,1),14:asym5(1,5,-1),15:asym5(2,3,1),16:asym5(2,4,-1),17:asym5(3,4,1),18:asym5(2,5,1),19:asym5(3,5,-1),20:asym5(4,5,1),
   21:dg5([1,-1,0,0,0]),22:dg5([1,1,-2,0,-1],math.sqrt(3.5)),23:dg5([1,1,1,-3,0],math.sqrt(6)),24:dg5([2,2,2,-3,-3],math.sqrt(30))}
def mm(A,B): return [[sum(A[i][k]*B[k][j] for k in range(len(B))) for j in range(len(B[0]))] for i in range(len(A))]
tr2=[0,1,-1,I,-I]; t5=[[1,1,1,-1,-1],tr2]; tt5=[[t5[0][k],t5[1][k]] for k in range(5)]
su5b=[]
for i in range(1,25):
    w=mm(mm(t5,S5[i]),tt5)
    if abs(w[0][0]+w[1][1])<0.3: w[0][0]+=2
    if abs(det(w))<1e-7: continue
    su5b.append(norm(w))
su5=su5b+[inv(m) for m in su5b]
su4raw=json.loads(io.open(os.path.join(_HERE, 'su4_baked.json')).read())
su4=[[[complex(*e[0]),complex(*e[1])],[complex(*e[2]),complex(*e[3])]] for e in su4raw]
# SO(5): the antisymmetric subset of the generalized Gell-Mann set is
# exactly so(5) (10 generators); same reduction tensor + trace plug.
so5b=[]
for i in range(11,21):
    w=mm(mm(t5,S5[i]),tt5)
    if abs(w[0][0]+w[1][1])<0.3: w[0][0]+=2
    if abs(det(w))<1e-7: continue
    so5b.append(norm(w))
so5=so5b+[inv(m) for m in so5b]
allm=su2+su3+su5+su4+so5
off={'su3':len(su2),'su5':len(su2)+len(su3),'su4':len(su2)+len(su3)+len(su5),
     'so5':len(su2)+len(su3)+len(su5)+len(su4)}
n_so5=len(so5)
def f(z): return "%.7f, %.7f" % (z.real, z.imag)
L=["// SU(n) SL(2,C) Mobius groups (Roger Bagula + our reductions). Chaos game",
   "// over a base set + inverses, conjugated by C = dk(delta).s0.qf(theta+i*eta).",
   "// Baked: SU(2) 6-group(12), SU(3) reduced(16), SU(5) reduced(46), SU(4)",
   "// reduced(30), SO(5) reduced(20, the antisymmetric so(5) subset). Custom",
   "// groups (SU2/SU3/SU4) compute the reduction live in the",
   "// init pass from the Reduce sliders and read it from derived slots.",
   "const SU_MOBIUS_BASE: array<vec4<f32>, %d> = array<vec4<f32>, %d>(" % (2*len(allm),2*len(allm))]
for M in allm:
    a,b,c,d=M[0][0],M[0][1],M[1][0],M[1][1]
    L.append("    vec4<f32>(%s), vec4<f32>(%s)," % (f(a)+", "+f(b), f(c)+", "+f(d)))
L.append(");")
r2=1/math.sqrt(2)
L += ["",
"// Baked-group index -> (table offset, generator count) for su_mobius:",
"// 0 SU(2) 6-group, 1 SU(3) reduced, 2 SU(4) reduced, 3 SU(5) reduced,",
"// 4 SO(5) reduced. (su_custom computes its groups in the init pass and",
"// does not consult this table.)",
"fn su_group_range(group: u32) -> vec2<u32> {",
"    switch group {",
"        case 0u: { return vec2<u32>(0u, 12u); }",
"        case 1u: { return vec2<u32>(%du, 16u); }" % off['su3'],
"        case 2u: { return vec2<u32>(%du, 30u); }" % off['su4'],
"        case 3u: { return vec2<u32>(%du, 46u); }" % off['su5'],
"        case 4u: { return vec2<u32>(%du, %du); }" % (off['so5'], n_so5),
"        default: { return vec2<u32>(%du, 16u); }" % off['su3'],
"    }",
"}",
"",
"const SU_S0_AB: vec4<f32> = vec4<f32>(%.7f, 0.0, 0.0, %.7f);" % (r2,-r2),
"const SU_S0_CD: vec4<f32> = vec4<f32>(0.0, %.7f, %.7f, 0.0);" % (-r2,r2),
"",
"struct SuMat { a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32> }",
"fn su_cmul(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> { return vec2<f32>(x.x*y.x-x.y*y.y, x.x*y.y+x.y*y.x); }",
"fn su_cdiv(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> { let dn=dot(y,y)+1e-30; return vec2<f32>(x.x*y.x+x.y*y.y, x.y*y.x-x.x*y.y)/dn; }",
"fn su_matmul(P: SuMat, Q: SuMat) -> SuMat { return SuMat(su_cmul(P.a,Q.a)+su_cmul(P.b,Q.c), su_cmul(P.a,Q.b)+su_cmul(P.b,Q.d), su_cmul(P.c,Q.a)+su_cmul(P.d,Q.c), su_cmul(P.c,Q.b)+su_cmul(P.d,Q.d)); }",
"fn su_matinv(P: SuMat) -> SuMat { let det=su_cmul(P.a,P.d)-su_cmul(P.b,P.c); return SuMat(su_cdiv(P.d,det), su_cdiv(-P.b,det), su_cdiv(-P.c,det), su_cdiv(P.a,det)); }",
"fn su_base(idx: u32) -> SuMat { let ab=SU_MOBIUS_BASE[2u*idx]; let cd=SU_MOBIUS_BASE[2u*idx+1u]; return SuMat(ab.xy, ab.zw, cd.xy, cd.zw); }",
"fn su_conjugator(theta: f32, eta: f32, delta: f32) -> SuMat { let ch=cosh(eta); let sh=sinh(eta); let ct=cos(theta); let st=sin(theta); let ca=vec2<f32>(ct*ch,-st*sh); let sa=vec2<f32>(st*ch,ct*sh); let qf=SuMat(ca, vec2<f32>(-sa.x,-sa.y), sa, ca); let dk=SuMat(vec2<f32>(1.0,delta), vec2<f32>(1.0,0.0), vec2<f32>(1.0,0.0), vec2<f32>(1.0,-delta)); let s0=SuMat(SU_S0_AB.xy, SU_S0_AB.zw, SU_S0_CD.xy, SU_S0_CD.zw); return su_matmul(su_matmul(dk, s0), qf); }",
"fn su_mobius_apply(idx: u32, z: vec2<f32>, cj: SuMat, cji: SuMat) -> vec2<f32> { let m=su_matmul(su_matmul(cj, su_base(idx)), cji); return su_cdiv(su_cmul(m.a,z)+m.b, su_cmul(m.c,z)+m.d); }",
"fn su_apply_m(base: SuMat, z: vec2<f32>, cj: SuMat, cji: SuMat) -> vec2<f32> { let m=su_matmul(su_matmul(cj, base), cji); return su_cdiv(su_cmul(m.a,z)+m.b, su_cmul(m.c,z)+m.d); }",
"fn su_qmul(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> { return vec4<f32>(a.x*b.x-a.y*b.y-a.z*b.z-a.w*b.w, a.x*b.y+a.y*b.x+a.z*b.w-a.w*b.z, a.x*b.z-a.y*b.w+a.z*b.x+a.w*b.y, a.x*b.w+a.y*b.z-a.z*b.y+a.w*b.x); }",
"fn su_qinv(a: vec4<f32>) -> vec4<f32> { let n=dot(a,a)+1e-30; return vec4<f32>(a.x,-a.y,-a.z,-a.w)/n; }",
"fn su_mobius_apply3(idx: u32, p3: vec3<f32>, cj: SuMat, cji: SuMat) -> vec3<f32> { let m=su_matmul(su_matmul(cj, su_base(idx)), cji); let qa=vec4<f32>(m.a,0.0,0.0); let qb=vec4<f32>(m.b,0.0,0.0); let qc=vec4<f32>(m.c,0.0,0.0); let qd=vec4<f32>(m.d,0.0,0.0); let q=vec4<f32>(p3.x,p3.y,abs(p3.z)+1e-4,0.0); let r=su_qmul(su_qmul(qa,q)+qb, su_qinv(su_qmul(qc,q)+qd)); return r.xyz; }",
"fn su_apply_m3(base: SuMat, p3: vec3<f32>, cj: SuMat, cji: SuMat) -> vec3<f32> { let m=su_matmul(su_matmul(cj, base), cji); let qa=vec4<f32>(m.a,0.0,0.0); let qb=vec4<f32>(m.b,0.0,0.0); let qc=vec4<f32>(m.c,0.0,0.0); let qd=vec4<f32>(m.d,0.0,0.0); let q=vec4<f32>(p3.x,p3.y,abs(p3.z)+1e-4,0.0); let r=su_qmul(su_qmul(qa,q)+qb, su_qinv(su_qmul(qc,q)+qd)); return r.xyz; }",
"// Un-conjugated apply (fuchsian_triangle: generators are used raw).",
"fn su_apply_plain(m: SuMat, z: vec2<f32>) -> vec2<f32> { return su_cdiv(su_cmul(m.a,z)+m.b, su_cmul(m.c,z)+m.d); }",
"fn su_apply_plain3(m: SuMat, p3: vec3<f32>) -> vec3<f32> { let qa=vec4<f32>(m.a,0.0,0.0); let qb=vec4<f32>(m.b,0.0,0.0); let qc=vec4<f32>(m.c,0.0,0.0); let qd=vec4<f32>(m.d,0.0,0.0); let q=vec4<f32>(p3.x,p3.y,abs(p3.z)+1e-4,0.0); let r=su_qmul(su_qmul(qa,q)+qb, su_qinv(su_qmul(qc,q)+qd)); return r.xyz; }"]
io.open(os.path.join(_ROOT, 'shaders', 'core', 'su_mobius.wgsl'),'w',encoding='utf-8',newline='').write("\n".join(L)+"\n")
print("wgsl written, matrices", len(allm), "offsets", off)
