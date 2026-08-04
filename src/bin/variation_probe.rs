//! Run the variation math probe, or compare two of its reports.
//!
//! ```text
//! cargo run --release --bin variation_probe
//! cargo run --release --bin variation_probe -- compare a.txt b.txt
//! ```
//!
//! Writes `docs/generated/variation-probe.txt` by default. The report is
//! committed, so a diff between platforms — or between builds — is a
//! `git diff`. See `src/probe/` for what the columns mean.

use fractal_flame_wgpu::probe::{self, report};
use std::path::{Path, PathBuf};

const DEFAULT_REPORT: &str = "docs/generated/variation-probe.txt";

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("compare") => match args.get(1..3) {
            Some([a, b]) => do_compare(Path::new(a), Path::new(b)),
            _ => {
                eprintln!("usage: variation_probe compare <old> <new>");
                2
            }
        },
        Some("--help") | Some("-h") => {
            println!(
                "variation_probe                       run the probe, write {DEFAULT_REPORT}\n\
                 variation_probe <path>                run the probe, write <path>\n\
                 variation_probe --no-sweep            skip the parameter sweep\n\
                 variation_probe compare <old> <new>   compare two reports\n\
                 \n\
                 The sweep writes a second report beside the first, named\n\
                 <path>-sweep.txt. Compare it the same way."
            );
            0
        }
        Some("--no-sweep") => do_run(PathBuf::from(DEFAULT_REPORT), false),
        Some(path) => do_run(PathBuf::from(path), true),
        None => do_run(PathBuf::from(DEFAULT_REPORT), true),
    };
    std::process::exit(code);
}

fn do_run(out: PathBuf, sweep: bool) -> i32 {
    // A running log, flushed as each batch starts. A GPU hang is a
    // device loss that takes the process down with it and cannot be
    // caught here, so the only way to learn *which* batch hung is to
    // have written it down before dispatching.
    let progress = out.with_extension("progress");

    let outcome = pollster::block_on(probe::run::run(sweep, |what| {
        eprintln!("  {what}");
        let _ = std::fs::write(&progress, format!("running {what}\n"));
    }));

    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => {
            eprintln!("probe failed: {e}");
            return 1;
        }
    };

    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&out, outcome.report.render()) {
        eprintln!("could not write {}: {e}", out.display());
        return 1;
    }

    if sweep {
        let sweep_path = sweep_path_for(&out);
        if let Err(e) = std::fs::write(&sweep_path, outcome.sweep.render()) {
            eprintln!("could not write {}: {e}", sweep_path.display());
            return 1;
        }
        println!(
            "{} sweep entries -> {}",
            outcome.sweep.entries.len(),
            sweep_path.display()
        );
    }

    if !sweep {
        // Both reports are committed and describe the same build. Having
        // just regenerated one and not the other, the file on disk now
        // describes a build that no longer exists — and it will be
        // committed alongside a report it no longer matches unless
        // someone is told.
        let stale = sweep_path_for(&out);
        if stale.exists() {
            eprintln!(
                "warning: {} was NOT regenerated and now describes an older build.\n\
                 \x20        Re-run without --no-sweep before committing, or drop it.",
                stale.display()
            );
        }
    }

    let timings_path = out.with_extension("timings.txt");
    if let Err(e) = std::fs::write(&timings_path, outcome.timings.render()) {
        eprintln!("could not write {}: {e}", timings_path.display());
    }
    let _ = std::fs::remove_file(&progress);

    println!("{} entries -> {}", outcome.report.entries.len(), out.display());
    if !outcome.report.skipped.is_empty() {
        println!("{} skipped — see the report header", outcome.report.skipped.len());
    }
    for (what, ms, factor) in outcome.timings.outliers(4.0) {
        println!("slow: {what} took {ms:.0}ms ({factor:.1}x the median)");
    }
    0
}

fn do_compare(a: &Path, b: &Path) -> i32 {
    let (ra, rb) = match (read(a), read(b)) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("{e}");
            return 2;
        }
    };

    let divergences = match report::compare(&ra, &rb) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    println!("{}  {} / {}", a.display(), ra.meta.adapter, ra.meta.backend);
    println!("{}  {} / {}", b.display(), rb.meta.adapter, rb.meta.backend);
    println!();

    if divergences.is_empty() {
        println!("no differences");
        return 0;
    }

    let hard = divergences.iter().filter(|d| d.is_hard()).count();
    for d in &divergences {
        match d {
            report::Divergence::Class { name, dim, at, before, after } => {
                println!("CLASS  {name} ({dim}) at {}", at.join(", "));
                println!("         {before}");
                println!("         {after}");
            }
            report::Divergence::Magnitude { name, dim } => {
                println!("value  {name} ({dim}) — same kinds, magnitudes moved");
            }
            report::Divergence::OnlyIn { name, dim, which } => {
                println!("ONLY   {name} ({dim}) present only in the {which} report");
            }
        }
    }

    println!();
    println!(
        "{hard} behavioural difference(s), {} value-only",
        divergences.len() - hard
    );
    if hard > 0 {
        println!(
            "\nCLASS differences mean a value changed kind — zero vs finite, finite vs\n\
             NaN, and so on. No rounding difference can do that, so each one is a real\n\
             behavioural difference between the two builds.\n\
             `value` lines are magnitudes past the comparison tolerance: worth reading,\n\
             but a value sitting on a quantisation boundary can produce one on its own."
        );
    }

    // Exit non-zero only for the hard signal. A value-only difference is
    // a prompt to look, not a failure — wiring it to a failing exit code
    // is how a check starts getting ignored.
    i32::from(hard > 0)
}

/// The sweep report sits beside its base report, so a directory holding
/// two platforms' output pairs up by name.
fn sweep_path_for(out: &Path) -> PathBuf {
    let stem = out.file_stem().map_or_else(|| "probe".into(), |s| s.to_string_lossy().to_string());
    let ext = out.extension().map_or_else(|| "txt".into(), |s| s.to_string_lossy().to_string());
    out.with_file_name(format!("{stem}-sweep.{ext}"))
}

fn read(path: &Path) -> Result<report::Report, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    report::Report::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}
