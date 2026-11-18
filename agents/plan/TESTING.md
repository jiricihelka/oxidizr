# TESTING.md — test architecture & benchmarks

Two-category discipline, the golden corpus policy, and bench rules. What the corpus
actually contains: `testdata/README.md`. Harness pattern: `agents/notes/testing/`.

## Categories (every test belongs to exactly one)

- **Category A — pure Rust** (default `cargo test --workspace`): unit, integration,
  snapshot (`expect-test`), golden byte-diffs against `testdata/golden/`, property
  tests (`proptest`), read-corpus loads. Must never need network, Java or Docker.
  Runs on the 3-OS CI matrix.
- **Category B — reference-backed**: needs the official `structurizr` binary (golden
  (re)generation via `cargo xtask oracle`, oracle verification, behavioral
  tie-breakers). The reference tool is **slow — use it minimally**: these tests are
  compiled and run only behind a default-OFF `reference` feature, and goldens are
  committed so per-PR CI never invokes the binary. The binary is located via
  `$OXIDZR_STRUCTURIZR_BIN`, falling back to `structurizr` on PATH (the nix dev flake
  provides it); when absent these tests **skip with a loud summary line** — except in
  the dedicated CI job (feature enabled), where a skip is a failure (coverage may not
  silently vanish).

Use all test kinds where they pay off: unit tests for rules (tags, id counter,
canonical names, validation), snapshot tests for lexer/parser shapes and diagnostics,
golden byte-diffs for the JSON contract, property tests for invariants (fmt
idempotence, parse↔print, round-trips), fuzzing (`cargo-fuzz`, nightly) on the
lexer/parser against the fixture corpus, and `cargo-llvm-cov` in CI as a **report
only** — no percentage gate. Dev-only deps: `proptest`/`arbitrary`, `criterion`,
`cargo-fuzz`; no mutation testing (poor ROI here).

## Test data & goldens

- `testdata/` holds vendored `.dsl` fixtures (reference test resources in
  `structurizr-dsl/src/test/resources` and the golden workspaces in
  `structurizr-export`/`structurizr-client` tests, source + Apache-2.0 attribution
  kept in `testdata/README.md`) and their golden `.json` files. The corpus is a byte
  contract — `.gitattributes` pins EOL (`agents/notes/testing/byte-exact-corpus.md`).
- Goldens are generated with the official unified application binary:
  `structurizr export --workspace <file> --format json --output <dir>` — GNU-style
  long options only (the unified binary rejects the old CLI's single-dash `-format`
  form; verified against 2026.10.26; the old `structurizr-cli` is archived).
  Driver: `cargo xtask oracle gen|verify`. Record the tool version in
  `testdata/README.md`. Goldens are **committed**; regenerate sparingly (fixture
  changes, milestones) — per-PR CI never calls the reference tool.
- Round-trip tests: JSON → model → JSON must be lossless for the fields Structurizr
  itself preserves (see DEVIATIONS.md D3 for the reference-exact default).

## E2E suite (`oxidizr-e2e`, Phase 1T interlude; export matrix at 2T)

End-to-end regression over a purpose-built corpus in `e2e/corpus/` — entries assert
**use-cases** (exit class + OXZ code set + shape counts), never bytes, never message
text; the JSON byte contract stays with the Category-A goldens. In-process via
`oxidizr-api::execute` (subprocess smoke anchors the real CLI once — it resolves
`oxidizr` **and** `oxidizr.exe` so the Windows CI leg works); entries fan out on
`thread::scope`. Plans and all decisions: `build/PHASE_1T.md`, `build/PHASE_2T.md`.

- **Quick tier** (rides `cargo test --workspace`, every PR): blessed sidecars per
  entry (`workspace.e2e.json`: exit class, codes, counts) for
  `dsl2json`/`json2json`/`validate`/`reconcile`/`fmt` **plus the export matrix**
  (P2T-D2): `export.{plantuml,plantuml-c4,mermaid,websequencediagrams}` blocks
  (exit, codes, files written) whenever the entry's input parses with ≥ 1 view —
  fan-out through `Request::Export` into per-entry scratch dirs, hermetic options
  (`allow_net` false; theme fixtures ride a `themes/` dir inside the entry via the
  reference `<workspace-parent>/themes` source). Also permanent: `path_leakage`
  (V-T7 — export bytes never embed the scratch path) and the manifest coverage
  gate (≥150 valid / ≥120 invalid, code coverage floor with a shrinking
  allow-list). Corpus tooling: `cargo xtask e2e bless` (re-record sidecars;
  byte-stability is asserted, goldens untouched) and `cargo xtask e2e mutate`
  (seeded mutation generator — REPLACES `corpus/mutated/` wholesale, always
  re-bless after; `agents/notes/pitfalls/t3-invalid-corpus-probes.md`).
- **Full tier** (nightly `.github/workflows/e2e.yml`, local:
  `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`): self-consistency invariants
  over every valid entry — parse determinism, `dsl2json`→`json2json` byte
  **fixpoint** (`out₃ == out₂`; the first pass may re-sort per the reader's
  `TreeSet` rebuild — WRITE-CATALOG §5), `fmt` fixpoint + `parse(fmt(x)) ≡
  parse(x)` (scratch-dir replay so relative `!include`/`extends` resolve),
  self-`reconcile` identity; and the 2T export invariants (`--test full_export`):
  export determinism, **path parity** `export(dsl) ≡ export(json2json(dsl))` and
  `export(fmt(x)) ≡ export(x)` per output file (replays from a scratch copy of
  the entry dir keep the theme chain + children honest). Env-gated instead of
  `#[ignore]` (V-T4: an env gate can never drag in future ignores). The nets
  caught two real engine bugs at P1T first run (P1T-D6 landed block) and two
  more pre-P2 regressions at 2T (PHASE_2T.md catch report).
- **Benches**: twelve `e2e_` corpus benches since 2T (parse/round-trip/fmt/
  validate/reconcile + export × 3 dialects, whole-corpus and biggest-entry),
  feature-gated, local-only — §Benchmarks.

## Benchmarks

`criterion` benches live in `benches/` per crate; mandated targets: DSL parse (full
fixture corpus + worst-case per feature), JSON read, JSON write, `fmt`, `reconcile`.
Baselines are committed at milestones M0/M3/M4/M7; compare with `critcmp`. An
optimization is accepted only with before/after numbers; >~10% regression against the
committed baseline is flagged by the CI bench job for human review (no auto-fail).
Run via `cargo xtask bench` — never forward args into `cargo bench --workspace --`
(`agents/notes/pitfalls/cargo-bench-args.md`).

**1T corpus benches** (`oxidizr-e2e/benches/e2e.rs`, P1T-D7): whole-corpus
`e2e_dsl2json_all` (+ biggest-entry worst case), `e2e_json2json_all`, `e2e_fmt_all`,
`e2e_validate_all` and a realistic `e2e_reconcile` pair, measured in-process through
`oxidizr-api::execute` over the e2e corpus (the realistic workspaces, not the
vendored fixtures). They are gated by cargo feature `e2e-bench` (`required-features`
on the bench target), so CI's `cargo bench --workspace` lists-but-skips them — they
are local-dev numbers (user decision: no new CI load). `cargo xtask bench` always
opts in (`--features oxidizr-e2e/e2e-bench`); the `1T` baseline under
`bench-baseline/` includes them and `cargo xtask bench cmp 1T` compares.
