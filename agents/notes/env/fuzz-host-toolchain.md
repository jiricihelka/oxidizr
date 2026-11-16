# Fuzzing: strip the cross-toolchain env (mingw leak)

Why: `cargo fuzz build` failed compiling libFuzzer's C++ runtime — the cc
crate used **`x86_64-w64-mingw32-g++`** (and even picked libFuzzer's *Windows*
sources) for a host-Linux sanitizer build.

Facts:
- the nix dev shell exports `CC/CXX/AR/RANLIB` for the windows-gnu leg (flake
  `windowsCross`); `cc` obeys those for ANY build unless stripped.
- fix lives in `crates/xtask/src/main.rs` `cmd_fuzz`: it `env_remove`s
  `CC/CXX/AR/RANLIB` before `cargo fuzz build|run` — CI runners have a clean
  environment, so this only matters locally (the mirror stays equal to CI).
- `fuzz/` is a SEPARATE cargo workspace (`exclude` in the root `Cargo.toml`):
  the fmt/clippy/test quartet never sees the targets; sanitizers still use
  the pinned nightly (cargo-fuzz requires it).
- seeds: `cmd_fuzz` copies `testdata/` fixtures into `fuzz/corpus/<target>/`
  (gitignored scratch — libFuzzer adds discoveries there next to the seeds).

Verify: `cargo xtask fuzz 3` (≈ 107 dsl + 26 json seeds, all three targets).

Last useful: 2026-11-16
