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
  | (?P<num>(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?[fuih]?)
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
            if self.peek() == "<" and v in ("vec2", "vec3", "vec4", "array", "mat2x2", "mat3x3", "ptr"):
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

def functions(src):
    """Each `fn name(args) -> ret { body }` in the WGSL: (name, args, ret, body tokens)."""
    toks = tokens(src)
    out, i = [], 0
    while i < len(toks):
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
                if t[1] == "," and cur.count("<") == cur.count(">"):
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

def vec_of(ty):
    if ty is None: return None
    if ty.startswith("vec2"): return "vec2"
    if ty.startswith("vec3"): return "vec3"
    if ty.startswith("vec4"): return "vec4"
    if ty in ("f32", "i32", "u32", "bool"): return ty
    if ty.startswith("array"): raise Refuse("an array")
    if ty.startswith("mat"): raise Refuse("a matrix")
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
        self.uses_ctx = False
        self.ret = vec_of(ret.strip()) if ret.strip() else None
        self.params = []
        self.point = None
        # The init's body (`prepare`): `user[i]` the parameters, `out` its
        # slots, each return the map of them.
        self.init = False
        # The pointers it writes besides the point, each a local from its
        # `ctx` field returned in the map: `vc`'s colour (WritesColor),
        # `hide`'s flag (CanHide). Pointer -> [key, local].
        self.outs = {}
        # A 3D body ending in an `if`: the arms set this, returned after.
        self.result_var = None
        for name, ty in args:
            ty = ty.strip()
            if name == "p" and not helper:
                self.types["p"] = "vec2" if dim == 2 else "vec3"
            elif name in ("xform_id", "variation_id"):
                pass
            elif ty.startswith("ptr<function,RngState>") or ty.startswith("ptr<function, RngState>"):
                pass
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
            elif ty.startswith("ptr"):
                raise Refuse(f"a pointer argument `{name}: {ty}`")
            else:
                self.types[name] = vec_of(ty)
                self.params.append(name)

    def ty(self, e):
        k = e[0]
        if k == "num":
            return "f32" if ("." in e[1] or "e" in e[1].lower().rstrip("fhiu")) and not e[1].endswith(("u", "i")) else ("f32" if e[1].endswith("f") else "int")
        if k == "id":
            if e[1] in self.types: return self.types[e[1]]
            if e[1] in ("true", "false"): return "bool"
            return "f32"
        if k == "paren": return self.ty(e[1])
        if k == "un":
            return "bool" if e[1] == "!" else self.ty(e[2])
        if k == "bin":
            if e[1] in ("==", "!=", "<", ">", "<=", ">=", "&&", "||"):
                return "bool"
            a, b = self.ty(e[2]), self.ty(e[3])
            for v in ("vec3", "vec2", "vec4"):
                if v in (a, b): return v
            if "f32" in (a, b): return "f32"
            return a
        if k == "field":
            base = self.ty(e[1])
            if base in ("vec2", "vec3", "vec4"):
                n = len(e[2])
                return {1: "f32", 2: "vec2", 3: "vec3", 4: "vec4"}[n]
            return "f32"
        if k == "index":
            return "f32"
        if k == "call":
            f, args = e[1], e[2]
            if f.startswith("vec2"): return "vec2"
            if f.startswith("vec3"): return "vec3"
            if f.startswith("vec4"): return "vec4"
            if f in ("length", "dot", "distance", "get_param", "rng_nextf", "f32", "atan2"): return "f32"
            if f in ("i32", "u32"): return f
            if f in ("select",): return self.ty(args[1])
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

    def hold(self, e):
        """An operand used more than once, made a name."""
        if e[0] in ("id", "num"):
            return self.ex(e)
        self.temps += 1
        n = f"t{self.temps}"
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
            if n == "accum":
                self.uses_ctx = True; return "ctx.accum"
            return n
        if k == "paren":
            return f"({self.ex(e[1])})"
        if k == "un":
            if e[1] == "*" and e[2][0] == "id" and e[2][1] in self.outs:
                return self.outs[e[2][1]][1]
            if e[1] == "*": raise Refuse("a pointer read")
            if e[1] == "&": raise Refuse("a pointer taken")
            if e[1] == "~": raise Refuse("a bitwise not")
            inner = self.ex(e[2])
            return f"{e[1]}{inner}"
        if k == "bin":
            op = e[1]
            if op in ("<<", ">>", "&", "|", "^"):
                raise Refuse("bit operations")
            ta, tb = self.ty(e[2]), self.ty(e[3])
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
                parts = ("re", "im") if vec[0] == "vec2" else ("x", "y", "z") if vec[0] == "vec3" else None
                if parts is None:
                    raise Refuse("a vec4 and a number")
                each = [f"{a + '.' + c if ta == vec[0] else a} {op} {b + '.' + c if tb == vec[0] else b}" for c in parts]
                return f"complex({each[0]}, {each[1]})" if vec[0] == "vec2" else f"vec3({', '.join(each)})"
            return f"{self.ex(e[2])} {op} {self.ex(e[3])}"
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
        if k == "index" and self.init and e[1] in (("id", "user"), ("id", "out")):
            if e[2][0] != "num":
                raise Refuse("the init's slots by a computed index")
            i = int(e[2][1].rstrip("ui"))
            return self.tr.param_read(i) if e[1] == ("id", "user") else f"out[{i}]"
        if k == "index":
            # The variation's own weight, JWildfire's inside the formula.
            if e[1] == ("field", ("index", ("id", "transforms"), ("id", "xform_id")), "variations") and e[2] == ("id", "variation_id"):
                self.uses_ctx = True
                return "ctx.weight"
            raise Refuse("an index")
        if k == "call":
            return self.call(e)
        raise Refuse(f"an expression {k}")

    def call(self, e):
        f, args = e[1], e[2]
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
            raise Refuse("a vec4")
        if f == "get_param":
            slot = args[2]
            if slot[0] != "num":
                raise Refuse("a parameter by a computed slot")
            return self.tr.param_read(int(slot[1].rstrip("u")))
        if f == "rng_nextf":
            return "rand()"
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
        if f == "radians" and self.ty(args[0]) == "f32":
            return f"({self.ex(args[0])} * 0.017453292519943295)"
        if f == "degrees" and self.ty(args[0]) == "f32":
            return f"({self.ex(args[0])} * 57.29577951308232)"
        if f == "get_state":
            return f"this.{self.tr.state_field(args[2])}"
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
        if f in ("u32", "bitcast", "bool"):
            raise Refuse(f"`{f}`")
        t0 = self.ty(args[0]) if args else None
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
                self.temps += 1
                t = f"t{self.temps}"
                self.lines.append(("let", t, f"clamp(({x} - {e0}) / ({e1} - {e0}), 0.0, 1.0)"))
                self.types[t] = "f32"
                out.append(f"{t} * {t} * (3.0 - 2.0 * {t})")
            return f"({out[0]})" if len(out) == 1 else f"complex({out[0]}, {out[1]})"
        if f in MATH:
            if f in ("smoothstep", "exp2"):
                raise Refuse(f"`{f}`")
            # WGSL's log is the natural one; Rhai's `log` is base 10.
            name = "ln" if f == "log" else f
            # Rhai's `max(a, b)` is `t = a; if b > t { t = b }`, keeping a
            # NaN `a`; WGSL's gives the other operand, so `max(x, 0.0)` is
            # a guard. A literal is never NaN: first, it is WGSL's exactly.
            if f in ("min", "max") and len(args) == 2 and t0 == "f32" and literal(args[1]) and not literal(args[0]):
                args = [args[1], args[0]]
            return f"{name}({', '.join(self.ex(a) for a in args)})"
        if f in ("normalize", "cross"):
            return f"{f}({', '.join(self.ex(a) for a in args)})"
        if f in self.tr.helpers:
            keep = [self.ex(a) for a, (n, ty) in zip(args, self.tr.helper_args[f]) if not ("RngState" in ty or n in ("xform_id", "variation_id"))]
            if any(n in ("xform_id", "variation_id") for n, _ in self.tr.helper_args[f]):
                raise Refuse(f"a helper taking the ids `{f}`")
            return f"{f}({', '.join(keep)})"
        raise Refuse(f"a call to `{f}`")

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
        if self.init and k == "decl" and s[2] == "out":
            self.emit(depth, f"let out = array({self.tr.d['init_param_count']}, 0.0);")
            return
        if self.init and k == "assign" and s[1][0] == "index" and s[1][1] == ("id", "out"):
            self.line(depth, lambda: f"{self.ex(s[1])} {s[2]} {self.ex(s[3])};")
            return
        if self.init and k == "return" and s[1] == ("id", "out"):
            slots = ", ".join(f"{n}: out[{i}]" for i, n in enumerate(self.tr.derived_list()))
            self.emit(depth, f"#{{ {slots} }}" if last else f"return #{{ {slots} }};")
            return
        if k == "decl":
            _, kind, name, ty, init = s
            t = vec_of(ty.strip()) if ty else (self.ty(init) if init else None)
            if t in ("i32",) and init is not None and self.ty(init) == "int":
                t = "int"
            self.types[name] = "f32" if t == "int" and ty is None and init is not None and self.ty(init) == "f32" else t
            if init is None:
                zero = {"vec2": "complex(0.0, 0.0)", "vec3": "vec3(0.0, 0.0, 0.0)", "f32": "0.0", "i32": "0", "u32": "0", "bool": "false"}.get(t)
                if zero is None: raise Refuse(f"a `var` of `{ty}`")
                self.emit(depth, f"let {name} = {zero};")
            else:
                self.line(depth, lambda: f"let {name} = {self.ex(init)};")
            return
        if k == "assign":
            _, lhs, op, rhs = s
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
            if lhs[0] == "un" and lhs[1] == "*" and lhs[2][0] == "id" and lhs[2][1] in self.outs:
                self.line(depth, lambda: f"{self.outs[lhs[2][1]][1]} {op} {self.ex(rhs)};")
                return
            if lhs[0] == "un" and lhs[1] == "*":
                raise Refuse("a write through a pointer")
            if lhs[0] != "id":
                if lhs[0] == "field" and self.ty(lhs[1]) == "vec3":
                    n = self.ex(lhs[1])
                    self.line(depth, lambda: f"{n}.{lhs[2]} {op} {self.ex(rhs)};")
                    return
                raise Refuse("an assignment to a part")
            if op in ("*=", "/=") and self.ty(lhs) == "vec2" and self.ty(rhs) == "vec2":
                raise Refuse("a componentwise compound assignment")
            self.line(depth, lambda: f"{self.ex(lhs)} {op} {self.ex(rhs)};")
            return
        if k == "expr" and s[1][0] == "call" and s[1][1] == "set_state":
            field = self.tr.state_field(s[1][2][2])
            self.line(depth, lambda: f"this.{field} = {self.ex(s[1][2][3])};")
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
            step_one = init and init[0] == "decl" and upd in (
                ("assign", ("id", init[2]), "+=", ("num", "1")),
                ("assign", ("id", init[2]), "=", ("bin", "+", ("id", init[2]), ("num", "1"))),
            )
            # Rhai's range is taken once; WGSL tests its bound every pass.
            bound_fixed = cond and cond[0] == "bin" and not any(assigns(body, n) for n in re.findall(r"'id', '(\w+)'", repr(cond[3])))
            if step_one and init[4] is not None and self.ty(init[4]) != "f32" and cond and cond[0] == "bin" and cond[1] in ("<", "<=") and cond[2] == ("id", init[2]) \
                    and not assigns(body, init[2]) and bound_fixed:
                name = init[2]
                self.types[name] = "int"
                rng = ".." if cond[1] == "<" else "..="
                self.line(depth, lambda: f"for {name} in {self.ex(init[4])}{rng}{self.ex(cond[3])} {{")
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
            raise Refuse("a `loop`")
        if k == "switch":
            _, e, cases = s
            self.temps += 1
            v = f"t{self.temps}"
            self.line(depth, lambda: f"let {v} = {self.ex(e)};")
            self.types[v] = self.ty(e)
            first = True
            dflt = None
            for vals, body in cases:
                if vals is None:
                    dflt = body; continue
                cond = " || ".join(f"{v} == {self.ex(x)}" for x in vals)
                self.emit(depth, ("if " if first else "} else if ") + cond + " {")
                self.stmts(body, depth + 1)
                first = False
            if dflt is not None:
                if first:
                    self.stmts(dflt, depth)
                else:
                    self.emit(depth, "} else {")
                    self.stmts(dflt, depth + 1)
            if not first:
                self.emit(depth, "}")
            return
        if k == "return":
            e = s[1]
            if e is None:
                raise Refuse("a bare return")
            value = (lambda: self.result_map(self.ex(e))) if self.outs else (lambda: self.ex(e))
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
    """Whether every way through `s` ends in a return (an if with an else)."""
    if s[0] == "return":
        return True
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
        self.derived = derived_names(d)
        self.state = state_names(d, set(self.derived.values()))

    def state_field(self, slot):
        """The state field a `get_state` / `set_state` slot names."""
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
            if "rng" in src:
                raise Refuse("a state init that draws")
            fns = functions("fn state_init_() {\n" + src + "\n}")
            for s in block(P(fns[0][3])):
                if s[0] == "expr" and s[1][0] == "call" and s[1][1] == "set_state":
                    values[self.state_field(s[1][2][2])] = s[1][2][3]
                elif s[0] == "decl" and not values:
                    f.stmt(s, 1, False)
                else:
                    raise Refuse("a state init that is not straight `set_state`s")
        fields = ", ".join(f"{n}: {f.ex(values[n]) if n in values else '0.0'}" for n in (self.state[k] for k in sorted(self.state)))
        f.emit(1, f"#{{ {fields} }}")
        return "fn init(p) {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

    def derived_list(self):
        return [self.derived[k] for k in range(self.d["init_param_count"])]

    def param_read(self, i):
        """Slot `i` of the variation's: a parameter, or past them a value
        `prepare` returns."""
        n = len(self.param_names)
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
        if len(fns) != 1:
            raise Refuse("a helper in the init")
        _, args, ret, toks = fns[0]
        # A local `p` would hide the parameters, `prepare`'s `p`.
        free = next(n for n in ("pv", "p_", "p0") if ("id", n) not in toks)
        toks = [("id", free) if t == ("id", "p") else t for t in toks]
        f = Fn(self, 2, [], "", True)
        f.init = True
        ss = fold_returns(block(P(toks)))
        if not ss or ss[-1] != ("return", ("id", "out")):
            raise Refuse("an init that does not end returning `out`")
        mentions = lambda s: "('id', 'out')" in repr(s)
        tail = len(ss) - 1
        while tail > 0 and ss[tail - 1][0] == "assign" and ss[tail - 1][1][0] == "index" and ss[tail - 1][1][1] == ("id", "out") and ss[tail - 1][2] == "=":
            tail -= 1
        head = [s for s in ss[:tail] if not (s[0] == "decl" and s[2] == "out")]
        if not any(mentions(s) for s in head):
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
        return "fn prepare(p) {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

    def body(self, src, dim, name):
        fns = functions(src)
        if not fns: raise Refuse("no function")
        main = fns[-1]
        out = []
        for fname, args, ret, toks in fns[:-1]:
            self.helpers[fname] = vec_of(ret.strip()) if ret.strip() else None
            self.helper_args[fname] = args
            f = Fn(self, dim, args, ret, True)
            p = P(toks)
            ss = block(p)
            f.stmts(ss, 1, last=True)
            if f.uses_ctx: raise Refuse(f"a helper reading the weight or the sum `{fname}`")
            params = [a for a in f.params]
            out.append(f"fn {fname}({', '.join(params)}) {{\n" + "\n".join(l[1] for l in f.lines) + "\n}\n")
        fname, args, ret, toks = main
        f = Fn(self, dim, args, ret, False)
        used = {tk[1] for tk in toks if tk[0] == "id"}
        f.point = next(n for n in (["z", "w", "q", "pt"] if dim == 2 else ["v", "q", "pt", "pv"]) if n not in used)
        for ptr, slot in f.outs.items():
            slot[1] = next(n for n in ({"vc": ("color", "vcolor", "col"), "hide": ("hidden", "hide_v", "hid")}[ptr]) if n not in used)
            f.uses_ctx = True
        p = P(toks)
        ss = block(p)
        if not ss or not ends_in_return(ss[-1]):
            raise Refuse("the body does not end in a return")
        start = lambda: [f.emit(1, f"let {local} = ctx.{key};") for key, local in f.outs.values()]
        start()
        f.stmts(ss, 1, last=True)
        if (dim == 3 or f.outs) and ss[-1][0] == "if":
            f.result_var = next(n for n in ("result", "out3", "res3") if n not in used)
            f.lines = []
            start()
            f.emit(1, f"let {f.result_var} = {'vec3(0.0, 0.0, 0.0)' if dim == 3 else 'complex(0.0, 0.0)'};")
            f.types = dict(f.types)
            f.stmts(ss, 1, last=True)
            f.emit(1, f.result_map(f.result_var) if f.outs else f.result_var)
        argz = f.point
        if any(l[0] == "let" for l in f.lines):
            raise Refuse("(translator error) a held temporary outside its statement")
        sig = f"fn {'variation' if dim == 2 else 'variation3'}({argz}, p{', ctx' if f.uses_ctx else ''})"
        return out, sig + " {\n" + "\n".join(l[1] for l in f.lines) + "\n}\n"

def fold_returns(ss):
    """An init's early `return out` (an `if` without an `else`) folded:
    what follows becomes its `else`, so `prepare` returns once, at its
    end, as the language has it."""
    ret = ("return", ("id", "out"))
    for i, s in enumerate(ss):
        if s[0] == "if" and s[3] is None and s[2] and fold_returns(s[2])[-1] == ret and ss[-1] == ret:
            return ss[:i] + [("if", s[1], fold_returns(s[2])[:-1], fold_returns(ss[i + 1:])[:-1]), ret]
    return ss

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

def translate(d):
    f = set(d["features"])
    for feat in ("writes_rgb", "needs_w", "needs_mobius_lib", "analytic_blur", "volume_side_emit"):
        if feat in f: raise Refuse(f"the feature {feat}")
    if d.get("plot_emits"): raise Refuse("plot emits")
    for prm in d["parameters"]:
        if prm["name"] in RHAI_RESERVED: raise Refuse(f"a parameter Rhai reserves the name of, `{prm['name']}`")
    # A pre or post variation returns the whole point, its weight in it
    # (`ctx.weight`), as the built-in does; `INFO.phase` says when it runs.
    t = Translation(d)
    # Derived parameters: the init as `prepare`; the state's start as
    # `init`; then the bodies.
    first = []
    if d["init_param_count"]:
        first.append(t.prepare(d["shader_init"]))
    if d["state_count"]:
        first.append(t.state_init())
    more, two = t.body(d["shader_2d"], 2, d["name"])
    helpers = first + more
    three = None
    # JWildfire's z rule as `INFO.z` where the twin's default is not the
    # built-in's: a 3D body's z is always kept, a 2D body's only when the
    # flame preserves z.
    z = "never" if "never_z" in f else None
    if d["shader_3d"] and ("always_z" in f or not passthrough_3d(d["shader_2d"], d["shader_3d"])):
        if "always_z" not in f and z is None:
            z = "preserve_z"
        t3 = Translation(d)
        h3, three = t3.body(d["shader_3d"], 3, d["name"])
        names2 = {h.split("(")[0] for h in helpers}
        for h in h3:
            if h.split("(")[0] not in names2:
                helpers.append(h)
    info = {}
    if z:
        info["z"] = f'"{z}"'
    # It replaces the point it is given (`Feature::Replace`).
    if "replace" in f:
        info["replace"] = "true"
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
