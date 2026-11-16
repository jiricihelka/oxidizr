//! M7 mandated benches (agents/plan/TESTING.md §Benchmarks): DSL parse over
//! the FULL fixture corpus plus the two biggest single fixtures (worst-case
//! proxy). Bench-only — reads the vendored corpus (testdata/), never a gate.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};
use std::fs;
use std::path::{Path, PathBuf};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// Collect `(path, text)` for every `extension` file below `dir` whose bytes
/// are UTF-8 (the latin-1 probe fixture can't reach `&str` APIs — V-8).
fn corpus(sub: &str, extension: &str) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, extension: &str, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return; // missing corpus dir: bench measures nothing (CI has it)
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, extension, out);
            } else if path.extension().is_some_and(|e| e == extension)
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(&testdata().join(sub), extension, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic across runs
    out
}

fn parse_corpus(c: &mut Criterion) {
    let files = corpus("dsl", "dsl");
    c.bench_function("m7_dsl_parse_corpus", |b| {
        b.iter(|| {
            let mut work = 0_usize;
            for (path, text) in &files {
                let lossy = path.to_string_lossy();
                let opts = ParseOptions::file(&lossy);
                if let Ok(r) = parse_dsl(text, opts) {
                    work += r.errors.len();
                }
                work += black_box(text).lines().count();
            }
            black_box(work)
        });
    });
}

fn parse_named(c: &mut Criterion) {
    // Worst-case proxy: the two biggest vendored fixtures (~12 KB each).
    for name in ["test.dsl", "big-bank-plc.dsl"] {
        let path = testdata().join("dsl").join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let id = format!("m7_dsl_parse_{name}");
        c.bench_function(&id, |b| {
            b.iter(|| {
                let lossy = path.to_string_lossy();
                let opts = ParseOptions::file(&lossy);
                if let Ok(r) = parse_dsl(black_box(&text), opts) {
                    black_box(r.errors.len());
                }
            });
        });
    }
}

criterion_group!(benches, parse_corpus, parse_named);
criterion_main!(benches);
