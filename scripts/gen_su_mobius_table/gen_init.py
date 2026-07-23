"""Generates the WGSL_INIT body of src/variations/defs/su_custom.rs — the
live reduction-tensor init pass. custom_forms.json holds the closed-form
reduced-matrix entries w[i] = t.s[i].tT as WGSL expressions in the Reduce
sliders a/b/c/d (derived symbolically from the Lie-algebra generator sets).
Group ids match su_custom's enum: 0 SU(2) Custom, 1 SU(3) Custom,
2 SU(4) Custom. Run from the repo root:

    python scripts/gen_su_mobius_table/gen_init.py

writes su_custom_init.wgsl next to this script — paste it into
su_custom.rs's WGSL_INIT if the forms change.
"""
import os
_HERE = os.path.dirname(os.path.abspath(__file__))
import io, json
forms=json.loads(io.open(os.path.join(_HERE, 'custom_forms.json')).read())
def branch(group_id, key):
    fs=forms[key]; M=len(fs); L=[]
    L.append('    if (group == %du) {' % group_id)
    for i,(w00,w01,w10,w11) in enumerate(fs):
        o=i*8; oi=M*8+i*8
        L+= ['        {',
             '            var wa = %s; let wb = %s;' % (w00,w01),
             '            let wc = %s; let wd = %s;' % (w10,w11),
             '            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }',
             '            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);',
             '            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);',
             '            out[%du]=na.x; out[%du]=na.y; out[%du]=nb.x; out[%du]=nb.y; out[%du]=nc.x; out[%du]=nc.y; out[%du]=nd.x; out[%du]=nd.y;'%(o,o+1,o+2,o+3,o+4,o+5,o+6,o+7),
             '            out[%du]=nd.x; out[%du]=nd.y; out[%du]=-nb.x; out[%du]=-nb.y; out[%du]=-nc.x; out[%du]=-nc.y; out[%du]=na.x; out[%du]=na.y;'%(oi,oi+1,oi+2,oi+3,oi+4,oi+5,oi+6,oi+7),
             '        }']
    L.append('    }')
    return "\n".join(L)
init=['fn init_su_custom(user: array<f32, 19>) -> array<f32, 240> {',
      '    let group = u32(user[0]);',
      '    let a = vec2<f32>(user[10], user[11]); let b = vec2<f32>(user[12], user[13]);',
      '    let c = vec2<f32>(user[14], user[15]); let d = vec2<f32>(user[16], user[17]);',
      '    let plug = user[18];',
      '    var out: array<f32, 240>;',
      branch(1,'su3'), branch(0,'su2'), branch(2,'su4'),
      '    return out;','}']
io.open(os.path.join(_HERE, 'su_custom_init.wgsl'),'w',encoding='utf-8').write("\n".join(init)+"\n")
print("init written")
