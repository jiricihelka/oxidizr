//! `oxidizr oracle` user-command tests (§P4-M5 E-1, F4, P4-D6) — Category A.
//! The legs drive a FAKE oracle: a POSIX-shell shim implementing the one
//! invocation the engine makes (`export --workspace F --format json
//! --output DIR`), so the golden round — including the inspection-counter
//! normalization — is proven through the real `oxidizr` binary without the
//! Java prerequisite (that A/B leg is `cargo xtask oracle landscape`'s
//! family, Category B). `--bin` always explicit ⇒ the tests never consult
//! `$OXIDZR_STRUCTURIZR_BIN`/PATH (hermetic); the exit family asserted is
//! the §P4-M5 E-1 contract: 0 ok · 1 mismatch/missing/error · 3 discovery/
//! spawn/I-O.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The fake reference CLI: writes the reference's pretty shape (Jackson
/// `" : "` separators, NO trailing newline) with two inspection counters
/// present, so the golden bytes prove normalization ran. A workspace stem
/// starting with `bad` reproduces a reference-side failure (exit 1 +
/// stderr, the leg-level `error` status).
const FAKE: &str = r#"#!/bin/sh
ws=""
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --workspace) ws="$2"; shift 2 ;;
        --output) out="$2"; shift 2 ;;
        *) shift ;;
    esac
done
stem=$(basename "$ws")
stem=${stem%.*}
case "$stem" in
    bad*) echo "fake oracle refuses $stem" >&2; exit 1 ;;
esac
printf '{\n  "name" : "Fake",\n  "properties" : {\n    "a" : "1",\n    "structurizr.inspection.error" : "0",\n    "structurizr.inspection.warning" : "0"\n  }\n}' > "$out/$stem.json"
exit 0
"#;

/// The golden bytes after normalization: both inspection lines gone, the
/// surviving `"a"` line's comma fixed up, EOF parity kept.
const NORMALIZED: &str =
    "{\n  \"name\" : \"Fake\",\n  \"properties\" : {\n    \"a\" : \"1\"\n  }\n}";

fn oxidizr() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oxidizr"));
    // Hermetic: NO_COLOR for deterministic stderr, and the real-oracle env
    // var is removed so discovery cannot leak in from the dev shell.
    cmd.env("NO_COLOR", "1")
        .env_remove("OXIDZR_STRUCTURIZR_BIN");
    cmd
}

fn run(args: &[&str]) -> Output {
    oxidizr().args(args).stdin(Stdio::null()).output().unwrap()
}

/// Unique temp dir per test (parallel tests share the process id, so the
/// caller's tag disambiguates).
fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-oracle-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fake_oracle(dir: &Path) -> PathBuf {
    let script = dir.join("fake-structurizr.sh");
    std::fs::write(&script, FAKE).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn workspace(dir: &Path, name: &str) -> PathBuf {
    let ws = dir.join(name);
    std::fs::write(&ws, "workspace \"Fake\" {\n}\n").unwrap();
    ws
}

#[test]
fn gen_writes_the_normalized_golden() {
    let dir = tempdir("gen");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "app.dsl");
    let out = run(&[
        "oracle",
        "gen",
        "--bin",
        bin.to_str().unwrap(),
        &ws.to_string_lossy(),
    ]);
    assert!(
        out.status.success(),
        "gen failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let golden = dir.join("app.golden.json");
    assert_eq!(std::fs::read_to_string(&golden).unwrap(), NORMALIZED);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("generated"), "report: {stdout}");
    assert!(stdout.contains("oracle gen: 1/1 ok"), "summary: {stdout}");
}

#[test]
fn gen_is_idempotent_and_verify_then_matches() {
    let dir = tempdir("idem");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "app.dsl");
    let args = ["oracle", "gen", "--bin", bin.to_str().unwrap()];
    run(&[&args[..], &[ws.to_string_lossy().as_ref()]].concat());
    let golden = dir.join("app.golden.json");
    let first = std::fs::read(&golden).unwrap();
    let out = run(&[&args[..], &[ws.to_string_lossy().as_ref()]].concat());
    assert!(out.status.success());
    assert_eq!(
        std::fs::read(&golden).unwrap(),
        first,
        "gen must be a fixed point"
    );
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        bin.to_str().unwrap(),
        ws.to_string_lossy().as_ref(),
    ]);
    assert!(out.status.success(), "verify should match the fresh golden");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("oracle verify: 1/1 ok"),
        "summary: {stdout}"
    );
}

#[test]
fn verify_mismatch_and_missing_exit_one() {
    let dir = tempdir("mismatch");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "app.dsl");
    // missing golden first
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        bin.to_str().unwrap(),
        ws.to_string_lossy().as_ref(),
    ]);
    assert_eq!(out.status.code(), Some(1), "missing golden is exit 1");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("missing"), "report: {stdout}");
    // then a mismatch
    run(&[
        "oracle",
        "gen",
        "--bin",
        bin.to_str().unwrap(),
        ws.to_string_lossy().as_ref(),
    ]);
    let golden = dir.join("app.golden.json");
    let mut bytes = std::fs::read(&golden).unwrap();
    bytes.extend_from_slice(b"\n");
    std::fs::write(&golden, &bytes).unwrap();
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        bin.to_str().unwrap(),
        ws.to_string_lossy().as_ref(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a trailing-newline-only edit is still a mismatch"
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("MISMATCH"), "report: {stdout}");
}

#[test]
fn json_report_is_the_schema_one_document() {
    let dir = tempdir("json");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "app.dsl");
    let out = run(&[
        "oracle",
        "gen",
        "--bin",
        bin.to_str().unwrap(),
        "--json",
        ws.to_string_lossy().as_ref(),
    ]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("{\"schema\":1,\"results\":[{"),
        "document: {stdout}"
    );
    assert!(
        stdout.contains("\"status\":\"generated\""),
        "document: {stdout}"
    );
    assert!(stdout.contains("app.golden.json"), "document: {stdout}");
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        bin.to_str().unwrap(),
        "--json",
        ws.to_string_lossy().as_ref(),
    ]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.ends_with("\"status\":\"match\"}]}\n"),
        "document: {stdout}"
    );
}

#[test]
fn bin_missing_is_exit_three() {
    let dir = tempdir("bin3");
    let ws = workspace(&dir, "app.dsl");
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        "/nonexistent/structurizr",
        ws.to_string_lossy().as_ref(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "discovery failure is the Io class"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("--bin") && stderr.contains("is not a file"),
        "OXZ-5006: {stderr}"
    );
}

#[test]
fn missing_workspace_file_is_exit_three() {
    let dir = tempdir("ws3");
    let bin = fake_oracle(&dir);
    let out = run(&[
        "oracle",
        "verify",
        "--bin",
        bin.to_str().unwrap(),
        dir.join("nope.dsl").to_string_lossy().as_ref(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "I/O leg failure raises the command"
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("cannot read"), "report: {stdout}");
}

#[test]
fn reference_failure_is_a_leg_error_exit_one() {
    let dir = tempdir("bad");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "bad.dsl");
    let out = run(&[
        "oracle",
        "gen",
        "--bin",
        bin.to_str().unwrap(),
        ws.to_string_lossy().as_ref(),
    ]);
    // The reference RAN and rejected the workspace: leg-level `error`,
    // exit 1 (not 3 — spawn and I/O were fine).
    assert_eq!(out.status.code(), Some(1));
    assert!(!dir.join("bad.golden.json").exists(), "no golden on error");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("error"), "report: {stdout}");
    assert!(stdout.contains("oracle gen: 0/1 ok"), "summary: {stdout}");
}

#[test]
fn goldens_dir_and_quiet_and_env_discovery() {
    let dir = tempdir("dirs");
    let bin = fake_oracle(&dir);
    let ws = workspace(&dir, "app.dsl");
    let goldens = dir.join("g"); // does NOT exist yet — gen creates it
    let out = oxidizr()
        .args([
            "oracle",
            "gen",
            "--bin",
            bin.to_str().unwrap(),
            "--goldens",
            goldens.to_string_lossy().as_ref(),
            "--quiet",
            ws.to_string_lossy().as_ref(),
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "--quiet carries no bytes");
    assert_eq!(
        std::fs::read_to_string(goldens.join("app.golden.json")).unwrap(),
        NORMALIZED,
        "the golden lands under --goldens DIR"
    );
    // Discovery through $OXIDZR_STRUCTURIZR_BIN (no --bin on the argv).
    let out = oxidizr()
        .env("OXIDZR_STRUCTURIZR_BIN", &bin)
        .args([
            "oracle",
            "verify",
            "--quiet",
            "--goldens",
            goldens.to_string_lossy().as_ref(),
            ws.to_string_lossy().as_ref(),
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "env discovery must work: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
