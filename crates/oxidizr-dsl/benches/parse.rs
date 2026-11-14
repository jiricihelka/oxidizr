//! M0 criterion skeleton — plumbing + committed baseline only.
//!
//! Real mandated targets land per phase plan: DSL parse (M4), `fmt` (M6), and the
//! JSON corpus benches in `oxidizr-json` (M3). Until then this measures a plain
//! source scan over a synthetic DSL string, so the bench job, baseline commit and
//! critcmp workflow are exercised from day one.

use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn scan_lines(c: &mut Criterion) {
    let src = "workspace \"S\" \"d\" {\n    model {\n        a = person \"A\" \"d\"\n    }\n}\n"
        .repeat(200);
    c.bench_function("m0_scaffold_scan", |b| {
        b.iter(|| {
            let mut chars = 0_usize;
            for line in black_box(&src).lines() {
                chars += line.chars().count();
            }
            black_box(chars);
        });
    });
}

criterion_group!(benches, scan_lines);
criterion_main!(benches);
