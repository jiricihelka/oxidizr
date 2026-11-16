# toolbox — gates and the verification loop (what is on PATH)

Inventory verified 2026-11-16 in `nix develop`. Detail lives in the linked
notes — this is the map, don't re-derive here.

| tool | when | how |
|---|---|---|
| `cargo xtask ci` | before EVERY commit (default quartet: fmt clippy test refs) | optional steps `lint` (actionlint), `windows` (Wine leg; dev shell provides runner+linker+stub) — testing/local-ci-mirrors.md |
| cargo `fmt`/`clippy`/`test` | inner loop; CHAIN into one command (the target-dir lock serializes parallel cargo) | `cargo test -p oxidizr-dsl --test m6_fmt` filters are fine |
| `structurizr` oracle (app 2026.10.26 = lib 6.2.4 = monorepo `ad34036`; `$OXIDZR_STRUCTURIZR_BIN` set in the shell) + JDK 21 | Category B only — slow JVM, NEVER in the quartet | `cargo xtask oracle verify` / `gen [--fixture N]` — env/reference-oracle.md |
| `wine` + mingw-w64 cross | reproduce Windows-only test failures locally | `cargo xtask ci windows` — env/wine-windows-leg.md |
| `cargo xtask bench [save <L> \| cmp <L>]`, `cargo xtask cov` | perf regression checks / coverage (report-only) | critcmp takes only in-root baseline names (see bench help) |
| `cargo-fuzz` | M6-d fuzz targets — installed, but NO `fuzz/` dir until M6-d | don't reach for it before then |
| structurizr monorepo clone | the reference DECIDES ambiguous behavior — read the Java, don't guess | `git clone https://github.com/structurizr/structurizr <scratch> && git -C <scratch> checkout ad34036` (COMPAT.md §Source of truth) |
| `cargo xtask vendor --src <clone>` | (re)vendoring reference fixtures into testdata/ | testdata/README.md contract |

Absent on this machine — do NOT chase: miri, cargo-nextest, cargo-edit,
cargo-audit, sccache, mold, hyperfine, just, cargo-outdated.
