//! Subprocess smoke (P1T-D5): a SPARSE anchor proving the shipped binary
//! wires the same API the in-process tests drive — exit classes must match
//! the blessed manifests, and valid entries must produce real JSON on
//! stdout. Bytes stay owned by `crates/oxidizr-cli/tests/cli.rs`; this only
//! anchors the argv↔API plumbing (~6 entries, every run).
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use oxidizr_e2e::corpus::{self, Kind};
use oxidizr_e2e::manifest::Exit;

/// The `oxidizr` binary of this build, resolved in order:
/// 1. `$OXIDZR_BIN` (explicit override);
/// 2. `$CARGO_BIN_EXE_oxidizr` if the toolchain exports it (cargo only
///    guarantees that variable for a bin of the tested package itself, and
///    this bin lives in `oxidizr-cli` — verified absent here at runtime);
/// 3. walking up from the test executable for a sibling `oxidizr` — this
///    repo places test binaries under `target/<profile>/build/<pkg>/<hash>/
///    out/`, but the walk finds `target/<profile>/oxidizr` in any cargo
///    layout.
///
/// `cargo test --workspace` (the quartet) always builds the bin; a bare
/// `cargo test -p oxidizr-e2e` on a cold cache must build it first
/// (`cargo build -p oxidizr-cli --bin oxidizr`) — the failure text says so.
fn binary() -> PathBuf {
    for var in ["OXIDZR_BIN", "CARGO_BIN_EXE_oxidizr"] {
        if let Ok(path) = std::env::var(var) {
            return PathBuf::from(path);
        }
    }
    let exe = std::env::current_exe().expect("test executable");
    for dir in exe.ancestors() {
        let candidate = dir.join("oxidizr");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "oxidizr binary not found near {} — build it with \
         `cargo build -p oxidizr-cli --bin oxidizr` (the workspace test \
         quartet builds it automatically)",
        exe.display()
    );
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("oxidizr binary runs")
}

fn exit_of(code: Option<i32>) -> Exit {
    match code {
        Some(0) => Exit::Ok,
        Some(1) => Exit::Error,
        Some(2) => Exit::Usage,
        Some(3) => Exit::Io,
        other => panic!("unexpected exit code {other:?}"),
    }
}

#[test]
fn binary_matches_blessed_manifests() {
    let entries = corpus::discover().expect("corpus discovers");
    let with_dsl: Vec<_> = entries.iter().filter(|e| e.dsl.is_some()).collect();
    let mut smoke: Vec<_> = with_dsl
        .iter()
        .filter(|e| e.kind == Kind::Valid)
        .take(3)
        .chain(with_dsl.iter().filter(|e| e.kind == Kind::Invalid).take(3))
        .collect();
    smoke.sort_by_key(|e| e.full_name());
    assert!(
        smoke.iter().any(|e| e.kind == Kind::Valid)
            && smoke.iter().any(|e| e.kind == Kind::Invalid),
        "smoke needs valid and invalid entries"
    );

    let mut problems: Vec<String> = Vec::new();
    for entry in smoke {
        let sidecar = entry.dir.join(oxidizr_e2e::manifest::FILE_NAME);
        let manifest = oxidizr_e2e::manifest::Manifest::read(&sidecar).expect("sidecar readable");
        let expected = manifest.dsl2json.expect("dsl2json section");
        let dsl = entry.dsl.as_ref().expect("smoke entry has a DSL");
        let out = run(&["dsl2json", &dsl.display().to_string()]);
        let got = exit_of(out.status.code());
        if got != expected.exit {
            problems.push(format!(
                "{}: dsl2json exit {:?} != blessed {:?}",
                entry.full_name(),
                got,
                expected.exit
            ));
        }
        if expected.exit == Exit::Ok {
            if out.stdout.is_empty() {
                problems.push(format!("{}: empty stdout on ok", entry.full_name()));
            } else if serde_json::from_slice::<serde_json::Value>(&out.stdout).is_err() {
                problems.push(format!("{}: stdout is not JSON", entry.full_name()));
            }
        }
        // validate exit class too (exercises a second command path).
        let expected_v = manifest.validate.expect("validate section");
        let out = run(&["validate", &dsl.display().to_string()]);
        let got = exit_of(out.status.code());
        if got != expected_v.exit {
            problems.push(format!(
                "{}: validate exit {:?} != blessed {:?}",
                entry.full_name(),
                got,
                expected_v.exit
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "subprocess smoke diverged:\n{}",
        problems.join("\n")
    );
}
