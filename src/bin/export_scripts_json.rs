//! Dump the shipped script library as JSON, for the API's bulk import.
//!
//! The script counterpart to `export_variations_json`, and the same
//! bargain: everything the API could want about a built-in script,
//! recovered from where it actually lives, in one command.
//!
//! Three sources, none of them a second copy of anything:
//!
//! * **The `script(...)` call** gives the declared name, the kind and
//!   the optional flags. Read by *running the script's declaration
//!   pass* (`ScriptHost::collect`) rather than by pattern-matching the
//!   source, so a name built from an expression reports what the app
//!   would show, and a script that declares its parameters inside an
//!   `if` is read the way the panel reads it.
//! * **The header comment** is the description. `parse_doc` splits it
//!   the way every other consumer does: a title line (which repeats
//!   the script's name and is dropped from the prose), a summary
//!   paragraph, and the body.
//! * **The file** is emitted verbatim as `source`. A script IS its
//!   text — reproducing it exactly is the point, and the CLI-parity
//!   fixtures in `wasm/script` are the standing proof that byte drift
//!   here changes what every published seed renders.
//!
//! Deliberately NOT reflowed, unlike the variation corpus: script
//! prose carries structure the stripper is written to preserve —
//! `lsystem.rhai` documents its turtle symbols as an indented table —
//! and the Scripts panel renders it verbatim. `description_plain` is
//! the same text through the same stripper the panel uses, so what the
//! API stores is what the app shows.
//!
//! ```text
//! cargo run --release --bin export_scripts_json -- out.json
//! ```
//!
//! Defaults to `output/scripts-corpus.json` (gitignored): it carries
//! every script's full source, so it is generated on demand rather
//! than committed.

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::script::{library, parse_doc, strip_markdown, ScriptHost, ScriptKind};

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "output/scripts-corpus.json".to_string());

    // The same base and host `library::discover` uses to read metadata,
    // so a script reports here exactly as it does in the picker.
    let base = FractalConfig::default();
    let host = ScriptHost::new();

    let mut rows = Vec::new();
    let mut problems = Vec::new();

    for (file, source) in library::EMBEDDED {
        // The stem is the reserved name: what `run_script` resolves and
        // what the API refuses at CREATE time (engine-contract.json's
        // `builtin_scripts`). Not the display name, which is prose.
        let id = file.trim_end_matches(".rhai");

        let meta = match host.collect(source, &base) {
            Ok(meta) => meta,
            Err(e) => {
                problems.push(format!("{id}: does not collect: {e}"));
                continue;
            }
        };
        if meta.kind.is_none() {
            problems.push(format!("{id}: no kind — the script(...) call is missing one"));
        }

        let doc = parse_doc(source);
        // Summary and body are one prose field for the API; they stay
        // separate as well, because the panel shows the summary and
        // keeps the body behind a disclosure, and a listing wants the
        // short form without re-splitting it.
        let description = match (doc.summary.is_empty(), doc.body.is_empty()) {
            (true, true) => String::new(),
            (false, true) => doc.summary.clone(),
            (true, false) => doc.body.clone(),
            (false, false) => format!("{}\n\n{}", doc.summary, doc.body),
        };
        if description.trim().is_empty() {
            problems.push(format!("{id}: no header comment to describe it"));
        }

        let parameters: Vec<serde_json::Value> =
            meta.params.iter().map(|p| p.to_api_json()).collect();

        rows.push(serde_json::json!({
            "id": id,
            "name": meta.name,
            "kind": meta.kind.map(ScriptKind::as_str),
            "flags": {
                "norng": meta.flags.no_rng,
                "palette": meta.flags.palette,
            },

            "title": doc.title,
            "summary": doc.summary,
            "description": description,
            "description_plain": strip_markdown(&description),

            "parameters": parameters,

            // Verbatim. This is the artifact; everything above is a
            // reading of it.
            "source": source,

            // What the collect pass wanted to say — an unknown flag,
            // usually. Empty for everything shipped, and a shipped
            // script that starts warning should be visible here rather
            // than only in a panel nobody is looking at.
            "warnings": meta.warnings,
        }));
    }

    rows.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));

    let doc = serde_json::json!({
        "$comment": "GENERATED by `cargo run --bin export_scripts_json`. \
                     Every built-in script: its `script(...)` declaration \
                     (name, kind, flags), its header comment as description, \
                     its declared parameters, and its source verbatim. \
                     `id` is the reserved stem — the same list \
                     engine-contract.json carries as `builtin_scripts`.",
        "contract_shape": fractal_flame_wgpu::contract::generate()["shape"],
        "count": rows.len(),
        "scripts": rows,
    });

    // A corpus with a hole in it is worse than no corpus: the API has
    // no other source for any of this, so a missing description or an
    // undeclared kind would just become a bad row nobody notices.
    if !problems.is_empty() {
        eprintln!("Error: {} shipped script(s) are not exportable:", problems.len());
        for p in &problems {
            eprintln!("  {p}");
        }
        std::process::exit(1);
    }

    let json = serde_json::to_string_pretty(&doc).expect("serialize corpus");
    if let Some(dir) = std::path::Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&out_path, &json).unwrap_or_else(|e| {
        eprintln!("Error: cannot write `{out_path}`: {e}");
        std::process::exit(1);
    });

    let generators = rows_of_kind(&doc, "generator");
    let modifiers = rows_of_kind(&doc, "modifier");
    eprintln!(
        "Wrote {out_path} — {} scripts ({generators} generators, {modifiers} modifiers), {:.0} KB",
        doc["count"],
        json.len() as f64 / 1e3,
    );
}

fn rows_of_kind(doc: &serde_json::Value, kind: &str) -> usize {
    doc["scripts"]
        .as_array()
        .map(|rows| rows.iter().filter(|r| r["kind"] == kind).count())
        .unwrap_or(0)
}
