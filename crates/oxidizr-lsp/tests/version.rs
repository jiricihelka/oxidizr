//! `--version` smoke (§P4-LSP L5, item L-7): the P4-D10 binary hook.
//! `CARGO_BIN_EXE_oxidizr-lsp` is guaranteed here because the binary
//! belongs to THIS package (the e2e cross-package walk in
//! `oxidizr-e2e/tests/subprocess_smoke.rs` is only needed when it does
//! not). A null stdin keeps a mistaken server-mode invocation from
//! blocking forever, whatever this test asserts.
#![allow(clippy::unwrap_used)]

use std::process::Command;

fn run(arg: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_oxidizr-lsp"))
        .arg(arg)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("oxidizr-lsp binary runs");
    assert!(out.status.success(), "{arg}: exit {:?}", out.status.code());
    String::from_utf8(out.stdout).expect("utf-8 stdout")
}

#[test]
fn version_flags_print_name_and_version() {
    let expected = format!("oxidizr-lsp {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(run("--version").trim(), expected);
    assert_eq!(run("-V").trim(), expected);
}
