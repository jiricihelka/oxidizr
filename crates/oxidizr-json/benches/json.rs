//! M7 mandated benches (agents/plan/TESTING.md §Benchmarks): JSON read and
//! write over the vendored corpus — goldens, read-corpus, validation fixtures,
//! probes (same corpus-walk convention as the DSL benches in `oxidizr-dsl`).
//! Bench-only, never a gate.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_core::Workspace;
use oxidizr_json::{from_slice, to_vec};
use std::fs;
use std::path::{Path, PathBuf};

/// Corpus roots (below `testdata/`): committed goldens, the reference
/// read-corpus, validation fixtures and M3b probes.
fn corpus() -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return; // missing corpus dir: bench measures nothing (CI has it)
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "json")
                && let Ok(bytes) = fs::read(&path)
            {
                out.push((path, bytes));
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let mut out = Vec::new();
    for sub in ["golden", "read-corpus", "validation", "probes"] {
        walk(&root.join(sub), &mut out);
    }
    out.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic across runs
    out
}

/// Cheap content counter so the optimiser can elide nothing.
fn model_size(ws: &Workspace) -> usize {
    ws.model.people.len()
        + ws.model.software_systems.len()
        + ws.model.deployment_nodes.len()
        + ws.model.relationships.len()
}

fn json_read(c: &mut Criterion) {
    let files = corpus();
    c.bench_function("m7_json_read_corpus", |b| {
        b.iter(|| {
            let mut work = 0_usize;
            for (_, bytes) in &files {
                match from_slice(black_box(bytes)) {
                    Ok(ws) => work += model_size(&ws),
                    Err(_) => work += 1, // rejection is a measured outcome too
                }
            }
            black_box(work)
        });
    });
}

fn json_write(c: &mut Criterion) {
    // Write-path corpus: everything the reader accepts (setup cost excluded).
    let workspaces: Vec<Workspace> = corpus()
        .into_iter()
        .filter_map(|(_, bytes)| from_slice(&bytes).ok())
        .collect();
    c.bench_function("m7_json_write_corpus", |b| {
        b.iter(|| {
            let mut bytes = 0_usize;
            for ws in &workspaces {
                if let Ok(out) = to_vec(black_box(ws)) {
                    bytes += out.len();
                }
            }
            black_box(bytes)
        });
    });
}

criterion_group!(benches, json_read, json_write);
criterion_main!(benches);
