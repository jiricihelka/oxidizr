//! P1T-D7 corpus benches (`cargo xtask bench` — feature `e2e-bench`, local
//! dev only; CI's `cargo bench --workspace` skips this target). Whole-corpus
//! `dsl2json`/`json2json`/`fmt`/`validate` over the REALISTIC valid entries
//! plus the biggest-entry worst cases and a self-`reconcile` pair. In-process
//! like the suite (no subprocess), hermetic options, `execute` end-to-end —
//! these measure the shipped pipeline, not a crate seam. Bench-only: never a
//! gate (TESTING.md §Benchmarks).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidizr_api::{FmtMode, Input, Request, execute};
use oxidizr_e2e::corpus::{self, Entry, Kind};
use std::fs;
use std::path::PathBuf;

/// Valid entries with a DSL, sorted, inputs pre-read (setup is not measured).
fn valid_entries() -> Vec<(Entry, Vec<u8>)> {
    let mut entries: Vec<(Entry, Vec<u8>)> = corpus::discover()
        .expect("corpus discovers")
        .into_iter()
        .filter(|e| e.kind == Kind::Valid && e.dsl.is_some())
        .filter_map(|e| {
            let text = fs::read(e.dsl.as_ref()?).ok()?;
            Some((e, text))
        })
        .collect();
    entries.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    entries
}

fn dsl_file(entry: &Entry) -> PathBuf {
    entry.dsl.clone().expect("filtered above")
}

/// Whole-corpus `dsl2json` (the headline pipeline number).
fn bench_dsl2json(c: &mut Criterion) {
    let entries = valid_entries();
    c.bench_function("e2e_dsl2json_all", |b| {
        b.iter(|| {
            let mut bytes = 0_usize;
            for (entry, _) in &entries {
                let out = execute(
                    Request::Dsl2Json {
                        input: Input::File(dsl_file(entry)),
                        output: None,
                    },
                    &oxidizr_e2e::hermetic_options(),
                );
                bytes += out.bytes.map_or(0, |b| b.len());
            }
            black_box(bytes)
        });
    });
    let biggest = entries
        .iter()
        .max_by_key(|(_, text)| text.len())
        .expect("corpus non-empty");
    let id = format!("e2e_dsl2json_biggest_{}", biggest.0.name);
    c.bench_function(&id, |b| {
        b.iter(|| {
            let out = execute(
                Request::Dsl2Json {
                    input: Input::File(dsl_file(&biggest.0)),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            );
            black_box(out.bytes.map_or(0, |b| b.len()))
        });
    });
}

/// Whole-corpus `json2json` over the entries' own exports (setup: one
/// `dsl2json` each, outside the measurement).
fn bench_json2json(c: &mut Criterion) {
    let layouts: Vec<Vec<u8>> = valid_entries()
        .iter()
        .filter_map(|(entry, _)| {
            execute(
                Request::Dsl2Json {
                    input: Input::File(dsl_file(entry)),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            )
            .bytes
        })
        .collect();
    c.bench_function("e2e_json2json_all", |b| {
        b.iter(|| {
            let mut bytes = 0_usize;
            for layout in &layouts {
                let out = execute(
                    Request::Json2Json {
                        input: Input::Bytes(black_box(layout).clone()),
                        output: None,
                    },
                    &oxidizr_e2e::hermetic_options(),
                );
                bytes += out.bytes.map_or(0, |b| b.len());
            }
            black_box(bytes)
        });
    });
}

/// Whole-corpus `fmt --print`.
fn bench_fmt(c: &mut Criterion) {
    let entries = valid_entries();
    c.bench_function("e2e_fmt_all", |b| {
        b.iter(|| {
            let mut bytes = 0_usize;
            for (entry, _) in &entries {
                let out = execute(
                    Request::Fmt {
                        inputs: vec![Input::File(dsl_file(entry))],
                        mode: FmtMode::Print,
                    },
                    &oxidizr_e2e::hermetic_options(),
                );
                bytes += out.bytes.map_or(0, |b| b.len());
            }
            black_box(bytes)
        });
    });
}

/// Whole-corpus `validate` (one batched invocation — the CLI form).
fn bench_validate(c: &mut Criterion) {
    let files: Vec<PathBuf> = valid_entries().iter().map(|(e, _)| dsl_file(e)).collect();
    c.bench_function("e2e_validate_all", |b| {
        b.iter(|| {
            let out = execute(
                Request::Validate {
                    files: black_box(&files).clone(),
                },
                &oxidizr_e2e::hermetic_options(),
            );
            black_box(out.log.len())
        });
    });
}

/// `reconcile` over the realistic pair: biggest entry's DSL (model) against
/// its own layout export (the way the command is actually fed, P1T-D3).
fn bench_reconcile(c: &mut Criterion) {
    let entries = valid_entries();
    let (biggest, _) = entries
        .iter()
        .max_by_key(|(_, text)| text.len())
        .expect("corpus non-empty");
    let layout = execute(
        Request::Dsl2Json {
            input: Input::File(dsl_file(biggest)),
            output: None,
        },
        &oxidizr_e2e::hermetic_options(),
    )
    .bytes
    .expect("valid entry exports");
    let id = format!("e2e_reconcile_{}", biggest.name);
    c.bench_function(&id, |b| {
        b.iter(|| {
            let out = execute(
                Request::Reconcile {
                    dsl: Input::File(dsl_file(biggest)),
                    layout: Input::Bytes(black_box(&layout).clone()),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            );
            black_box(out.bytes.map_or(0, |b| b.len()))
        });
    });
}

/// Whole-corpus diagram `export` (Phase 2 M4-d): load + dialect export +
/// the reference write loop, per format. Output goes to a scratch dir
/// outside the measurement setup; file names collide across iterations on
/// purpose (the workspace-id prefix is stable) — each pass overwrites.
fn bench_export(c: &mut Criterion) {
    let entries = valid_entries();
    let dir = std::env::temp_dir().join(format!("oxidizr-bench-export-{}", std::process::id()));
    for format in ["plantuml", "mermaid"] {
        let id = format!("e2e_export_{format}_all");
        c.bench_function(&id, |b| {
            b.iter(|| {
                let mut notes = 0_usize;
                for (entry, _) in &entries {
                    let out = execute(
                        Request::Export {
                            workspace: dsl_file(entry),
                            format: black_box(format).to_owned(),
                            output: Some(dir.clone()),
                        },
                        &oxidizr_e2e::hermetic_options(),
                    );
                    notes += out.log.len();
                }
                black_box(notes)
            });
        });
    }
    let _ = fs::remove_dir_all(&dir);
}

criterion_group!(
    benches,
    bench_dsl2json,
    bench_json2json,
    bench_fmt,
    bench_validate,
    bench_reconcile,
    bench_export
);
criterion_main!(benches);
