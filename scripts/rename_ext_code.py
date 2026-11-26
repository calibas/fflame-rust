#!/usr/bin/env python3
"""One-shot: the app writes `.fae` and reads `.fae` and `.fflame`.

docs/projects/rename.md, step 4. Every extension check goes through
`config::is_config_path` (both spellings, any case); the save dialogs, the
web download, the browser's file picker, the CLI's script output, the
texture recipes and user textures write `.fae`; the open dialogs and
pickers accept both. Diagnostics that read the local deep-zoom corpus
(`output/flame-zoom/*.fflame`) keep their file names: those files exist
under them, and the app reads them as they are.
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent


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


def exact(pairs):
    def fn(s):
        for a, b in pairs:
            assert s.count(a) == 1, (a[:70], s.count(a))
            s = s.replace(a, b)
        return s
    return fn


# --- The helper.
HELPER = '''
/// The config file's extension: `.fae`, the Fractal Art Editor's own.
pub const CONFIG_EXT: &str = "fae";
/// What it was until 2026-11-26 (docs/projects/rename.md). Read forever:
/// every config saved, shipped or shared before then has it.
pub const LEGACY_CONFIG_EXT: &str = "fflame";

/// Whether `path` names a config file by its extension: `.fae` or the old
/// `.fflame`, in any case. Not `.flame`: that is Apophysis XML.
pub fn is_config_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(CONFIG_EXT) || e.eq_ignore_ascii_case(LEGACY_CONFIG_EXT))
}
'''
edit("src/config/mod.rs", lambda s: s if "fn is_config_path" in s else s.rstrip("\n") + "\n" + HELPER)

# --- Every extension check, through the helper.
CHECKS = [
    (r'([\w.()]+?)\.extension\(\)\.and_then\(\|(\w+)\| \2\.to_str\(\)\) == Some\("fflame"\)', r"PFX::config::is_config_path(&\1)"),
    (r'([\w.()]+?)\.extension\(\)\.and_then\(\|(\w+)\| \2\.to_str\(\)\) != Some\("fflame"\)', r"!PFX::config::is_config_path(&\1)"),
    (r'([\w.()]+?)\.extension\(\)\.is_some_and\(\|(\w+)\| \2 == "fflame"\)', r"PFX::config::is_config_path(&\1)"),
    (r'([\w.()]+?)\.extension\(\)\.map\(\|(\w+)\| \2 == "fflame"\)\.unwrap_or\(false\)', r"PFX::config::is_config_path(&\1)"),
    (r'Some\("fflame"\) =>', r'Some("fae" | "fflame") =>'),
]
for p in sorted(list((ROOT / "src").rglob("*.rs")) + list((ROOT / "tests").glob("*.rs"))):
    rel = p.relative_to(ROOT).as_posix()
    prefix = "crate" if rel.startswith("src/") else "fae"

    def fn(s, prefix=prefix):
        for pat, rep in CHECKS:
            s = re.sub(pat, rep.replace("PFX", prefix), s)
        return s
    edit(rel, fn)

# --- Writing .fae; dialogs and pickers accept both.
edit("src/app/ui_handlers.rs", exact([
    ('.add_filter("Fractal Flame Config", &["fflame"])\n                    .set_file_name("fractal.fflame")',
     '.add_filter("Fractal Art Editor config", &["fae"])\n                    .set_file_name("fractal.fae")'),
    ('let filename = format!("{}.fflame", config.flame.name.to_lowercase().replace(\' \', "_"));',
     'let filename = format!("{}.{}", config.flame.name.to_lowercase().replace(\' \', "_"), crate::config::CONFIG_EXT);'),
    ('                    .add_filter("Fractal Flame Config", &["fflame"])\n                    .pick_file()\n                {\n                    match FractalConfig::load_from_file(&path) {',
     '                    .add_filter("Fractal Art Editor config", &["fae", "fflame"])\n                    .pick_file()\n                {\n                    match FractalConfig::load_from_file(&path) {'),
    ('log::error!("Failed to load .fflame file: {}", e);', 'log::error!("Failed to load the config file: {}", e);'),
    ('".fflame", ctx, "pending_subflame_load_raw",', '".fae,.fflame", ctx, "pending_subflame_load_raw",'),
    ('log::error!("Failed to parse .fflame JSON: {}", e);', 'log::error!("Failed to parse the config JSON: {}", e);'),
    ('// native JSON `.fflame` and Apophysis/JWildfire XML', '// native JSON `.fae` (or the old `.fflame`) and Apophysis/JWildfire XML'),
    ('.add_filter("Fractal Flame", &["fflame", "flame"])\n                    .add_filter("Fractal Flame Config (.fflame)", &["fflame"])',
     '.add_filter("Fractals", &["fae", "fflame", "flame"])\n                    .add_filter("Fractal Art Editor config (.fae, .fflame)", &["fae", "fflame"])'),
    ('super::trigger_browser_file_picker(".fflame,.flame", ctx, "pending_config_load_raw");',
     'super::trigger_browser_file_picker(".fae,.fflame,.flame", ctx, "pending_config_load_raw");'),
    ('                    .add_filter("Fractal Flame Config", &["fflame"])\n                    .pick_file()\n                {\n                    self.egui_layer.load_file_into_fractal_browser(path);',
     '                    .add_filter("Fractal Art Editor config", &["fae", "fflame"])\n                    .pick_file()\n                {\n                    self.egui_layer.load_file_into_fractal_browser(path);'),
    ('super::trigger_browser_file_picker(".fflame", ctx, "pending_file_browser_json_raw");',
     'super::trigger_browser_file_picker(".fae,.fflame", ctx, "pending_file_browser_json_raw");'),
    ('/// Open a config file\'s text, JSON `.fflame` or XML `.flame`:', '/// Open a config file\'s text, JSON `.fae` (or `.fflame`) or XML `.flame`:'),
]))

edit("src/script/cli.rs", exact([('.with_extension("fflame")', '.with_extension(crate::config::CONFIG_EXT)')]))
edit("src/lib.rs", exact([
    ("    // Find all .fflame and .flame files\n", "    // Find all config files (.fae, .fflame) and .flame files\n"),
    ('eprintln!("No .fflame files found");', 'eprintln!("No config files (.fae, .fflame, .flame) found");'),
]))
edit("src/escape/compile_cost.rs", lambda s: s.replace('format!("tests/visual/configs/{f}.fflame")', 'format!("tests/visual/configs/{f}.fae")'))
edit("src/escape/ifs.rs", lambda s: s.replace('format!("{slug}.fflame")', 'format!("{slug}.fae")')
     .replace('format!("julia-{slug}.fflame")', 'format!("julia-{slug}.fae")')
     .replace('format!("{name}-{tag}.fflame")', 'format!("{name}-{tag}.fae")'))
edit("src/ui/inventory_preview.rs", lambda s: s.replace('format!("{k}-{name}-{i}.fflame")', 'format!("{k}-{name}-{i}.fae")'))
edit("src/ui/subflames.rs", exact([('loaded from a .fflame file"', 'loaded from a config file (.fae)"')]))
edit("src/flame_xml.rs", exact([("(only a .fflame carries it)", "(only a .fae carries it)")]))
edit("src/bin/variation_probe.rs", lambda s: s.replace("census <config.fflame>", "census <config.fae>"))

# --- Textures: recipes and user textures are .fae; old user files still load.
edit("src/textures/mod.rs", exact([
    ('const EXT: &str = "fflame";', 'const EXT: &str = crate::config::CONFIG_EXT;'),
    ('                let name = file.strip_suffix(&format!(".{EXT}"))?.to_string();\n'
     '                let json = crate::storage::backend::read_file(&user_path(&name)).ok()?;',
     '                let name = file\n'
     '                    .strip_suffix(&format!(".{EXT}"))\n'
     '                    .or_else(|| file.strip_suffix(&format!(".{}", crate::config::LEGACY_CONFIG_EXT)))?\n'
     '                    .to_string();\n'
     '                let json = crate::storage::backend::read_file(&stored_path(&name)).ok()?;'),
    ('fn user_path(name: &str) -> std::path::PathBuf {\n'
     '    std::path::Path::new(USER_DIR).join(format!("{name}.{EXT}"))\n'
     '}\n',
     'fn user_path(name: &str) -> std::path::PathBuf {\n'
     '    std::path::Path::new(USER_DIR).join(format!("{name}.{EXT}"))\n'
     '}\n\n'
     '/// Where a user texture is: `.fae`, or a `.fflame` saved before the\n'
     '/// rename (docs/projects/rename.md), which is read and renamed as is.\n'
     'fn stored_path(name: &str) -> std::path::PathBuf {\n'
     '    let legacy = std::path::Path::new(USER_DIR).join(format!("{name}.{}", crate::config::LEGACY_CONFIG_EXT));\n'
     '    if crate::storage::backend::read_file(&user_path(name)).is_err() && crate::storage::backend::read_file(&legacy).is_ok() {\n'
     '        legacy\n'
     '    } else {\n'
     '        user_path(name)\n'
     '    }\n'
     '}\n'),
    ('    crate::storage::backend::delete_file(&user_path(name)).map_err(|e| anyhow::anyhow!("{e:?}"))?;',
     '    crate::storage::backend::delete_file(&stored_path(name)).map_err(|e| anyhow::anyhow!("{e:?}"))?;'),
    ('    let json = crate::storage::backend::read_file(&user_path(old)).map_err(|e| anyhow::anyhow!("{e:?}"))?;',
     '    let json = crate::storage::backend::read_file(&stored_path(old)).map_err(|e| anyhow::anyhow!("{e:?}"))?;'),
]))

# build.rs embeds the texture recipes: .fae (or .fflame).
edit("build.rs", exact([
    ('/// per `assets/textures/*.fflame`, the contents embedded with', '/// per `assets/textures/*.fae` (or `.fflame`), the contents embedded with'),
    ('                .filter(|n| n.ends_with(".fflame"))', '                .filter(|n| n.ends_with(".fae") || n.ends_with(".fflame"))'),
    ('        let stem = file.trim_end_matches(".fflame");', '        let stem = file.trim_end_matches(".fae").trim_end_matches(".fflame");'),
]))

# The visual suite and the benchmarks find both.
edit("tests/visual/run_tests.py", exact([
    ('for fflame_path in self.configs_dir.rglob("*.fflame"):',
     'for fflame_path in sorted([*self.configs_dir.rglob("*.fae"), *self.configs_dir.rglob("*.fflame")]):'),
]))
edit("scripts/run_benchmarks.py", exact([
    ('config_files = sorted(self.configs_dir.rglob("*.fflame"))',
     'config_files = sorted([*self.configs_dir.rglob("*.fae"), *self.configs_dir.rglob("*.fflame")])'),
]))

# The locale strings.
edit("locales/en.yml", exact([
    ('empty_hint: "Click \'Open File...\' to browse .fflame files"', 'empty_hint: "Click \'Open File...\' to browse config files (.fae)"'),
    ('save_as_fflame: "💾 Save as .fflame"', 'save_as_fflame: "💾 Save as .fae"'),
    ('load_fflame: "📁 Load .fflame"', 'load_fflame: "📁 Load .fae"'),
]))
