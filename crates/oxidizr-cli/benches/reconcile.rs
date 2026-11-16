//! M7 mandated bench (agents/plan/TESTING.md §Benchmarks): `reconcile` (F1) —
//! the identifier/canonical matching plus geometry graft. Setup (parse/read)
//! is excluded; only the pure merge runs in the timed loop.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_cli::reconcile::reconcile;
use oxidizr_core::Workspace;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};
use oxidizr_json::from_slice;
use std::fs;
use std::path::PathBuf;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn parse_dsl_file(name: &str) -> Option<Workspace> {
    let path = testdata().join("dsl").join(name);
    let text = fs::read_to_string(&path).ok()?;
    let lossy = path.to_string_lossy();
    let opts = ParseOptions::file(&lossy);
    parse_dsl(&text, opts).ok().map(|r| r.workspace)
}

/// Full-match case: layout and model are the same workspace (the "DSL edited,
/// ids shifted, geometry re-attaches everywhere" shape) — every view, element
/// and relationship runs the graft.
fn reconcile_matched(c: &mut Criterion) {
    let Some(updated) = parse_dsl_file("big-bank-plc.dsl") else {
        return;
    };
    let layout = updated.clone();
    c.bench_function("m7_reconcile_matched_big_bank", |b| {
        b.iter(|| {
            let (ws, report) = reconcile(black_box(&layout), &updated);
            black_box((
                ws.model.relationships.len(),
                report.views,
                report.elements,
                report.relationships,
                report.dropped.len(),
            ));
        });
    });
}

/// Near-zero-match case: layout from the big official export, model from an
/// unrelated DSL file — claim machinery and drop reporting at scale.
fn reconcile_cross_corpus(c: &mut Criterion) {
    let layout_path = testdata().join("read-corpus/structurizr-36141-workspace.json");
    let Ok(bytes) = fs::read(layout_path) else {
        return;
    };
    let Ok(layout) = from_slice(&bytes) else {
        return;
    };
    let Some(updated) = parse_dsl_file("test.dsl") else {
        return;
    };
    c.bench_function("m7_reconcile_cross_corpus", |b| {
        b.iter(|| {
            let (ws, report) = reconcile(black_box(&layout), black_box(&updated));
            black_box((
                ws.model.relationships.len(),
                report.elements,
                report.dropped.len(),
                report.ambiguous.len(),
            ));
        });
    });
}

criterion_group!(benches, reconcile_matched, reconcile_cross_corpus);
criterion_main!(benches);
