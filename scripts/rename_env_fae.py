#!/usr/bin/env python3
"""One-shot: the FFLAME_* environment variables become FAE_* (the old name
still read, for one release; src/env_vars.rs). docs/projects/rename.md,
step 3.

Text everywhere says FAE_; the Rust reads go through `env_vars::var` /
`var_os`, which fall back to FFLAME_; build.rs reads both itself. Left
alone: the archive, the rename plan, src/env_vars.rs (it names the old
spelling on purpose), this script.
"""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
SKIP = {"docs/projects/rename.md", "scripts/rename_env_fae.py", "src/env_vars.rs", "build.rs"}

files = subprocess.run(["git", "grep", "-l", "FFLAME_"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
for rel in files:
    if rel in SKIP or rel.startswith("docs/archive/"):
        continue
    p = ROOT / rel
    s = p.read_bytes().decode("utf-8")
    n = s.replace("FFLAME_", "FAE_")
    if rel.endswith(".rs"):
        n = re.sub(r'std::env::var\("FAE_(\w+)"\)\s*\.ok\(\)', r'crate::env_vars::var("\1")', n)
        n = re.sub(r'std::env::var\("FAE_(\w+)"\)\.is_ok_and\(', r'crate::env_vars::var("\1").is_some_and(', n)
        n = re.sub(r'std::env::var_os\("FAE_(\w+)"\)', r'crate::env_vars::var_os("\1")', n)
    if n != s:
        p.write_bytes(n.encode("utf-8"))
        print("ok", rel)

# build.rs: both spellings, the new one first; it emits FAE_ for env!().
p = ROOT / "build.rs"
s = p.read_bytes().decode("utf-8")
nl = "\r\n" if "\r\n" in s else "\n"
s = s.replace("\r\n", "\n")
edits = [
    ("    // local API. `FFLAME_API_BASE_URL` set at build time overrides both.\n"
     "    println!(\"cargo:rerun-if-env-changed=FFLAME_API_BASE_URL\");\n"
     "    let api_base_url = env::var(\"FFLAME_API_BASE_URL\")",
     "    // local API. `FAE_API_BASE_URL` set at build time overrides both (its\n"
     "    // old name, `FFLAME_API_BASE_URL`, is still read: src/env_vars.rs).\n"
     "    println!(\"cargo:rerun-if-env-changed=FAE_API_BASE_URL\");\n"
     "    println!(\"cargo:rerun-if-env-changed=FFLAME_API_BASE_URL\");\n"
     "    let api_override = env::var(\"FAE_API_BASE_URL\").or_else(|_| env::var(\"FFLAME_API_BASE_URL\"));\n"
     "    let api_base_url = api_override.clone()"),
    ("    println!(\"cargo:rustc-env=FFLAME_API_BASE_URL={api_base_url}\");\n"
     "    let chosen_by = if env::var(\"FFLAME_API_BASE_URL\").is_ok_and(|u| !u.trim().is_empty()) { \"override\" } else { \"profile\" };\n"
     "    println!(\"cargo:rustc-env=FFLAME_API_BASE_URL_CHOSEN_BY={chosen_by}\");",
     "    println!(\"cargo:rustc-env=FAE_API_BASE_URL={api_base_url}\");\n"
     "    let chosen_by = if api_override.is_ok_and(|u| !u.trim().is_empty()) { \"override\" } else { \"profile\" };\n"
     "    println!(\"cargo:rustc-env=FAE_API_BASE_URL_CHOSEN_BY={chosen_by}\");"),
    ("    println!(\"cargo:rerun-if-env-changed=FFLAME_SKIP_GIT_PROVENANCE\");\n"
     "    if env::var_os(\"FFLAME_SKIP_GIT_PROVENANCE\").is_some() {",
     "    println!(\"cargo:rerun-if-env-changed=FAE_SKIP_GIT_PROVENANCE\");\n"
     "    println!(\"cargo:rerun-if-env-changed=FFLAME_SKIP_GIT_PROVENANCE\");\n"
     "    if env::var_os(\"FAE_SKIP_GIT_PROVENANCE\").or_else(|| env::var_os(\"FFLAME_SKIP_GIT_PROVENANCE\")).is_some() {"),
]
for a, b in edits:
    assert s.count(a) == 1, a[:60]
    s = s.replace(a, b)
p.write_bytes(s.replace("\n", nl).encode("utf-8"))
print("ok build.rs")
