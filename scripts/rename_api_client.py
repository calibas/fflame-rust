#!/usr/bin/env python3
"""One-shot: the client's API calls follow ffapi's flames -> fractals rename.

docs/projects/rename.md, step 2. The API's routes, resource segments and
wire fields; the resolution of a curated name (`resource` "fractal" ->
"fractals"); the hints that read a pasted link (the browser's old
`?flame=` links still open, as fractals). `AnimationListItem` is also
stored locally, so it reads the old key too.
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent

GENERIC = [
    (re.compile(r"/api/v1/flames\b"), "/api/v1/fractals"),
    (re.compile(r"/api/v1/users/me/flames\b"), "/api/v1/users/me/fractals"),
]

EXACT = {
    "src/api/mod.rs": [
        ('self.browse(scope, "flames", words).await', 'self.browse(scope, "fractals", words).await'),
        ('assert_eq!(Scope::Public.path("flames"), "/api/v1/fractals");', 'assert_eq!(Scope::Public.path("fractals"), "/api/v1/fractals");'),
    ],
    "src/api/integration_tests.rs": [
        ('author.patch_item("flames", &id,', 'author.patch_item("fractals", &id,'),
        ('guest.exists("flames", &flame_id)', 'guest.exists("fractals", &flame_id)'),
    ],
    "src/inventory/online.rs": [
        ('("flame", "flames"), ("fractal", "flames")', '("flame", "fractals"), ("fractal", "fractals")'),
        ('parse_open(&format!("https://fractalsforall.com/app/?flame={id}")), at(Some("flames"))',
         'parse_open(&format!("https://fractalsforall.com/app/?flame={id}")), at(Some("fractals"))'),
    ],
    "src/ui/inventory_online.rs": [
        ('None => ["flames", "animations", "palettes", "scripts"]', 'None => ["fractals", "animations", "palettes", "scripts"]'),
        ('"flames" => api.load_flame_with_visibility(&id)', '"fractals" => api.load_flame_with_visibility(&id)'),
        ('let resource = if matches!(item, Item::Flame(_)) { "flames" } else { "palettes" };',
         'let resource = if matches!(item, Item::Flame(_)) { "fractals" } else { "palettes" };'),
    ],
}


def rename_fields(s):
    """`flame_id` / `flame` on the animation wire structs."""
    out, struct = [], None
    for line in s.split("\n"):
        m = re.match(r"pub struct (\w+)", line)
        if m:
            struct = m.group(1)
        if struct in ("CreateAnimationRequest", "AnimationResponse", "AnimationListItem"):
            fm = re.match(r"(\s*)pub flame_id: ", line)
            if fm:
                alias = ', alias = "flame_id"' if struct == "AnimationListItem" else ""
                out.append(f'{fm.group(1)}#[serde(rename = "fractal_id"{alias})]')
            fm = re.match(r"(\s*)pub flame: ", line)
            if fm and struct == "AnimationResponse":
                out.append(f'{fm.group(1)}#[serde(rename = "fractal")]')
        out.append(line)
    return "\n".join(out)


def edit(rel, fn):
    p = ROOT / rel
    raw = p.read_bytes().decode("utf-8")
    crlf = "\r\n" in raw
    s = fn(raw.replace("\r\n", "\n"))
    if crlf:
        s = s.replace("\n", "\r\n")
    if s != raw:
        p.write_bytes(s.encode("utf-8"))
        print("ok", rel)


for p in sorted((ROOT / "src").rglob("*.rs")):
    rel = p.relative_to(ROOT).as_posix()

    def fn(s, rel=rel):
        for pat, rep in GENERIC:
            s = pat.sub(rep, s)
        for a, b in EXACT.get(rel, []):
            assert s.count(a) == 1, (rel, a, s.count(a))
            s = s.replace(a, b)
        if rel == "src/api/types.rs":
            s = rename_fields(s)
        return s

    edit(rel, fn)
