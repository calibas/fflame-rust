"""Translate built-in variations' WGSL into Rhai twins (rhai-plugins P7
step 3, V2).

For each name: the skeleton the Rust writer made
(`write_variation_twin_skeletons`: its INFO, its PARAMS exactly) gets the
built-in's 2D body as `fn variation(z, p[, ctx])`, and its 3D body as
`fn variation3(v, p[, ctx])` where the 3D body is more than the 2D one with
z passed through. The WGSL comments the skeleton carries are dropped.

What it cannot say (derived parameters, state, a colour read, u32 maths,
pointers it does not know, a switch on a non-literal ...) it refuses, with
the reason, and writes nothing: those are written by hand or wait for
their hook. Every twin it writes is held by the probe's pair compare
(`every_variation_twin_runs_as_its_builtin`), not trusted.

  python scripts/translate_variation_twins.py name [name ...]
  python scripts/translate_variation_twins.py --report names...   # translate nothing, say why not
"""
import json, re, sys

CORPUS = "output/variations-corpus.json"

class Refuse(Exception):
    pass

# ---- tokens -------------------------------------------------------------

TOKEN = re.compile(r"""
    (?P<comment>//[^\n]*)
  | (?P<num>0[xX][0-9a-fA-F]+[ui]?|(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?[fuih]?)
  | (?P<id>[A-Za-z_]\w*)
  | (?P<op>->|&&|\|\||==|!=|<=|>=|<<|>>|\+=|-=|\*=|/=|%=|\+\+|--|[-+*/%<>=!&|^~(){}\[\],;.:?])
  | (?P<ws>\s+)
""", re.X)

def tokens(src):
    out, i = [], 0
    while i < len(src):
        m = TOKEN.match(src, i)
        if not m:
            raise Refuse(f"cannot read WGSL at {src[i:i+20]!r}")
        k = m.lastgroup
        if k != "ws":
            out.append((k, m.group()))
        i = m.end()
    return out

# ---- expressions --------------------------------------------------------

PREC = [("||",), ("&&",), ("|",), ("^",), ("&",), ("==", "!="), ("<", ">", "<=", ">="), ("<<", ">>"), ("+", "-"), ("*", "/", "%")]

class P:
    def __init__(self, toks):
        self.t = [t for t in toks if t[0] != "comment"]
        self.i = 0
    def peek(self, k=0):
        return self.t[self.i + k][1] if self.i + k < len(self.t) else None
    def next(self):
        v = self.t[self.i][1]; self.i += 1; return v
    def eat(self, v):
        if self.peek() != v:
            raise Refuse(f"expected {v!r}, found {self.peek()!r}")
        self.i += 1
    def expr(self, level=0):
        if level == len(PREC):
            return self.unary()
        a = self.expr(level + 1)
        while self.peek() in PREC[level]:
            op = self.next()
            b = self.expr(level + 1)
            a = ("bin", op, a, b)
        return a
    def unary(self):
        if self.peek() in ("-", "!", "~", "*", "&"):
            op = self.next()
            return ("un", op, self.unary())
        return self.postfix(self.primary())
    def primary(self):
        v = self.peek()
        if v == "(":
            self.next(); e = self.expr(); self.eat(")"); return ("paren", e)
        k = self.t[self.i][0]
        if k == "num":
            self.next(); return ("num", v)
        if k == "id":
            self.next()
            # A type's constructor: vec2<f32>(...), array<...>(...).
            if self.peek() == "<" and v in ("vec2", "vec3", "vec4", "array", "mat2x2", "mat3x3", "ptr", "bitcast"):
                depth = 0; ty = v
                while True:
                    w = self.next(); ty += w
                    if w == "<": depth += 1
                    elif w == ">": depth -= 1
                    elif w == ">>": depth -= 2
                    if depth <= 0: break
                v = ty
            if self.peek() == "(":
                self.next(); args = []
                while self.peek() != ")":
                    args.append(self.expr())
                    if self.peek() == ",": self.next()
                self.eat(")")
                return ("call", v, args)
            return ("id", v)
        raise Refuse(f"unexpected {v!r}")
    def postfix(self, e):
        while True:
            if self.peek() == ".":
                self.next(); e = ("field", e, self.next())
            elif self.peek() == "[":
                self.next(); i = self.expr(); self.eat("]"); e = ("index", e, i)
            else:
                return e

# ---- statements ---------------------------------------------------------

def block(p):
    p.eat("{"); out = []
    while p.peek() != "}":
        out.append(stmt(p))
    p.eat("}")
    return out

def stmt(p):
    v = p.peek()
    if v in ("let", "var", "const"):
        p.next(); name = p.next(); ty = None
        if p.peek() == ":":
            p.next(); ty = ""
            depth = 0
            while not (depth == 0 and p.peek() in ("=", ";")):
                w = p.next(); ty += w
                if w == "<": depth += 1
                elif w == ">": depth -= 1
        init = None
        if p.peek() == "=":
            p.next(); init = p.expr()
        p.eat(";")
        return ("decl", v, name, ty, init)
    if v == "if":
        p.next(); c = p.expr(); then = block(p); els = None
        if p.peek() == "else":
            p.next()
            els = [stmt(p)] if p.peek() == "if" else block(p)
        return ("if", c, then, els)
    if v == "for":
        p.next(); p.eat("(")
        init = None if p.peek() == ";" else stmt_simple(p)
        p.eat(";")
        cond = None if p.peek() == ";" else p.expr()
        p.eat(";")
        upd = None if p.peek() == ")" else stmt_simple(p)
        p.eat(")")
        return ("for", init, cond, upd, block(p))
    if v == "while":
        p.next(); c = p.expr(); return ("while", c, block(p))
    if v == "loop":
        p.next(); return ("loop", block(p))
    if v == "switch":
        p.next(); e = p.expr(); p.eat("{"); cases = []
        while p.peek() != "}":
            if p.peek() == "case":
                p.next(); vals = []
                while p.peek() not in (":", "{"):
                    vals.append(p.expr())
                    if p.peek() == ",": p.next()
                if p.peek() == ":": p.next()
                cases.append((vals, block(p)))
            elif p.peek() == "default":
                p.next()
                if p.peek() == ":": p.next()
                cases.append((None, block(p)))
            else:
                raise Refuse(f"switch: {p.peek()!r}")
        p.eat("}")
        return ("switch", e, cases)
    if v == "return":
        p.next(); e = None if p.peek() == ";" else p.expr(); p.eat(";"); return ("return", e)
    if v in ("break", "continue"):
        p.next(); p.eat(";"); return (v,)
    if v == "{":
        return ("block", block(p))
    s = stmt_simple(p); p.eat(";"); return s

def stmt_simple(p):
    if p.peek() in ("let", "var"):
        p.next(); name = p.next(); ty = None
        if p.peek() == ":":
            p.next(); ty = p.next()
        p.eat("="); return ("decl", "var", name, ty, p.expr())
    lhs = p.expr()
    op = p.peek()
    if op in ("=", "+=", "-=", "*=", "/=", "%="):
        p.next(); return ("assign", lhs, op, p.expr())
    if op in ("++", "--"):
        p.next(); return ("assign", lhs, "+=" if op == "++" else "-=", ("num", "1"))
    return ("expr", lhs)

# The structs and module constants of the WGSL being translated (its own
# and the shared helpers'): a struct whose fields share one type, a number
# or a complex number, is an array of it, its fields by their places (the
# user, 2026-11-06: structs as helpers, no struct types); a constant is
# written out where it is read.
STRUCTS = {}
CONSTS = {}

def struct_array(name):
    """A struct as ("f32" | "vec2", {field: place}), or refused."""
    fields = STRUCTS[name]
    tys = {ty.replace(" ", "") for _, ty in fields}
    if len(tys) != 1 or not tys <= {"f32", "vec2<f32>"}:
        raise Refuse(f"a struct of other fields `{name}`")
    return ("vec2" if tys == {"vec2<f32>"} else "f32"), {f: i for i, (f, _) in enumerate(fields)}

def struct_field(field):
    """The place of `field` in whichever array-struct has it, or None."""
    at = {struct_array(s)[1][field] for s in STRUCTS if field in dict(STRUCTS[s]) and len({t.replace(" ", "") for _, t in STRUCTS[s]}) == 1}
    if len(at) > 1:
        raise Refuse(f"a field `.{field}` at two places")
    return at.pop() if at else None

def functions(src):
    """Each `fn name(args) -> ret { body }` in the WGSL: (name, args, ret, body tokens).
    Its structs and constants go to STRUCTS and CONSTS."""
    toks = tokens(src)
    out, i = [], 0
    while i < len(toks):
        if toks[i][1] == "struct":
            name = toks[i + 1][1]
            j = i + 2
            assert toks[j][1] == "{"
            k = j
            while toks[k][1] != "}":
                k += 1
            fields, cur = [], []
            for t in toks[j + 1:k]:
                if t[0] == "comment":
                    continue
                if t[1] == "," and cur.count("<") == cur.count(">"):
                    fields.append("".join(cur)); cur = []
                else:
                    cur.append(t[1])
            if cur: fields.append("".join(cur))
            STRUCTS[name] = [tuple(f.split(":", 1)) for f in fields if ":" in f]
            i = k + 1
            if i < len(toks) and toks[i][1] == ";":
                i += 1
            continue
        if toks[i][1] == "const":
            k, depth = i, 0
            while not (depth == 0 and toks[k][1] == ";"):
                if toks[k][1] in ("(", "{", "["): depth += 1
                elif toks[k][1] in (")", "}", "]"): depth -= 1
                k += 1
            d = stmt(P([t for t in toks[i:k + 1] if t[0] != "comment"]))
            CONSTS[d[2]] = (d[3], d[4])
            i = k + 1
            continue
        if toks[i][1] == "fn":
            name = toks[i + 1][1]
            j = toks.index((toks[i + 2][0], "("), i) if False else i + 2
            assert toks[j][1] == "("
            depth, k = 0, j
            while True:
                if toks[k][1] == "(": depth += 1
                elif toks[k][1] == ")":
                    depth -= 1
                    if depth == 0: break
                k += 1
            args_toks = toks[j + 1:k]
            k += 1
            ret = ""
            if toks[k][1] == "->":
                k += 1
                while toks[k][1] != "{":
                    ret += toks[k][1]; k += 1
            depth, b = 0, k
            while True:
                if toks[b][1] == "{": depth += 1
                elif toks[b][1] == "}":
                    depth -= 1
                    if depth == 0: break
                b += 1
            args = []
            cur = []
            for t in args_toks:
                # By characters: a nested type closes with one `>>` token.
                if t[1] == "," and "".join(cur).count("<") == "".join(cur).count(">"):
                    args.append("".join(cur)); cur = []
                else:
                    cur.append(t[1])
            if cur: args.append("".join(cur))
            args = [a.split(":", 1) for a in args]
            out.append((name, [(a[0], a[1] if len(a) > 1 else "") for a in args], ret, toks[k:b + 1]))
            i = b + 1
        elif toks[i][0] == "comment":
            i += 1
        else:
            raise Refuse(f"outside a function: {toks[i][1]!r}")
    return out

# ---- emitting Rhai ------------------------------------------------------

def array_of(ty):
    """`array<f32, 8>` as ("f32", "8"): an array of numbers or of complex
    numbers (`vec2`), the language's; of `vec3` or `vec4`, an array of
    their floats, flat (`FLAT`); any other refused."""
    m = re.fullmatch(r"array<\s*(f32|i32|u32|vec2<f32>|vec3<f32>|vec4<f32>)\s*,\s*(\w+)\s*>", ty.strip())
    if not m:
        raise Refuse(f"an array `{ty.strip()}`")
    return {"f32": "f32", "i32": "int", "u32": "u32", "vec2<f32>": "vec2", "vec3<f32>": "vec3", "vec4<f32>": "vec4"}[m.group(1)], m.group(2)

# A vector array's floats a vector, flat in the language's array.
FLAT = {"vec3": 3, "vec4": 4}

ZERO_OF = {"f32": "0.0", "int": "0", "u32": "u32(0)", "vec2": "complex(0.0, 0.0)"}

def palette_read(e):
    """`t` of the flame palette's read at `t` as stored --
    `textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t,
    0.5), 0.0).rgb`, a variation's `palette_srgb(t)` -- else None."""
    if e[0] != "field" or e[2] != "rgb" or e[1][0] != "call" or e[1][1] != "textureSampleLevel" or len(e[1][2]) != 4:
        return None
    tex, smp, uv, lod = e[1][2]
    if tex != ("id", "palette_texture") or smp != ("id", "palette_sampler") or lod[0] != "num":
        return None
    if uv[0] != "call" or not uv[1].startswith("vec2") or len(uv[2]) != 2 or uv[2][1][0] != "num" or float(uv[2][1][1].rstrip("f")) != 0.5:
        return None
    return uv[2][0]

def vec_of(ty):
    if ty is None: return None
    if ty.strip() in STRUCTS:
        return "array:" + struct_array(ty.strip())[0]
    # The language's vectors are floats: an integer one is an array of its
    # integers (a helper's pair, read by `.x` and `.y`), never a float one.
    if ty.startswith("vec") and ("u32" in ty or "i32" in ty):
        return "array:" + ("u32" if "u32" in ty else "int")
    if ty.startswith("vec") and "bool" in ty:
        raise Refuse(f"a vector of bools `{ty}`")
    if ty.startswith("vec2"): return "vec2"
    if ty.startswith("vec3"): return "vec3"
    if ty.startswith("vec4"): return "vec4"
    if ty in ("f32", "i32", "u32", "bool"): return ty
    if ty.startswith("array"): raise Refuse("an array")
    # A 3x3 matrix: its nine floats, column by column, an array (as the
    # structs are helpers over arrays; the user, 2026-11-06).
    if ty.replace(" ", "") == "mat3x3<f32>": return "mat3"
    if ty.startswith("mat"): raise Refuse("a matrix of other than 3x3")
    if ty.startswith("ptr"): return "ptr"
    return ty

MATH = {"sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "exp", "log", "exp2", "log2", "sqrt",
        "abs", "sign", "fract", "min", "max", "clamp", "mix", "pow", "atan2", "inverseSqrt", "step", "smoothstep", "fma", "degrees", "radians"}

class Fn:
    def __init__(self, tr, dim, args, ret, helper):
        self.tr, self.dim, self.helper = tr, dim, helper
        self.types = {}
        self.lines = []
        self.temps = 0
        # The function's own names, which a temporary must not take.
        self.taken = set()
        self.uses_ctx = False
        self.ret = ("array:" + array_of(ret)[0]) if ret.replace(" ", "").startswith("array<") else (vec_of(ret.strip()) if ret.strip() else None)
        self.params = []
        self.point = None
        # The init's body (`prepare`): `user[i]` the parameters, `out` its
        # slots, each return the map of them.
        self.init = False
        # The pointers it writes besides the point, each a local from its
        # `ctx` field returned in the map: `vc`'s colour (WritesColor),
        # `hide`'s flag (CanHide). Pointer -> [key, local].
        self.outs = {}
        # Names bound to `transforms[xform_id]`, read for its coefficients.
        self.xform_alias = set()
        # Variables a constant table is copied into whole and never
        # written (`var verts = POLYCHORA_VERTS;`): read as the table.
        self.table_alias = {}
        # A 3D body ending in an `if`: the arms set this, returned after.
        self.result_var = None
        # A helper's pointer argument, written as `this`.
        self.this_ptr = None
        # The constant tables it reads, each a literal at its top
        # (`tbl_<name>`), which the language writes as a module constant.
        self.tables = set()
        self.reads_state = False
        for name, ty in args:
            ty = ty.strip()
            if name == "p" and not helper:
                self.types["p"] = "vec2" if dim == 2 else "vec3"
            elif name == "xform_id" and helper:
                # A helper reading the parameters takes them, `p`, for
                # the ids, as the language passes them whole.
                self.params.append("p")
            elif name in ("xform_id", "variation_id"):
                pass
            elif ty.startswith("ptr<function,RngState>") or ty.startswith("ptr<function, RngState>"):
                pass
            elif helper and ty.startswith("ptr"):
                # A helper's pointer: the value it is called on, `this`, as
                # Rhai binds a method call's (by reference).
                if self.this_ptr:
                    raise Refuse("a helper taking two pointers")
                self.this_ptr = name
                pointee = ty.replace(" ", "")[len("ptr<function,"):-1]
                self.types[name] = ("array:" + array_of(pointee)[0]) if pointee.startswith("array<") else vec_of(pointee)
                if pointee.startswith("array<"):
                    self.types["this"] = self.types[name]
                self.outs[name] = ["this", "this"]
            elif name == "accum":
                self.types["accum"] = vec_of(ty)
            elif name == "vc" and ty.replace(" ", "") == "ptr<function,f32>":
                # The colour: a local from `ctx.color`, returned with the point.
                if helper:
                    raise Refuse("a helper writing the colour")
                self.outs["vc"] = ["color", None]
            elif name == "hide" and ty.replace(" ", "") == "ptr<function,bool>":
                if helper:
                    raise Refuse("a helper hiding the point")
                self.outs["hide"] = ["hide", None]
            elif name == "vrc" and ty.replace(" ", "") == "ptr<function,vec3<f32>>":
                if helper:
                    raise Refuse("a helper writing the direct colour")
                self.outs["vrc"] = ["rgb", None]
            elif ty.startswith("ptr"):
                raise Refuse(f"a pointer argument `{name}: {ty}`")
            elif ty.replace(" ", "").startswith("array<"):
                # An array, by value as the language passes one.
                self.types[name] = "array:" + array_of(ty)[0]
                self.params.append(name)
            else:
                self.types[name] = vec_of(ty)
                self.params.append(name)

    def ty(self, e):
        k = e[0]
        if k == "num":
            if e[1].lower().startswith("0x"):
                return "u32" if e[1].endswith("u") else "int"
            if e[1].endswith("u"):
                return "u32"
            return "f32" if ("." in e[1] or "e" in e[1].lower().rstrip("fhiu")) and not e[1].endswith(("u", "i")) else ("f32" if e[1].endswith("f") else "int")
        if k == "id":
            if e[1] in self.types: return self.types[e[1]]
            if e[1] in ("true", "false"): return "bool"
            if e[1] in CONSTS:
                ty, init = CONSTS[e[1]]
                return vec_of(ty.strip()) if ty else self.ty(init)
            return "f32"
        if k == "paren": return self.ty(e[1])
        if k == "un":
            return "bool" if e[1] == "!" else self.ty(e[2])
        if k == "bin":
            if e[1] in ("==", "!=", "<", ">", "<=", ">=", "&&", "||"):
                return "bool"
            a, b = self.ty(e[2]), self.ty(e[3])
            if e[1] == "*" and (a, b) == ("mat3", "vec3"):
                return "vec3"
            for v in ("vec3", "vec2", "vec4"):
                if v in (a, b): return v
            if "u32" in (a, b) and all(x in ("int", "i32", "u32") for x in (a, b)):
                return "u32"
            if "f32" in (a, b): return "f32"
            return a
        if palette_read(e) is not None:
            return "vec3"
        if k == "field" and self.ty(e[1]).startswith("array:") and struct_field(e[2]) is not None:
            return self.ty(e[1])[len("array:"):]
        if k == "field" and self.ty(e[1]) in ("array:u32", "array:int") and len(e[2]) == 1 and e[2] in "xyzw":
            return self.ty(e[1])[len("array:"):]
        if k == "field":
            base = self.ty(e[1])
            if base in ("vec2", "vec3", "vec4"):
                n = len(e[2])
                return {1: "f32", 2: "vec2", 3: "vec3", 4: "vec4"}[n]
            return "f32"
        if k == "index" and e[1][0] == "id" and e[1][1] in self.table_alias:
            return array_of(CONSTS[self.table_alias[e[1][1]]][0])[0]
        if k == "index" and e[1][0] == "id" and e[1][1] in CONSTS and e[1][1] not in self.types:
            return array_of(CONSTS[e[1][1]][0])[0]
        if k == "index":
            if e[1][0] == "id" and self.types.get(e[1][1], "").startswith("array:"):
                return self.types[e[1][1]][len("array:"):]
            return "f32"
        if k == "call":
            f, args = e[1], e[2]
            if f.startswith("array<"): return "array:" + array_of(f)[0]
            if f in STRUCTS: return "array:" + struct_array(f)[0]
            if f.startswith("vec") and ("u32" in f or "i32" in f): return "array:" + ("u32" if "u32" in f else "int")
            if f.replace(" ", "") == "mat3x3<f32>": return "mat3"
            if f.startswith("vec2"): return "vec2"
            if f.startswith("vec3"): return "vec3"
            if f.startswith("vec4"): return "vec4"
            if f in ("length", "dot", "distance", "get_param", "rng_nextf", "f32", "atan2"): return "f32"
            if f in ("i32", "u32"): return f
            if f == "countOneBits": return "u32"
            if f in ("bitcast<u32>", "bitcast<i32>"): return "u32" if f == "bitcast<u32>" else "int"
            if f == "rng_next": return "u32"
            if f in ("select",): return self.ty(args[1])
            if f == "transpose": return "mat3"
            # A function WGSL takes part by part gives the vector it is of.
            if f in PARTWISE:
                vk = next((k for k in (self.ty(a) for a in args) if k in ("vec2", "vec3", "vec4")), None)
                if vk:
                    return vk
            if f in ("normalize", "cross", "abs", "floor", "ceil", "fract", "sign", "round", "trunc", "sqrt", "exp", "sin", "cos", "min", "max", "clamp", "mix", "pow", "log"):
                return self.ty(args[0]) if args else "f32"
            if f in ("all", "any"): return "bool"
            if f in self.tr.helpers: return self.tr.helpers[f]
            return "f32"
        return "f32"

    def atom(self, e):
        """`e` as a method's receiver: in parentheses unless one already
        (`u - d * 0.2` is not `(u - d * 0.2).abs()`)."""
        text = self.ex(e)
        return text if e[0] in ("id", "paren", "call", "field") else f"({text})"

    def fresh(self):
        """A temporary's name, none the WGSL's own: `let t1` there and a held
        `t1` here would shadow one another."""
        while True:
            self.temps += 1
            n = f"t{self.temps}"
            if n not in self.taken:
                return n

    def hold(self, e):
        """An operand used more than once, made a name."""
        if e[0] in ("id", "num"):
            return self.ex(e)
        n = self.fresh()
        self.lines.append(("let", n, self.ex(e)))
        self.types[n] = self.ty(e)
        return n

    def part(self, s, ty, c):
        """Part c (0, 1, 2) of a value s of type ty."""
        if ty == "vec2":
            return f"{s}.{['re', 'im'][c]}"
        return f"{s}.{'xyzw'[c]}"

    def ex(self, e):
        k = e[0]
        if k == "num" and e[1].lower().startswith("0x"):
            v = e[1].rstrip("ui")
            return f"u32({v})" if e[1].endswith("u") else v
        if k == "num" and e[1].endswith("u"):
            return f"u32({e[1][:-1]})"
        if k == "num":
            v = e[1].rstrip("fhiu") if e[1][-1] in "fhiu" and not e[1][-1].isdigit() else e[1]
            if e[1].endswith("u") or e[1].endswith("i"):
                v = e[1][:-1]
            if e[1].endswith("f"):
                v = e[1][:-1]
            if v.startswith("."):
                v = "0" + v
            if v.endswith("."):
                v = v + "0"
            if ("e" in v or "E" in v) and "." not in v:
                m = re.match(r"(\d+)([eE].*)", v)
                v = f"{m.group(1)}.0{m.group(2)}"
            return v
        if k == "id":
            n = e[1]
            if n == "p" and not self.helper:
                return self.point
            if n in ("PI", "M_PI"): return "PI()"
            if n in ("true", "false"): return n
            if n == "point_w" and not self.helper:
                self.uses_ctx = True
                return "ctx.w"
            if n == "point_w_out" and "point_w_out" in self.outs:
                return self.outs["point_w_out"][1]
            if n == "accum":
                self.uses_ctx = True; return "ctx.accum"
            # A module constant, written out.
            if n in CONSTS and n not in self.types:
                ty, init = CONSTS[n]
                if init is None or (ty and ty.replace(" ", "").startswith("array<")):
                    raise Refuse(f"a constant table `{n}`")
                return self.ex(init)
            return n
        if k == "paren":
            return f"({self.ex(e[1])})"
        if k == "un":
            if e[1] == "*" and e[2][0] == "id" and e[2][1] in self.outs:
                return self.outs[e[2][1]][1]
            if e[1] == "*": raise Refuse("a pointer read")
            if e[1] == "&": raise Refuse("a pointer taken")
            if e[1] == "~":
                if self.ty(e[2]) not in ("int", "i32", "u32"):
                    raise Refuse("a bitwise not of a float")
                return f"({self.ex(e[2])} ^ 0xffffffff)"
            inner = self.ex(e[2])
            return f"{e[1]}{inner}"
        if k == "bin" and e[1] == "*" and (self.ty(e[2]), self.ty(e[3])) == ("mat3", "vec3"):
            m = e[2][1] if e[2][0] == "id" else self.hold(e[2])
            v = self.hold(e[3])
            rows = [" + ".join(f"{m}[{3 * c + r}] * {v}.{'xyz'[c]}" for c in range(3)) for r in range(3)]
            return f"vec3({', '.join(f'({x})' for x in rows)})"
        if k == "bin":
            op = e[1]
            ta, tb = self.ty(e[2]), self.ty(e[3])
            if op in ("<<", ">>", "&", "|", "^") and not (all(x in ("int", "i32", "u32") for x in (ta, tb)) or (ta, tb) == ("bool", "bool")):
                raise Refuse("bit operations of a float")
            vec = [t for t in (ta, tb) if t in ("vec2", "vec3", "vec4")]
            if op in ("==", "!=", "<", ">", "<=", ">=") and vec:
                raise Refuse("a vector compared")
            if "vec2" in vec and op in ("*", "/"):
                # Componentwise, where a complex product is not.
                if ta == "vec2" and tb == "vec2":
                    a, b = self.hold(e[2]), self.hold(e[3])
                    return f"complex({a}.re {op} {b}.re, {a}.im {op} {b}.im)"
                if op == "/" and ta != "vec2":
                    a, b = self.hold(e[2]), self.hold(e[3])
                    return f"complex({a} / {b}.re, {a} / {b}.im)"
            if op == "%" and vec:
                raise Refuse("a vector's remainder")
            if op in ("+", "-") and len(vec) == 1 and (ta in ("f32", "int") or tb in ("f32", "int")):
                a, b = self.hold(e[2]), self.hold(e[3])
                parts = {"vec2": ("re", "im"), "vec3": ("x", "y", "z"), "vec4": ("x", "y", "z", "w")}[vec[0]]
                each = [f"{a + '.' + c if ta == vec[0] else a} {op} {b + '.' + c if tb == vec[0] else b}" for c in parts]
                return {"vec2": "complex", "vec3": "vec3", "vec4": "vec4"}[vec[0]] + f"({', '.join(each)})"
            return f"{self.ex(e[2])} {op} {self.ex(e[3])}"
        if k == "field" and (e[1] == ("index", ("id", "transforms"), ("id", "xform_id")) or (e[1][0] == "id" and e[1][1] in self.xform_alias)):
            if e[2] not in ("a", "b", "c", "d", "e", "f"):
                raise Refuse(f"the transform's `{e[2]}`")
            self.uses_ctx = True
            return f"ctx.{e[2]}"
        # The flame's camera, as the shader holds it: the context's.
        if k == "field" and e[1] == ("id", "params"):
            if e[2] not in CAMERA:
                raise Refuse(f"the flame's `params.{e[2]}`")
            self.uses_ctx = True
            return f"ctx.{CAMERA[e[2]]}"
        # An integer vector's part: its place in the array.
        if k == "field" and self.ty(e[1]) in ("array:u32", "array:int") and len(e[2]) == 1 and e[2] in "xyzw":
            b = e[1][1] if e[1][0] == "id" and e[1][1] not in CONSTS else self.hold(e[1])
            return f"{b}[{'xyzw'.index(e[2])}]"
        # A struct's field: its place in the array.
        if k == "field" and self.ty(e[1]).startswith("array:") and struct_field(e[2]) is not None:
            b = e[1][1] if e[1][0] == "id" and e[1][1] not in CONSTS else self.hold(e[1])
            return f"{b}[{struct_field(e[2])}]"
        # The flame's palette at `t`, as stored.
        if palette_read(e) is not None:
            return f"palette_srgb({self.ex(palette_read(e))})"
        if k == "field":
            base_t = self.ty(e[1])
            sw = e[2]
            if base_t in ("vec2", "vec3", "vec4"):
                if base_t == "vec2" and len(sw) == 1:
                    return f"{self.ex(e[1])}.{ {'x': 're', 'y': 'im', 'r': 're', 'g': 'im'}[sw] }"
                if len(sw) == 1:
                    return f"{self.ex(e[1])}.{sw}"
                b = self.hold(e[1])
                parts = [self.part(b, base_t, "xyzw".index(c) if c in "xyzw" else "rgba".index(c)) for c in sw]
                if len(sw) == 2: return f"complex({parts[0]}, {parts[1]})"
                if len(sw) == 3: return f"vec3({', '.join(parts)})"
                return f"rgba({', '.join(parts)})"
            raise Refuse(f"a field `.{sw}`")
        if k == "index" and e[1][0] == "id" and e[1][1] in self.table_alias:
            return self.table_read(self.table_alias[e[1][1]], e[2])
        if k == "index" and e[1][0] == "id" and e[1][1] in CONSTS and e[1][1] not in self.types:
            return self.table_read(e[1][1], e[2])
        if k == "index" and e[1][0] == "index" and e[1][1][0] == "id" and self.types.get(e[1][1][1]) == "mat3":
            return f"{e[1][1][1]}[3 * {self.ex(e[1][2])} + {self.ex(e[2])}]"
        if k == "index" and e[1][0] == "id" and self.types.get(e[1][1], "")[len("array:"):] in FLAT:
            elem = self.types[e[1][1]][len("array:"):]
            i = self.ex(e[2]) if e[2][0] in ("id", "num") else self.hold(e[2])
            n = FLAT[elem]
            return f"{elem}(" + ", ".join(f"{e[1][1]}[{n} * {i} + {c}]" for c in range(n)) + ")"
        if k == "index" and e[1][0] == "id" and self.types.get(e[1][1], "").startswith("array:"):
            return f"{e[1][1]}[{self.ex(e[2])}]"
        if k == "index" and self.init and self.tr.slotted and e[1] == ("id", "out"):
            return f"out[{self.tr.n_slots()} + {self.slot_index(e[2])}]"
        if k == "index" and self.init and self.tr.slotted and e[1] == ("id", "user"):
            return f"out[{self.slot_index(e[2])}]" if e[2][0] != "num" else self.tr.param_read(int(e[2][1].rstrip("ui")))
        if k == "index" and self.init and e[1] in (("id", "user"), ("id", "out")):
            if e[2][0] != "num":
                raise Refuse("the init's slots by a computed index")
            i = int(e[2][1].rstrip("ui"))
            return self.tr.param_read(i) if e[1] == ("id", "user") else f"out[{i}]"
        if k == "index":
            # The variation's own weight, JWildfire's inside the formula.
            if e[2] == ("id", "variation_id") and e[1][0] == "field" and e[1][2] == "variations" and (
                e[1][1] == ("index", ("id", "transforms"), ("id", "xform_id")) or (e[1][1][0] == "id" and e[1][1][1] in self.xform_alias)
            ):
                self.uses_ctx = True
                return "ctx.weight"
            raise Refuse("an index")
        if k == "call":
            return self.call(e)
        raise Refuse(f"an expression {k}")

    def call(self, e):
        f, args = e[1], e[2]
        if f == "countOneBits":
            if self.ty(args[0]) != "u32":
                raise Refuse("`countOneBits` of other than a u32")
            self.tr.written.add("count_one_bits")
            return f"count_one_bits({self.ex(args[0])})"
        if f.startswith("bitcast<"):
            src = self.ty(args[0])
            if f == "bitcast<u32>" and src in ("int", "i32", "u32"):
                return f"u32({self.ex(args[0])})"
            if f == "bitcast<i32>" and src in ("int", "i32", "u32"):
                return f"to_int({self.ex(args[0])})" if src == "u32" else self.ex(args[0])
            raise Refuse("a float's bits (`bitcast`), which the language has not")
        # A struct built: the array of its fields.
        if f in STRUCTS:
            struct_array(f)
            return "[" + ", ".join(self.ex(a) for a in args) + "]"
        if f == "transpose":
            if self.ty(args[0]) != "mat3":
                raise Refuse("a transpose of other than a 3x3 matrix")
            m = args[0][1] if args[0][0] == "id" else self.hold(args[0])
            return "[" + ", ".join(f"{m}[{3 * r + c}]" for c in range(3) for r in range(3)) + "]"
        if f.replace(" ", "") == "mat3x3<f32>":
            if len(args) == 3 and all(self.ty(a) == "vec3" for a in args):
                parts = []
                for a in args:
                    if a[0] == "call" and a[1].startswith("vec3") and len(a[2]) == 3:
                        parts += [self.ex(x) for x in a[2]]
                    else:
                        c = self.hold(a)
                        parts += [f"{c}.{x}" for x in "xyz"]
                return "[" + ", ".join(parts) + "]"
            if len(args) == 9:
                return "[" + ", ".join(self.ex(a) for a in args) + "]"
            raise Refuse("a matrix built otherwise")
        if f.startswith("array<"):
            elem, _ = array_of(f)
            if elem in FLAT:
                parts = []
                for a in args:
                    if a[0] == "call" and a[1].startswith(elem) and len(a[2]) == FLAT[elem] and all(not self.ty(x).startswith(("vec", "array")) for x in a[2]):
                        parts += [self.ex(x) for x in a[2]]
                    else:
                        v = self.hold(a)
                        parts += [f"{v}.{c}" for c in "xyzw"[:FLAT[elem]]]
                return "[" + ", ".join(parts) + "]"
            return "[" + ", ".join(self.ex(a) for a in args) + "]"
        if f.startswith("vec") and ("u32" in f or "i32" in f):
            # An integer vector: the array of its parts, each its type.
            n = int(f[3])
            conv = (lambda x: f"u32({x})") if "u32" in f else (lambda x: f"to_int({x})")
            def part(a):
                t = self.ty(a)
                want = "u32" if "u32" in f else "int"
                return self.ex(a) if t == want or (want == "int" and t == "i32") else conv(self.ex(a))
            if len(args) == 1:
                a = self.hold(args[0])
                return "[" + ", ".join([part(("id", a))] * n) + "]"
            if len(args) != n or any(self.ty(a).startswith(("vec", "array")) for a in args):
                raise Refuse(f"an integer vector `{f}` of other parts")
            return "[" + ", ".join(part(a) for a in args) + "]"
        if f.startswith("vec") and "bool" in f:
            raise Refuse(f"a vector of bools `{f}`")
        if f.startswith("vec2"):
            if len(args) == 1:
                a = self.hold(args[0]); return f"complex({a}, {a})"
            return f"complex({self.ex(args[0])}, {self.ex(args[1])})"
        if f.startswith("vec3"):
            if len(args) == 1:
                a = self.hold(args[0]); return f"vec3({a}, {a}, {a})"
            if len(args) == 2:
                # vec3(vec2, f32) or vec3(f32, vec2)
                t0 = self.ty(args[0])
                if t0 == "vec2":
                    a = self.hold(args[0]); return f"vec3({a}.re, {a}.im, {self.ex(args[1])})"
                b = self.hold(args[1]); return f"vec3({self.ex(args[0])}, {b}.re, {b}.im)"
            return f"vec3({', '.join(self.ex(a) for a in args)})"
        if f.startswith("vec4"):
            # The language's `vec4`, its four-part kind: part by part, as
            # WGSL's (`*`, `/` and a number's scaling too).
            kinds = [self.ty(a) for a in args]
            if len(args) == 1 and kinds[0] == "vec4":
                return self.ex(args[0])
            if len(args) == 1:
                a = self.hold(args[0])
                return f"vec4({a}, {a}, {a}, {a})"
            parts = []
            for a, k in zip(args, kinds):
                if k in ("vec2", "vec3"):
                    h = self.hold(a)
                    parts += [self.part(h, k, c) for c in range(2 if k == "vec2" else 3)]
                elif k == "vec4":
                    raise Refuse("a vec4 of a vec4 and more")
                else:
                    parts.append(self.ex(a))
            if len(parts) != 4:
                raise Refuse("a vec4 of other than four parts")
            return f"vec4({', '.join(parts)})"
        if f == "get_param":
            slot = args[2]
            if slot[0] != "num" and self.tr.slotted:
                i = self.ex(slot)
                return f"p.slots[{f'to_int({i})' if self.ty(slot) == 'u32' else i}]"
            if slot[0] != "num":
                raise Refuse("a parameter by a computed slot")
            return self.tr.param_read(int(slot[1].rstrip("u")))
        if f == "rng_nextf":
            return "rand()"
        # The raw draw, the stream's next 32 bits.
        if f == "rng_next":
            return "rand_u32()"
        # The project's IEEE-exact atan2 is the language's.
        if f == "ff_atan2":
            return f"atan2({self.ex(args[0])}, {self.ex(args[1])})"
        if f == "sign" and self.ty(args[0]) == "f32":
            x = self.hold(args[0])
            return f"(if {x} > 0.0 {{ 1.0 }} else if {x} < 0.0 {{ -1.0 }} else {{ 0.0 }})"
        if f == "step" and all(self.ty(a) == "f32" for a in args):
            return f"(if {self.ex(args[1])} < {self.ex(args[0])} {{ 0.0 }} else {{ 1.0 }})"
        if f == "fma" and all(self.ty(a) == "f32" for a in args):
            return f"({self.ex(args[0])} * {self.ex(args[1])} + {self.ex(args[2])})"
        if f == "exp2" and self.ty(args[0]) == "f32":
            return f"pow(2.0, {self.ex(args[0])})"
        if f == "radians" and self.ty(args[0]) == "f32":
            return f"({self.ex(args[0])} * 0.017453292519943295)"
        if f == "degrees" and self.ty(args[0]) == "f32":
            return f"({self.ex(args[0])} * 57.29577951308232)"
        if f == "get_state":
            self.reads_state = True
            return f"this.{self.tr.state_field(args[2], self)}"
        if f == "set_state":
            raise Refuse("a `set_state` inside an expression")
        if f == "select":
            a, b, c = args
            return f"(if {self.ex(c)} {{ {self.ex(b)} }} else {{ {self.ex(a)} }})"
        if f in ("f32",):
            t = self.ty(args[0])
            if t in ("f32",): return self.ex(args[0])
            return f"to_float({self.ex(args[0])})"
        if f in ("i32",):
            return f"to_int({self.ex(args[0])})"
        if f == "u32":
            return self.ex(args[0]) if self.ty(args[0]) == "u32" else f"u32({self.ex(args[0])})"
        if f in ("bitcast", "bool"):
            raise Refuse(f"`{f}`")
        t0 = self.ty(args[0]) if args else None
        if f == "length" and t0 == "vec4":
            h = self.hold(args[0])
            return f"sqrt({h}.x * {h}.x + {h}.y * {h}.y + {h}.z * {h}.z + {h}.w * {h}.w)"
        if f == "dot" and t0 == "vec4":
            x, y = self.hold(args[0]), self.hold(args[1])
            return f"({x}.x * {y}.x + {x}.y * {y}.y + {x}.z * {y}.z + {x}.w * {y}.w)"
        if f == "length":
            if t0 == "vec2": return f"{self.atom(args[0])}.abs()"
            return f"length({self.ex(args[0])})"
        if f == "dot":
            if t0 == "vec2":
                a, b = args
                if a == b:
                    return f"{self.atom(a)}.norm_sqr()"
                x, y = self.hold(a), self.hold(b)
                return f"({x}.re * {y}.re + {x}.im * {y}.im)"
            return f"dot({self.ex(args[0])}, {self.ex(args[1])})"
        if f == "distance":
            if t0 == "vec2":
                return f"({self.ex(args[0])} - {self.ex(args[1])}).abs()"
            return f"length({self.ex(args[0])} - {self.ex(args[1])})"
        # A function WGSL takes part by part, of a vector: written out so,
        # where the language has no such vector form or its complex one is
        # another function (`sin` of a complex is the complex sine).
        kinds = [self.ty(a) for a in args]
        vk = next((k for k in kinds if k in ("vec2", "vec3", "vec4")), None)
        native = (
            f in ("min", "max", "abs")
            or (f == "clamp" and not any(k in ("vec2", "vec3") for k in kinds[1:]))
            or (f == "mix" and len(kinds) == 3 and kinds[2] not in ("vec2", "vec3"))
            or (f in ("floor", "ceil", "trunc", "smoothstep", "fract") and vk == "vec2")
        )
        if vk and f in PARTWISE and not native:
            if any(k in ("vec2", "vec3") and k != vk for k in kinds):
                raise Refuse(f"`{f}` of two kinds of vector")
            held = [a if a[0] in ("id", "num") else ("id", self.hold(a)) for a in args]
            comps = {"vec2": ("x", "y"), "vec3": ("x", "y", "z"), "vec4": ("x", "y", "z", "w")}[vk]
            parts = [self.ex(("call", f, [("field", h, c) if k == vk else h for h, k in zip(held, kinds)])) for c in comps]
            return {"vec2": "complex", "vec3": "vec3", "vec4": "vec4"}[vk] + f"({', '.join(parts)})"
        if f in ("floor", "ceil", "trunc"):
            if t0 == "vec2": return f"{f}({self.ex(args[0])})"
            if t0 == "vec3": raise Refuse(f"`{f}` of a vec3")
            return f"to_float({f}({self.ex(args[0])}))"
        if f == "round":
            return f"round_even({self.ex(args[0])})"
        if f == "inverseSqrt":
            if t0 != "f32": raise Refuse("inverseSqrt of a vector")
            return f"(1.0 / sqrt({self.ex(args[0])}))"
        if f in ("all", "any"):
            raise Refuse(f"`{f}`")
        if f in ("min", "max", "clamp", "abs", "sign") and t0 == "vec2":
            parts = []
            held = [self.hold(a) if self.ty(a) == "vec2" else self.ex(a) for a in args]
            for c in ("re", "im"):
                parts.append(f"{f}({', '.join(h + '.' + c if self.ty(a) == 'vec2' else h for h, a in zip(held, args))})")
            return f"complex({parts[0]}, {parts[1]})"
        if f in ("sin", "cos", "tan", "exp", "log", "sqrt", "pow", "atan", "sinh", "cosh", "tanh") and t0 in ("vec2", "vec3"):
            raise Refuse(f"`{f}` of a vector")
        if f == "smoothstep" and all(self.ty(a) in ("f32", "vec2") for a in args):
            # WGSL's: t = clamp((x - e0) / (e1 - e0), 0, 1), then t·t·(3 - 2t);
            # a complex one part by part.
            held = [self.hold(a) for a in args]
            parts = [""] if self.ty(args[2]) == "f32" else [".re", ".im"]
            out = []
            for c in parts:
                e0, e1, x = (h + c if self.ty(a) == "vec2" else h for h, a in zip(held, args))
                t = self.fresh()
                self.lines.append(("let", t, f"clamp(({x} - {e0}) / ({e1} - {e0}), 0.0, 1.0)"))
                self.types[t] = "f32"
                out.append(f"{t} * {t} * (3.0 - 2.0 * {t})")
            return f"({out[0]})" if len(out) == 1 else f"complex({out[0]}, {out[1]})"
        if f in MATH:
            if f in ("smoothstep",):
                raise Refuse(f"`{f}`")
            # WGSL's log is the natural one; Rhai's `log` is base 10.
            name = "ln" if f == "log" else f
            # Rhai's `max(a, b)` is `t = a; if b > t { t = b }`, keeping a
            # NaN `a`; WGSL's gives the other operand, so `max(x, 0.0)` is
            # a guard. A literal is never NaN: first, it is WGSL's exactly.
            if f in ("min", "max") and len(args) == 2 and t0 == "f32" and literal(args[1]) and not literal(args[0]):
                args = [args[1], args[0]]
            return f"{name}({', '.join(self.ex(a) for a in args)})"
        # A 2D vector normalized: WGSL's `v / length(v)`, as a complex.
        if f == "normalize" and self.ty(args[0]) == "vec2":
            x = self.hold(args[0])
            return f"({x} / {x}.abs())"
        if f in ("normalize", "cross"):
            return f"{f}({', '.join(self.ex(a) for a in args)})"
        if f in self.tr.helpers:
            hargs = self.tr.helper_args[f]
            pointer = lambda ty: ty.strip().startswith("ptr") and "RngState" not in ty
            valptr = self.tr.helper_valptr.get(f, set())
            def arg(a, n):
                if n == "xform_id":
                    return "p"
                if n in valptr:
                    # An array by value: what the pointer points at.
                    if a[0] == "un" and a[1] == "&" and a[2][0] == "id":
                        return self.ex(a[2])
                    if a[0] == "id" and a[1] in self.outs:
                        return self.outs[a[1]][1]
                    if a[0] == "id" and a[1] == self.this_ptr:
                        return "this"
                    if a[0] == "id" and self.types.get(a[1], "").startswith("array:"):
                        return a[1]
                    raise Refuse(f"a pointer of another kind to `{f}`")
                return self.ex(a)
            keep = [arg(a, n) for a, (n, ty) in zip(args, hargs) if not ("RngState" in ty or n == "variation_id" or pointer(ty))]
            if f in self.tr.helper_ctx:
                keep.append("ctx")
                self.uses_ctx = True
            ptrs = [a for a, (n, ty) in zip(args, hargs) if pointer(ty)]
            if f in self.tr.helper_state:
                # Called on the state.
                self.reads_state = True
                return f"this.{f}({', '.join(keep)})"
            if ptrs:
                # Called on what its pointer points at.
                a = ptrs[0]
                if a[0] == "id" and a[1] in self.outs:
                    recv = self.outs[a[1]][1]
                elif a[0] == "un" and a[1] == "&" and a[2][0] == "id":
                    recv = self.ex(a[2])
                else:
                    raise Refuse(f"a pointer of another kind to `{f}`")
                return f"{recv}.{f}({', '.join(keep)})"
            return f"{f}({', '.join(keep)})"
        raise Refuse(f"a call to `{f}`")

    def slot_index(self, e):
        """An index into an array of the language's, an int."""
        i = self.ex(e)
        return f"to_int({i})" if self.ty(e) == "u32" else i

    def table_read(self, name, idx):
        """An element of the constant table `name`: of its local, a
        vector's floats flat."""
        ty, init = CONSTS[name]
        if not ty or init is None:
            raise Refuse(f"a constant table `{name}` without its type")
        elem, _ = array_of(ty)
        self.tables.add(name)
        local = "tbl_" + name.lower()
        if elem in FLAT:
            i = self.ex(idx) if idx[0] in ("id", "num") else self.hold(idx)
            k = FLAT[elem]
            return f"{elem}(" + ", ".join(f"{local}[{k} * {i} + {c}]" for c in range(k)) + ")"
        return f"{local}[{self.ex(idx)}]"

    def table_lines(self):
        """Each table it reads, its literal bound at the top."""
        out = []
        for name in sorted(self.tables):
            ty, init = CONSTS[name]
            elem, _ = array_of(ty)
            if init[0] != "call" or not init[1].startswith("array<"):
                raise Refuse(f"a constant table `{name}` not of literals")
            parts = []
            for a in init[2]:
                if elem in FLAT:
                    if a[0] != "call" or len(a[2]) != FLAT[elem]:
                        raise Refuse(f"a constant table `{name}` not of literals")
                    parts += [self.ex(x) for x in a[2]]
                else:
                    parts.append(self.ex(a))
            out.append(("raw", f"    let tbl_{name.lower()} = [{', '.join(parts)}];"))
        return out

    def result_map(self, point):
        """`#{ z: point, color: .., hide: .. }`, the pointers' locals."""
        return "#{ z: " + point + "".join(f", {k}: {v}" for k, v in self.outs.values()) + " }"

    def stmts(self, ss, depth, last=False):
        for i, s in enumerate(ss):
            self.stmt(s, depth, last and i == len(ss) - 1)

    def emit(self, depth, text):
        self.lines.append(("raw", "    " * depth + text))

    def flush_temps(self, depth):
        pending = [l for l in self.lines if l[0] == "let"]
        if pending:
            out = []
            for l in self.lines:
                if l[0] == "let":
                    out.append(("raw", "    " * depth + f"let {l[1]} = {l[2]};"))
                else:
                    out.append(l)
            self.lines = out

    def line(self, depth, make):
        """A statement whose expressions may hold temporaries: those first."""
        before = len(self.lines)
        text = make()
        lets = [l for l in self.lines[before:] if l[0] == "let"]
        self.lines = self.lines[:before]
        for l in lets:
            self.lines.append(("raw", "    " * depth + f"let {l[1]} = {l[2]};"))
        self.lines.append(("raw", "    " * depth + text))

    def stmt(self, s, depth, last):
        k = s[0]
        if self.init and self.tr.slotted and k == "decl" and s[2] == "out":
            # The parameters first, then the built-in's derived values.
            self.emit(depth, f"let out = array({self.tr.n_slots() + self.tr.d['init_param_count']}, 0.0);")
            for i in range(self.tr.n_slots()):
                self.emit(depth, f"out[{i}] = {self.tr.param_read(i)};")
            return
        if self.init and k == "decl" and s[2] == "out":
            self.emit(depth, f"let out = array({self.tr.d['init_param_count']}, 0.0);")
            return
        if self.init and k == "assign" and s[1][0] == "index" and s[1][1] == ("id", "out"):
            self.line(depth, lambda: f"{self.ex(s[1])} {s[2]} {self.ex(s[3])};")
            return
        if self.init and self.tr.slotted and k == "return" and s[1] == ("id", "out"):
            self.emit(depth, "#{ slots: out }" if last else "return #{ slots: out };")
            return
        if self.init and k == "return" and s[1] == ("id", "out"):
            slots = ", ".join(f"{n}: out[{i}]" for i, n in enumerate(self.tr.derived_list()))
            self.emit(depth, f"#{{ {slots} }}" if last else f"return #{{ {slots} }};")
            return
        if k == "decl" and s[4] == ("index", ("id", "transforms"), ("id", "xform_id")):
            # `let xf = transforms[xform_id];`: its reads are `ctx`'s.
            self.xform_alias.add(s[2])
            return
        if k == "decl" and s[3] and s[3].replace(" ", "").startswith("array<"):
            elem, n = array_of(s[3])
            self.types[s[2]] = "array:" + elem
            if s[4] is None and elem in FLAT:
                self.emit(depth, f"let {s[2]} = array({n} * {FLAT[elem]}, 0.0);")
            elif s[4] is None:
                self.emit(depth, f"let {s[2]} = array({n}, {ZERO_OF[elem]});")
            else:
                self.line(depth, lambda: f"let {s[2]} = {self.ex(s[4])};")
            return
        if k == "decl" and s[4] is not None and s[4][0] == "id" and s[4][1] in CONSTS and s[4][1] not in self.types \
                and (CONSTS[s[4][1]][0] or "").replace(" ", "").startswith("array<"):
            # A whole table copied into a variable (one copy where a read of
            # the constant at a run-time index copies it each time): its
            # reads are the table's.
            self.table_alias[s[2]] = s[4][1]
            return
        if k == "decl":
            _, kind, name, ty, init = s
            t = vec_of(ty.strip()) if ty else (self.ty(init) if init else None)
            if t in ("i32",) and init is not None and self.ty(init) == "int":
                t = "int"
            self.types[name] = "f32" if t == "int" and ty is None and init is not None and self.ty(init) == "f32" else t
            if init is None and t and t.startswith("array:") and ty and ty.strip() in STRUCTS:
                self.emit(depth, f"let {name} = array({len(STRUCTS[ty.strip()])}, {ZERO_OF[t[len('array:'):]]});")
                return
            if init is None:
                zero = {"vec2": "complex(0.0, 0.0)", "vec3": "vec3(0.0, 0.0, 0.0)", "vec4": "vec4(0.0, 0.0, 0.0, 0.0)", "f32": "0.0", "i32": "0", "u32": "u32(0)", "bool": "false"}.get(t)
                if zero is None: raise Refuse(f"a `var` of `{ty}`")
                self.emit(depth, f"let {name} = {zero};")
            else:
                self.line(depth, lambda: f"let {name} = {self.ex(init)};")
            return
        if k == "assign":
            _, lhs, op, rhs = s
            base = lhs
            while base[0] in ("index", "field"):
                base = base[1]
            if base[0] == "id" and base[1] in self.table_alias:
                raise Refuse("a table's copy written")
            # WGSL's `_ = e;`: e evaluated (a draw kept in the stream).
            if lhs == ("id", "_"):
                self.temps += 1
                self.line(depth, lambda: f"let unused{self.temps} = {self.ex(rhs)};")
                return
            if lhs[0] == "field" and lhs[1][0] == "id" and self.ty(lhs[1]) == "vec2" and len(lhs[2]) == 1:
                n = lhs[1][1]
                part = {"x": "re", "y": "im"}[lhs[2]]
                other = {"re": "im", "im": "re"}[part]
                value = (lambda: self.ex(rhs)) if op == "=" else (lambda: f"{n}.{part} {op[0]} ({self.ex(rhs)})")
                self.line(depth, lambda: f"{n} = complex({value()}, {n}.im);" if part == "re" else f"{n} = complex({n}.re, {value()});")
                return
            if self.init and self.tr.slotted and lhs[0] == "index" and lhs[1] == ("id", "out"):
                self.line(depth, lambda: f"out[{self.tr.n_slots()} + {self.slot_index(lhs[2])}] {op} {self.ex(rhs)};")
                return
            if lhs[0] == "index" and lhs[1][0] == "id" and self.types.get(lhs[1][1], "")[len("array:"):] in FLAT:
                elem = self.types[lhs[1][1]][len("array:"):]
                if op != "=":
                    raise Refuse("a vector array's element set by an operator")
                k = FLAT[elem]
                # The index and the vector held once (inside `line`, which
                # writes the temporaries before the statement).
                held = {}
                def first():
                    held["i"] = self.ex(lhs[2]) if lhs[2][0] in ("id", "num") else self.hold(lhs[2])
                    held["v"] = self.hold(rhs)
                    return f"{lhs[1][1]}[{k} * {held['i']} + 0] = {held['v']}.x;"
                self.line(depth, first)
                for c in range(1, k):
                    self.emit(depth, f"{lhs[1][1]}[{k} * {held['i']} + {c}] = {held['v']}.{'xyzw'[c]};")
                return
            if lhs[0] == "index" and lhs[1][0] == "id" and self.types.get(lhs[1][1], "").startswith("array:"):
                self.line(depth, lambda: f"{lhs[1][1]}[{self.ex(lhs[2])}] {op} {self.ex(rhs)};")
                return
            if lhs[0] == "field" and lhs[1][0] == "id" and self.ty(lhs[1]).startswith("array:") and struct_field(lhs[2]) is not None:
                self.line(depth, lambda: f"{lhs[1][1]}[{struct_field(lhs[2])}] {op} {self.ex(rhs)};")
                return
            if lhs == ("id", "point_w_out") and "point_w_out" in self.outs:
                self.line(depth, lambda: f"{self.outs['point_w_out'][1]} {op} {self.ex(rhs)};")
                return
            if lhs[0] == "un" and lhs[1] == "*" and lhs[2][0] == "id" and lhs[2][1] in self.outs:
                self.line(depth, lambda: f"{self.outs[lhs[2][1]][1]} {op} {self.ex(rhs)};")
                return
            if lhs[0] == "un" and lhs[1] == "*":
                raise Refuse("a write through a pointer")
            if lhs[0] != "id":
                if lhs[0] == "field" and self.ty(lhs[1]) in ("vec3", "vec4") and len(lhs[2]) == 1:
                    n = self.ex(lhs[1])
                    self.line(depth, lambda: f"{n}.{lhs[2]} {op} {self.ex(rhs)};")
                    return
                raise Refuse("an assignment to a part")
            if op in ("*=", "/=") and self.ty(lhs) == "vec2" and self.ty(rhs) == "vec2":
                raise Refuse("a componentwise compound assignment")
            self.line(depth, lambda: f"{self.ex(lhs)} {op} {self.ex(rhs)};")
            return
        if k == "expr" and s[1][0] == "call" and s[1][1] == "set_state":
            self.reads_state = True
            field = self.tr.state_field(s[1][2][2], self)
            self.line(depth, lambda: f"this.{field} = {self.ex(s[1][2][3])};")
            return
        # An emission: the language's statement (`INFO.emits` its cap).
        if k == "expr" and s[1][0] == "call" and s[1][1] in EMITS:
            name, args = EMITS[s[1][1]], s[1][2]
            self.line(depth, lambda: f"{name}({', '.join(self.ex(a) for a in args)});")
            return
        # A helper called for what it writes through its pointer.
        if k == "expr" and s[1][0] == "call" and s[1][1] in self.tr.helpers:
            self.line(depth, lambda: f"{self.ex(s[1])};")
            return
        if k == "expr":
            raise Refuse("a statement of an expression")
        if k == "if":
            _, c, then, els = s
            # The last statement, its arms each ending in a return: the
            # result, an `if` as a value.
            tail = last and ends_in_return(s)
            self.line(depth, lambda: f"if {self.ex(c)} {{")
            self.stmts(then, depth + 1, tail)
            while els is not None:
                if len(els) == 1 and els[0][0] == "if":
                    _, c2, then2, els2 = els[0]
                    self.line(depth, lambda: f"}} else if {self.ex(c2)} {{")
                    self.stmts(then2, depth + 1, tail)
                    els = els2
                else:
                    self.emit(depth, "} else {")
                    self.stmts(els, depth + 1, tail)
                    els = None
            self.emit(depth, "}")
            return
        if k == "for":
            _, init, cond, upd, body = s
            # for (var i = a; i < b; i++) -> for i in a..b
            step_one = init and init[0] == "decl" and any(upd in (
                ("assign", ("id", init[2]), "+=", ("num", one)),
                ("assign", ("id", init[2]), "=", ("bin", "+", ("id", init[2]), ("num", one))),
            ) for one in ("1", "1u", "1i"))
            # Rhai's range is taken once; WGSL tests its bound every pass.
            bound_fixed = cond and cond[0] == "bin" and not any(assigns(body, n) for n in re.findall(r"'id', '(\w+)'", repr(cond[3])))
            if step_one and init[4] is not None and self.ty(init[4]) != "f32" and cond and cond[0] == "bin" and cond[1] in ("<", "<=") and cond[2] == ("id", init[2]) \
                    and not assigns(body, init[2]) and bound_fixed:
                name = init[2]
                rng = ".." if cond[1] == "<" else "..="
                # A range counts in ints: a u32 bound by `to_int` (its
                # bits; a counter's are small), and a u32 counter a u32 in
                # the body again, as WGSL's is.
                unsigned = (init[3] or "").strip() == "u32" or self.ty(init[4]) == "u32"
                def bound(e):
                    if e[0] == "num":
                        return e[1].rstrip("ui")
                    return f"to_int({self.ex(e)})" if self.ty(e) == "u32" else self.ex(e)
                counter = next(n for n in (f"{name}_n", f"{name}_k", f"{name}_i") if n not in self.taken) if unsigned else name
                self.line(depth, lambda: f"for {counter} in {bound(init[4])}{rng}{bound(cond[3])} {{")
                if unsigned:
                    # Each range's counter its own: sibling loops share it.
                    self.emit(depth + 1, f"let {name} = u32({counter});")
                self.types[name] = "u32" if unsigned else "int"
                self.stmts(body, depth + 1)
                self.emit(depth, "}")
                return
            # Any other: a `while`, its step last -- which a `continue`
            # would skip.
            if init and cond and upd and upd[0] == "assign" and "'continue'" not in repr(body):
                self.stmt(init, depth, False)
                self.line(depth, lambda: f"while {self.ex(cond)} {{")
                self.stmts(body, depth + 1)
                self.stmt(upd, depth + 1, False)
                self.emit(depth, "}")
                return
            raise Refuse("a `for` that is not a counted range")
        if k == "while":
            _, c, body = s
            self.line(depth, lambda: f"while {self.ex(c)} {{")
            self.stmts(body, depth + 1)
            self.emit(depth, "}")
            return
        if k == "loop":
            # WGSL's `loop`, left by its `break`s: the language's
            # `while true`, under its loop cap as any.
            self.emit(depth, "while true {")
            self.stmts(s[1], depth + 1)
            self.emit(depth, "}")
            return
        if k == "switch":
            _, e, cases = s
            v = self.fresh()
            self.line(depth, lambda: f"let {v} = {self.ex(e)};")
            self.types[v] = self.ty(e)
            first = True
            dflt = None
            # Every arm returning, the switch last: the function's end.
            tail = last and ends_in_return(s)
            for vals, body in cases:
                if vals is None:
                    dflt = body; continue
                cond = " || ".join(f"{v} == {self.ex(x)}" for x in vals)
                self.emit(depth, ("if " if first else "} else if ") + cond + " {")
                self.stmts(body, depth + 1, tail)
                first = False
            if dflt is not None:
                if first:
                    self.stmts(dflt, depth, tail)
                else:
                    self.emit(depth, "} else {")
                    self.stmts(dflt, depth + 1, tail)
            if not first:
                self.emit(depth, "}")
            return
        if k == "return":
            e = s[1]
            if e is None:
                raise Refuse("a bare return")
            value = (lambda: self.result_map(self.ex(e))) if self.outs and not self.helper else (lambda: self.ex(e))
            if last and self.result_var:
                self.line(depth, lambda: f"{self.result_var} = {self.ex(e)};")
            elif last:
                self.line(depth, value)
            else:
                self.line(depth, lambda: f"return {value()};")
            return
        if k in ("break", "continue"):
            self.emit(depth, f"{k};")
            return
        if k == "block":
            self.stmts(s[1], depth)
            return
        raise Refuse(f"a statement {k}")

def ends_in_return(s):
    """Whether every way through `s` ends in a return (an if with an else, a
    switch with a default, every arm returning)."""
    if s[0] == "return":
        return True
    if s[0] == "switch":
        cases = s[2]
        return any(v is None for v, _ in cases) and all(body and ends_in_return(body[-1]) for _, body in cases)
    if s[0] == "if":
        _, c, then, els = s
        return bool(then) and ends_in_return(then[-1]) and els is not None and bool(els) and ends_in_return(els[-1])
    return False

def assigns(ss, name):
    for s in ss:
        if s[0] == "assign" and s[1] == ("id", name): return True
        for part in s[1:]:
            if isinstance(part, list) and assigns(part, name): return True
    return False

class Translation:
    def __init__(self, d):
        self.d = d
        self.param_names = [p["name"] for p in d["parameters"]]
        # A bool, an int or a choice the WGSL reads as an f32.
        self.param_floats = [p["param_type"] in ("float", "unlimited_float", "angle") for p in d["parameters"]]
        self.helpers = {}
        self.helper_args = {}
        # The helpers reading the context (the weight, the sum), which
        # take it, `ctx`, passed whole as the language allows.
        self.helper_ctx = set()
        # A helper's pointers to arrays it never writes through, by name:
        # passed by value as arrays (`&x` at the call is `x`).
        self.helper_valptr = {}
        # The helpers reading or setting the variation's state: called on
        # it, `this.f(..)`, their `this` the state as Rhai binds it.
        self.helper_state = set()
        # WGSL built-ins the language has not, written as helpers of the
        # twin's own (`WRITTEN`), by name.
        self.written = set()
        self.derived = derived_names(d)
        self.state = state_names(d, set(self.derived.values()))
        # A variation reading its slots by a computed index: `prepare`'s one
        # array, `slots` -- its parameters, then the built-in's derived
        # values -- so a slot is the built-in's number (Rhai's own terms,
        # no syntax of ours; rhai-plugins P7 step 3).
        every = (d["shader_2d"] or "") + (d["shader_3d"] or "")
        self.slotted = bool(re.search(r"get_param\(\s*xform_id\s*,\s*variation_id\s*,\s*(?!\s)(?!\d+u\s*\))", every)) \
            or bool(re.search(r"\b(?:out|user)\[(?!\d+u?\])", d.get("shader_init") or ""))
        # One keeping its state by a computed slot: the state one array, `st`.
        self.state_array = bool(re.search(r"[gs]et_state\(\s*xform_id\s*,\s*variation_id\s*,\s*(?!\s)(?!\d+u\s*[,)])", every + (d.get("shader_state_init") or "")))

    def state_field(self, slot, f=None):
        """The state field a `get_state` / `set_state` slot names: the
        state array's element where the state is one."""
        if self.state_array:
            if slot[0] == "num":
                return f"st[{slot[1].rstrip('ui')}]"
            i = f.ex(slot)
            return f"st[{f'to_int({i})' if f.ty(slot) == 'u32' else i}]"
        if slot[0] != "num":
            raise Refuse("state by a computed slot")
        k = int(slot[1].rstrip("ui"))
        if k not in self.state:
            raise Refuse("a state slot past `state_count`")
        return self.state[k]

    def state_init(self):
        """`fn init(p)`: the map each thread starts with -- the def's
        `wgsl_state_init` (its `set_state`s, straight through), every
        other field zero, as the built-in's slots start."""
        src = self.d["shader_state_init"]
        values = {}
        f = Fn(self, 2, [], "", True)
        if src:
            fns = functions("fn state_init_() {\n" + src + "\n}")
            f.taken = {tk[1] for tk in fns[0][3] if tk[0] == "id"}
            for s in block(P(fns[0][3])):
                if s[0] == "expr" and s[1][0] == "call" and s[1][1] == "set_state":
                    values[self.state_field(s[1][2][2])] = s[1][2][3]
                elif s[0] == "decl" and not values:
                    f.stmt(s, 1, False)
                else:
                    raise Refuse("a state init that is not straight `set_state`s")
        if self.state_array:
            parts = ", ".join(f.ex(values[f"st[{k}]"]) if f"st[{k}]" in values else "0.0" for k in range(self.d["state_count"]))
            f.emit(1, f"#{{ st: [{parts}] }}")
        else:
            fields = ", ".join(f"{n}: {f.ex(values[n]) if n in values else '0.0'}" for n in (self.state[k] for k in sorted(self.state)))
            f.emit(1, f"#{{ {fields} }}")
        return "fn init(p) {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

    def n_slots(self):
        """The parameters' slots."""
        return len(self.param_names)

    def slots_prepare(self):
        """`prepare` for a variation reading its parameters by a computed
        slot and computing nothing of its own: the parameters as `slots`."""
        f = Fn(self, 2, [], "", True)
        f.init = True
        f.emit(1, f"let out = array({self.n_slots()}, 0.0);")
        for i in range(self.n_slots()):
            f.emit(1, f"out[{i}] = {self.param_read(i)};")
        f.emit(1, "#{ slots: out }")
        return "fn prepare(p) {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

    def derived_list(self):
        return [self.derived[k] for k in range(self.d["init_param_count"])]

    def param_read(self, i):
        """Slot `i` of the variation's: a parameter, or past them a value
        `prepare` returns."""
        n = len(self.param_names)
        if i >= n and self.slotted:
            return f"p.slots[{i}]"
        if i >= n:
            if i - n not in self.derived:
                raise Refuse("a slot past the derived ones")
            return f"p.{self.derived[i - n]}"
        if not self.param_floats[i]:
            return f"to_float(p.{self.param_names[i]})"
        return f"p.{self.param_names[i]}"

    def prepare(self, src):
        """The def's `wgsl_init` as `fn prepare(p)`: its values by the names
        the body reads them by. A body ending in its slots' writes returns
        them as a map; any other keeps WGSL's `out`, an array."""
        fns = functions(src)
        k = next((i for i, fn in enumerate(fns) if fn[0].startswith("init_")), len(fns) - 1)
        helpers = self.helper_fns(fns[:k] + fns[k + 1:], 2)
        _, args, ret, toks = fns[k]
        # A local `p` would hide the parameters, `prepare`'s `p`.
        free = next(n for n in ("pv", "p_", "p0") if ("id", n) not in toks)
        toks = [("id", free) if t == ("id", "p") else t for t in toks]
        f = Fn(self, 2, [], "", True)
        f.init = True
        f.taken = {tk[1] for tk in toks if tk[0] == "id"}
        ss = fold_returns(block(P(toks)))
        if not ss or ss[-1] != ("return", ("id", "out")):
            raise Refuse("an init that does not end returning `out`")
        mentions = lambda s: "('id', 'out')" in repr(s)
        tail = len(ss) - 1
        while tail > 0 and ss[tail - 1][0] == "assign" and ss[tail - 1][1][0] == "index" and ss[tail - 1][1][1] == ("id", "out") and ss[tail - 1][2] == "=":
            tail -= 1
        head = [s for s in ss[:tail] if not (s[0] == "decl" and s[2] == "out")]
        if not any(mentions(s) for s in head) and not self.slotted:
            # Straight to the map.
            f.stmts(head, 1)
            values = {}
            for s in ss[tail:-1]:
                values[int(s[1][2][1].rstrip("ui"))] = s[3]
            parts = [f"{n}: {f.ex(values[i]) if i in values else '0.0'}" for i, n in enumerate(self.derived_list())]
            f.emit(1, f"#{{ {', '.join(parts)} }}")
        else:
            f.stmts(ss, 1, last=True)
        if f.uses_ctx:
            raise Refuse("an init reading the weight")
        # Its tables' literals at its top, as a body's (the vertices
        # polychoron's init copies).
        f.lines[0:0] = f.table_lines()
        return helpers + ["fn prepare(p) {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"]

    def helper_fns(self, fns, dim):
        """Each WGSL helper as a Rhai function."""
        out = []
        for fname, args, ret, toks in fns:
            if ret.replace(" ", "").startswith("array<"):
                self.helpers[fname] = "array:" + array_of(ret)[0]
            else:
                self.helpers[fname] = vec_of(ret.strip()) if ret.strip() else None
            if any(n == "xform_id" for n, _ in args) and ("id", "p") in toks:
                # Its own `p` would hide the parameters it takes as `p`.
                free = next(n for n in ("pv", "p_", "p0") if ("id", n) not in toks)
                toks = [("id", free) if t == ("id", "p") else t for t in toks]
                args = [(free if n == "p" else n, ty) for n, ty in args]
            # Its pointers to arrays: one never written through is an array
            # passed by value, the one written `this` (as a scalar's is).
            ss = block(P(toks))
            array_ptrs = [n for n, ty in args if ty.replace(" ", "").startswith("ptr<function,array<")]
            valptr = {n for n in array_ptrs if not writes_through(ss, n)}
            written = [n for n in array_ptrs if n not in valptr]
            if len(written) > 1 or (written and any(ty.strip().startswith("ptr") and "RngState" not in ty and n not in array_ptrs for n, ty in args)):
                raise Refuse("a helper writing through two pointers")
            ss = deref(ss, {n: n for n in valptr} | {n: "this" for n in written})
            args = [(n, ty.replace(" ", "")[len("ptr<function,"):-1]) if n in valptr else (n, ty) for n, ty in args]
            self.helper_valptr[fname] = valptr
            self.helper_args[fname] = args
            f = Fn(self, dim, args, ret, True)
            f.taken = {tk[1] for tk in toks if tk[0] == "id"}
            if (self.helpers[fname] or "").startswith("array:"):
                ss = fold_early(ss)
            f.stmts(ss, 1, last=True)
            f.lines[0:0] = f.table_lines()
            params = [a for a in f.params]
            if f.reads_state:
                if f.this_ptr:
                    raise Refuse(f"a helper with a pointer and the variation's state `{fname}`")
                self.helper_state.add(fname)
            if f.uses_ctx:
                params.append("ctx")
                self.helper_ctx.add(fname)
            out.append(f"fn {fname}({', '.join(params)}) {{\n" + "\n".join(l[1] for l in f.lines) + "\n}\n")
        return out

    def body(self, src, dim, name):
        fns = functions(src)
        if not fns: raise Refuse("no function")
        # The variation's own function, its helpers before or after it.
        k = next((i for i, fn in enumerate(fns) if fn[0] == f"variation_{name}"), len(fns) - 1)
        out = self.helper_fns(fns[:k] + fns[k + 1:], dim)
        fname, args, ret, toks = fns[k]
        f = Fn(self, dim, args, ret, False)
        used = {tk[1] for tk in toks if tk[0] == "id"}
        f.taken = set(used)
        names = (["z", "w", "q", "pt"] if dim == 2 else ["v", "q", "pt", "pv"]) + [f"pt{k}" for k in range(2, 10)]
        f.point = next(n for n in names if n not in used)
        # A 3D body's 4th coordinate given (`point_w_out`): a local, returned
        # as `w`.
        if dim == 3 and "point_w_out" in used:
            f.outs["point_w_out"] = ["w", None]
        for ptr, slot in f.outs.items():
            slot[1] = next(n for n in ({"vc": ("color", "vcolor", "col"), "hide": ("hidden", "hide_v", "hid"), "vrc": ("rgb", "rgb_v", "direct"), "point_w_out": ("w_out", "w4", "w_given")}[ptr]) if n not in used)
            f.types[slot[1]] = {"vc": "f32", "hide": "bool", "vrc": "vec3", "point_w_out": "f32"}[ptr]
            f.uses_ctx = True
        p = P(toks)
        ss = block(p)
        if not ss or not ends_in_return(ss[-1]):
            raise Refuse("the body does not end in a return")
        start = lambda: [f.emit(1, f"let {local} = {'0.0' if key == 'w' else 'ctx.' + key};") for key, local in f.outs.values()]
        start()
        f.stmts(ss, 1, last=True)
        if (dim == 3 or f.outs) and ss[-1][0] in ("if", "switch"):
            f.result_var = next(n for n in ("result", "out3", "res3") if n not in used)
            f.lines = []
            start()
            f.emit(1, f"let {f.result_var} = {'vec3(0.0, 0.0, 0.0)' if dim == 3 else 'complex(0.0, 0.0)'};")
            f.types = dict(f.types)
            f.stmts(ss, 1, last=True)
            f.emit(1, f.result_map(f.result_var) if f.outs else f.result_var)
        argz = f.point
        f.lines[0:0] = f.table_lines()
        if any(l[0] == "let" for l in f.lines):
            raise Refuse("(translator error) a held temporary outside its statement")
        sig = f"fn {'variation' if dim == 2 else 'variation3'}({argz}, p{', ctx' if f.uses_ctx else ''})"
        return out, sig + " {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

def writes_through(ss, name):
    """Whether any statement of `ss` assigns through the pointer `name`."""
    target = repr(("un", "*", ("id", name)))
    def walk(node):
        if isinstance(node, tuple) and node and node[0] == "assign" and target in repr(node[1]):
            return True
        if isinstance(node, (tuple, list)):
            return any(walk(x) for x in node)
        return False
    return walk(ss)

def deref(node, names):
    """`node` with each `*name` (and `(*name)`) the name `names` gives."""
    if isinstance(node, tuple) and len(node) == 3 and node[0] == "un" and node[1] == "*" and node[2][0] == "id" and node[2][1] in names:
        return ("id", names[node[2][1]])
    if isinstance(node, tuple) and len(node) == 2 and node[0] == "paren":
        inner = deref(node[1], names)
        return inner if inner[0] == "id" and inner != node[1] else ("paren", inner)
    if isinstance(node, tuple):
        return tuple(deref(x, names) for x in node)
    if isinstance(node, list):
        return [deref(x, names) for x in node]
    return node

def fold_early(ss):
    """Early returns (an `if` without an `else` that returns) folded: what
    follows becomes its `else`, so the body ends in one `if` value -- for
    a helper giving an array, which the language's `if` value can give
    and its early return cannot."""
    for i, s in enumerate(ss):
        if s[0] == "if" and s[3] is None and s[2] and i + 1 < len(ss) and ends_in_return(fold_early(s[2])[-1]):
            return ss[:i] + [("if", s[1], fold_early(s[2]), fold_early(ss[i + 1:]))]
    return ss

def fold_returns(ss):
    """An init's early `return out` (an `if` without an `else`) folded:
    what follows becomes its `else`, so `prepare` returns once, at its
    end, as the language has it."""
    ret = ("return", ("id", "out"))
    for i, s in enumerate(ss):
        if s[0] == "if" and s[3] is None and s[2] and fold_returns(s[2])[-1] == ret and ss[-1] == ret:
            return ss[:i] + [("if", s[1], fold_returns(s[2])[:-1], fold_returns(ss[i + 1:])[:-1]), ret]
    return ss

# Built-ins whose shader the builder rewrites for each flame with data
# the host computes (`variation_specialized_source`): their WGSL alone is
# not the variation. (synth's rewrite only drops the modes a flame does
# not use, so its twin is whole.)
HOST_BAKED = {
    "mondrianomies": "a table the host computes for each flame (the Exact fill's rectangles) baked into its shader",
}

# The flame's camera in the shader's `params`: the context's names.
CAMERA = {"camera_x": "camera_x", "camera_y": "camera_y", "camera_z": "camera_z", "camera_rotation_x": "camera_pitch",
          "camera_rotation_y": "camera_yaw", "camera_bank": "camera_bank", "perspective_strength": "perspective"}

# The flame shader's emission collector (`Feature::PlotEmits`) as the
# language's statements.
EMITS = {"emit_plot": "emit", "emit_plot_weighted": "emit", "emit_plot_offset": "emit_offset", "emit_suppress_main": "suppress_main"}

# WGSL built-ins the language has not, as Rhai helpers: exact, if slower
# than the hardware's.
WRITTEN = {
    # `countOneBits` of a u32: Kernighan's, a bit cleared a pass.
    "count_one_bits": "fn count_one_bits(x) {\n    let v = x;\n    let n = u32(0);\n    while v != u32(0) {\n        v = v & (v - u32(1));\n        n = n + u32(1);\n    }\n    n\n}\n",
}

# Rhai's reserved words: not a `PARAMS` key, nor `p.` a name.
RHAI_RESERVED = {"super", "this", "fn", "let", "const", "if", "else", "while", "loop", "for", "in", "do", "until", "break", "continue",
                 "return", "throw", "try", "catch", "import", "export", "as", "private", "global", "true", "false", "switch", "is", "Fn",
                 "call", "curry", "type_of", "print", "debug", "eval", "var", "static", "shared", "with", "goto", "exit", "match", "case",
                 "public", "protected", "new", "use", "module", "package", "spawn", "thread", "go", "sync", "async", "await", "yield",
                 "default", "void", "null", "nil"}

def state_names(d, taken):
    """Each state slot's field: the name the body reads it into (`let x0 =
    get_state(xform_id, variation_id, 0u)`), else `st<k>`; never a
    parameter's or a derived value's, never twice."""
    taken = set(taken) | {p["name"] for p in d["parameters"]} | RHAI_RESERVED
    names = {}
    for src in (d["shader_2d"] or "", d["shader_3d"] or ""):
        for g in re.finditer(r"(?:let|var) (\w+)\s*=\s*get_state\(xform_id,\s*variation_id,\s*(\d+)u\)", src):
            k = int(g.group(2))
            if k < d["state_count"] and k not in names and g.group(1) not in taken:
                names[k] = g.group(1)
                taken.add(g.group(1))
    for k in range(d["state_count"]):
        if k not in names:
            name = f"st{k}"
            while name in taken:
                name += "_"
            names[k] = name
            taken.add(name)
    return names

def derived_names(d):
    """Each derived slot's name: the one the body binds it to (`let va =
    get_param(xform_id, variation_id, 3u)`), else `d<k>`; never a
    parameter's, never twice."""
    n, m = len(d["parameters"]), d["init_param_count"]
    taken = {p["name"] for p in d["parameters"]} | RHAI_RESERVED
    names = {}
    for src in (d["shader_2d"] or "", d["shader_3d"] or ""):
        for g in re.finditer(r"let (\w+)\s*=\s*get_param\(xform_id,\s*variation_id,\s*(\d+)u\)", src):
            k = int(g.group(2)) - n
            if 0 <= k < m and k not in names and g.group(1) not in taken:
                names[k] = g.group(1)
                taken.add(g.group(1))
    for k in range(m):
        if k not in names:
            name = f"d{k}"
            while name in taken:
                name += "_"
            names[k] = name
            taken.add(name)
    return names

# WGSL's functions of a number it takes part by part of a vector.
PARTWISE = {"clamp", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp", "exp2", "log", "log2", "sqrt", "inverseSqrt",
            "fract", "sign", "floor", "ceil", "trunc", "round", "step", "smoothstep", "pow", "fma", "radians", "degrees", "mix", "atan2"}

def literal(e):
    """A number written out, or its negation: never NaN."""
    while e[0] in ("paren", "un") and (e[0] == "paren" or e[1] == "-"):
        e = e[1] if e[0] == "paren" else e[2]
    return e[0] == "num"

def passthrough_3d(two, three):
    """Whether the 3D WGSL is the 2D one with z passed through."""
    a = re.sub(r"\s+", " ", two)
    b = re.sub(r"\s+", " ", three)
    b = b.replace("vec3<f32>", "vec2<f32>").replace("p.xy", "p")
    b = re.sub(r",\s*p\.z\s*\)", ")", b)
    a = re.sub(r"//[^\n]*", "", two); b2 = re.sub(r"//[^\n]*", "", three)
    norm = lambda s: re.sub(r"\s+", "", s)
    b2 = b2.replace("vec3<f32>", "vec2<f32>").replace("p.xy", "p")
    b2 = re.sub(r",\s*p\.z\s*\)", ")", b2)
    return norm(a) == norm(b2)

# The shared helpers a body may call (`shaders/core/`): their own WGSL,
# translated as the twin's helpers, so the formulas (cdiv's guard,
# csqrt's sign test, the noise's hashes) are the built-in's exactly. Of
# complex.wgsl, the functions the language has no form of.
COMPLEX_LIB = {"cadd", "csub", "cmul", "cmul_real", "cconj", "cmag2", "csquare", "cdiv", "csqrt", "cmat2_make", "cmat2_apply", "cmat2_inverse_sl2"}
SHARED_LIBS = ["complex", "utilities", "noise", "voronoi", "polyhedra", "polychora", "su_mobius", "fractwf"]
# Of these, the functions carried in: the others the translator writes
# as the language's own (`ff_atan2` as `atan2`) or refuses.
SHARED_ONLY = {"complex": COMPLEX_LIB, "utilities": {"srgb_to_linear", "build_camera_matrix", "camera_transform"}}

def shared_lib():
    """Each shared helper: name -> its WGSL."""
    out = {}
    for lib in SHARED_LIBS:
        src = open(f"shaders/core/{lib}.wgsl", encoding="utf-8").read()
        for m in re.finditer(r"^fn (\w+)\(", src, re.M):
            if lib in SHARED_ONLY and m.group(1) not in SHARED_ONLY[lib]:
                continue
            # To its closing brace, which may share its line.
            i, depth = src.index("{", m.end()), 0
            for j in range(i, len(src)):
                depth += {"{": 1, "}": -1}.get(src[j], 0)
                if depth == 0:
                    break
            out[m.group(1)] = src[m.start():j + 1]
    return out

LIB_DECLS = None

def lib_decls():
    """The shared libraries' structs and constants."""
    global LIB_DECLS
    if LIB_DECLS is None:
        STRUCTS.clear(); CONSTS.clear()
        for lib in SHARED_LIBS:
            # Its template's lines (`{{#if}}`) dropped: only the
            # declarations are wanted here.
            src = open(f"shaders/core/{lib}.wgsl", encoding="utf-8").read()
            functions("".join(l for l in src.splitlines(True) if not l.lstrip().startswith(("#", "{{"))))
        LIB_DECLS = (dict(STRUCTS), dict(CONSTS))
    return LIB_DECLS

def with_complex_lib(src):
    """`src` with the shared helpers it calls, and theirs, before it."""
    lib = shared_lib()
    calls = lambda text: [n for n in lib if re.search(r"\b" + n + r"\(", text)]
    defined = set(re.findall(r"^\s*fn (\w+)", src, re.M))
    need, seen = [], set()
    def want(n):
        if n in seen or n in defined:
            return
        seen.add(n)
        for dep in calls(lib[n][lib[n].index("{"):]):
            want(dep)
        # A helper before its callers.
        need.append(n)
    for n in calls(src):
        want(n)
    return "".join(lib[n] + "\n" for n in need) + src

def translate(d):
    structs, consts = lib_decls()
    STRUCTS.clear(); STRUCTS.update(structs)
    CONSTS.clear(); CONSTS.update(consts)
    f = set(d["features"])
    for feat in ("analytic_blur", "volume_side_emit"):
        if feat in f: raise Refuse(f"the feature {feat}")
    if d["name"] in HOST_BAKED: raise Refuse(HOST_BAKED[d["name"]])

    for prm in d["parameters"]:
        if prm["name"] in RHAI_RESERVED: raise Refuse(f"a parameter Rhai reserves the name of, `{prm['name']}`")
        if not re.fullmatch(r"[A-Za-z_]\w*", prm["name"]): raise Refuse(f"a parameter named `{prm['name']}`, no Rhai name")
    # A pre or post variation returns the whole point, its weight in it
    # (`ctx.weight`), as the built-in does; `INFO.phase` says when it runs.
    t = Translation(d)
    # Derived parameters: the init as `prepare`; the state's start as
    # `init`; then the bodies.
    first = []
    if d["init_param_count"]:
        first += t.prepare(with_complex_lib(d["shader_init"]))
    elif t.slotted:
        first.append(t.slots_prepare())
    if d["state_count"]:
        first.append(t.state_init())
    more, two = t.body(with_complex_lib(d["shader_2d"]), 2, d["name"])
    # A helper both the init and the body call, once.
    helpers = []
    for h in first + more:
        same = [g for g in helpers if g.split("(")[0] == h.split("(")[0]]
        if same and same[0] != h:
            raise Refuse(f"two helpers named `{h.split('(')[0][3:]}`")
        if not same:
            helpers.append(h)
    three = None
    # JWildfire's z rule as `INFO.z` where the twin's default is not the
    # built-in's: a 3D body's z is always kept, a 2D body's only when the
    # flame preserves z.
    z = "never" if "never_z" in f else None
    if d["shader_3d"] and ("always_z" in f or not passthrough_3d(d["shader_2d"], d["shader_3d"])):
        if "always_z" not in f and z is None:
            z = "preserve_z"
        t3 = Translation(d)
        h3, three = t3.body(with_complex_lib(d["shader_3d"]), 3, d["name"])
        names2 = {h.split("(")[0] for h in helpers}
        for h in h3:
            if h.split("(")[0] not in names2:
                helpers.append(h)
    for t_ in (t, t3) if three else (t,):
        for w in sorted(t_.written):
            if not any(h.startswith(f"fn {w}(") for h in helpers):
                helpers.append(WRITTEN[w])
    info = {}
    if z:
        info["z"] = f'"{z}"'
    # It replaces the point it is given (`Feature::Replace`).
    if "replace" in f:
        info["replace"] = "true"
    # The most points one call emits.
    if d.get("plot_emits"):
        info["emits"] = str(d["plot_emits"])
    return helpers, two, three, info

def write(name, d):
    path = f"assets/variations/twins/{name}.rhai"
    try:
        text = open(path, encoding="utf-8").read()
    except FileNotFoundError:
        raise Refuse("no skeleton (write_variation_twin_skeletons)")
    helpers, two, three, info = translate(d)
    head = text.split("// The built-in's ")[0].rstrip("\n") + "\n\n"
    if "fn variation" in head:
        raise Refuse("already written")
    body = "".join(h + "\n" for h in helpers) + two + ("\n" + three if three else "")
    head = head.replace("the built-in variation's twin.", "the built-in variation's twin, translated from its WGSL.", 1)
    if info:
        extra = "".join(f", {k}: {v}" for k, v in info.items())
        head = re.sub(r'(const INFO = #\{ .*?) \};', lambda m: m.group(1) + extra + " };", head, count=1)
    open(path, "w", encoding="utf-8", newline="\n").write(head + body)

if __name__ == "__main__":
    args = sys.argv[1:]
    report = "--report" in args
    names = [a for a in args if not a.startswith("--")]
    V = {x["name"]: x for x in json.load(open(CORPUS, encoding="utf-8"))["variations"]}
    ok, refused = [], {}
    for n in names:
        try:
            if report:
                translate(V[n])
            else:
                write(n, V[n])
            ok.append(n)
        except Refuse as e:
            refused.setdefault(str(e), []).append(n)
        except Exception as e:
            refused.setdefault(f"(translator error) {type(e).__name__}: {e}", []).append(n)
    print(f"translated {len(ok)}: {' '.join(ok)}")
    for why, ns in sorted(refused.items(), key=lambda kv: -len(kv[1])):
        print(f"  refused {len(ns):3} {why}: {' '.join(ns[:12])}{' ...' if len(ns) > 12 else ''}")
