# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T6 commit (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, `v0.1.0` local) + **PHASE 1T COMPLETE
  (T0–T6, no tag — interlude)**: API crate, 300-entry corpus (use-case sidecars,
  never bytes), full-tier invariants (`OXIDZR_E2E_FULL=1`, nightly `e2e.yml`,
  caught 2 oracle-fixed engine bugs), six `e2e_` benches behind `e2e-bench`
  + `1T` baseline, close-out docs (TESTING.md §E2E + 1T bench paragraph,
  PROGRESS log, README dev line, note `testing/e2e-suite-operations.md` with
  the bless-REVIEW rule). Quartet + actionlint green at every step.
- **Next:** **PHASE 2 M0** per `build/PHASE_2.md`: new crate
  `crates/oxidizr-export` (deps core/json/diag — **NO dsl/ast**, P2-D1),
  `IndentingWriter` (buffer into one `String`), `Diagram` type
  (`key, definition, file_extension, frames, legend`), CLI `export` command
  skeleton: format dispatch (`theme`, `plantuml[/structurizr|/c4plantuml]`
  ±`-light|-dark`, `mermaid`) + reference file naming
  `structurizr-<wsId>-<viewKey>.<ext>` (legend `-key`, frames `-<n>`,
  `--prefix`); theme.json de/ser in `oxidizr-json`; vendored default theme
  (`include_bytes!`, attribution in `testdata/README.md`). Then **M1**:
  `core::styles::resolve` + `ThemeStore` — but FIRST the
  `EXPORT-PROPERTIES.md` catalog (P2-D8) + P2-V1..V6 oracle verifications.
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — ask the user before any push (also all P1T commits sit unpushed).
  Consider registering the 9001 Collector-wiring decision.
- **Don't re-derive (trust):** P2-D1..D11 + risks (`build/PHASE_2.md`, read
  fully before M0 code); P1T-D1..D9 + landed blocks (`build/PHASE_1T.md`);
  `testing/e2e-suite-operations.md`; `pitfalls/dsl-style-color-normalization.md`;
  `pitfalls/fmt-line-end-guards.md`; NO_COLOR stderr has ANSI; fuzz
  mingw-CC-strip (`env/fuzz-host-toolchain.md`); oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (`env/reference-oracle.md`, slow JVM — one-shot
  `export --workspace F --format json --output DIR`; P2 goldens use
  `--format plantuml/...` later).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
