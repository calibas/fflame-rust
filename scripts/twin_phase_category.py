"""Give every variation twin its built-in's category and phase in INFO
(rhai-plugins P7 step 3): `category` where the pickers group it, `phase`
when it is not normal -- `any` for the 295 a user may move to pre or post.
Reads the corpus (`cargo run --release --bin export_variations_json`).
A twin that has a category already is left alone.

  python scripts/twin_phase_category.py
"""
import json, os, re

V = {x["name"]: x for x in json.load(open("output/variations-corpus.json", encoding="utf-8"))["variations"]}
DIR = "assets/variations/twins"
done = 0
for f in sorted(os.listdir(DIR)):
    if not f.endswith(".rhai"):
        continue
    name = f[:-5]
    path = os.path.join(DIR, f)
    text = open(path, encoding="utf-8").read()
    m = re.search(r'const INFO = #\{ (.*?) \};', text)
    if not m or "category:" in m.group(1):
        continue
    d = V[name]
    extra = f', category: "{d["category"]}"'
    if d["phase"] != "normal":
        extra += f', phase: "{d["phase"]}"'
    text = text[:m.start()] + "const INFO = #{ " + m.group(1) + extra + " };" + text[m.end():]
    open(path, "w", encoding="utf-8", newline="\n").write(text)
    done += 1
print(f"{done} twins given their category and phase")
