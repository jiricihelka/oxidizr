//! M7 mandated bench (agents/plan/TESTING.md §Benchmarks): `fmt` over the
//! full UTF-8 DSL fixture corpus (same corpus + corpus-walk convention as
//! `benches/parse.rs`).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_dsl::fmt;
use std::fs;
use std::path::{Path, PathBuf};

fn corpus(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "dsl")
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push(text);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort(); // deterministic across runs
    out
}

fn fmt_corpus(c: &mut Criterion) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/dsl");
    let files = corpus(&root);
    c.bench_function("m7_dsl_fmt_corpus", |b| {
        b.iter(|| {
            let mut chars = 0_usize;
            for text in &files {
                chars += fmt::format(black_box(text)).chars().count();
            }
            black_box(chars)
        });
    });
}

criterion_group!(benches, fmt_corpus);
criterion_main!(benches);
