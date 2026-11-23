# nix dev shell

`nix develop` provides: pinned nightly rust toolchain, JDK 21 + `structurizr`
oracle, graphviz `dot`, the windows-gnu cross leg (see wine-windows-leg.md),
and cargo-llvm-cov/cargo-fuzz come via cargo (installed components) — CI installs them.

**Double-pin rule**: `rust-toolchain.toml` is authoritative for plain-cargo users;
the flake pins the same toolchain via flake.lock. A `pinMatch` eval guard (flake.nix)
compares the overlay toolchain's `rustc -Vv` against rust-toolchain.toml's channel —
a mismatch *fails* `nix flake check` (its message arrives as `builtins.trace` noise,
which looks like an error but is the guard reporting). Bump both together at a
milestone + re-check NIGHTLY.md.

`nix flake check` builds every dev-shell leg incl. the windows cross leg (catches
broken toolchain wiring without running a test). The flake is deliberately NOT
nixfmt-styled (compact list literals) — don't "fix" its formatting.

**Plain `cargo` works outside the dev shell** on this machine (rust-toolchain.toml
supplies the nightly; the full gate quartet runs without nix). Reach for
`nix develop` only for the extras above (`cargo-dist`, `critcmp`, Wine leg, `dot`,
oracle) — never as a precondition for `cargo test`/`fmt`/`doc`.

Last useful: 2026-11-22
