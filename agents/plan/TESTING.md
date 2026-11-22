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
  allow-list). Schema v3 (§P4T T-3) adds the Phase-4 axes: `inspect`
  (violations at the CLI default threshold `info`), `query` (matches of the
  fixed smoke filter `element.tag==Element`) and `diff` (self-diff findings —
  the identity axis, 0 for every loading input) over the entry's OWN input,
  plus `generate` (`system-landscape` over the ENTRY DIRECTORY — the command
  fans over directories and the sidecar never matches `DEFAULT_INCLUDE`;
  the blessed `views: 0` pins the witnessed view-free merge and load failures
  feed OXZ-5010 into the coverage gate). Corpus tooling: `cargo xtask e2e bless` (re-record sidecars;
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

## Feature × suite matrix (P4T-D1 — accountability document)

Every feature through Phase 4 (P4T additions included) carries ≥ 1 NAMED
test in every suite that fits it; `—` = the suite does not apply, `T-n` =
a gap resolved by that `build/PHASE_4T.md` item. Cells are test names
harvested from the tree, never memory — refresh with
`cargo test --workspace -- --list > LIST 2>&1` and join `Running` lines to
`…: test` lines (stderr carries the targets — `pitfalls/bench-machine-
drift-ab.md` style footgun). Maintenance rule (D0): a commit adding a
feature row adds its matrix row; one representative per cell — the matrix
does not duplicate coverage. Gap legend: **T-2** importer golden file
suite · **T-3** e2e corpus Phase-4 axes · **T-4** LSP capability
transcripts + stress · **T-5** fuzz targets `import_sniff`/`query_parse`/
`pathfilter`/`diff_pair` · **T-6** CLI audit row + Category B extension.

| Feature | unit | golden (Cat A) | CLI | e2e quick | e2e full | Cat B oracle | fuzz |
|---|---|---|---|---|---|---|---|
| `diff` (M0) | `diff::tests::consistent_id_shift_is_id_changed_never_add_remove` | `diff_probes::edited_pair_human_snapshot` | `cli::diff_human_report_exit_one_golden` | `quick::corpus_matches_manifests` (self-diff identity axis) | — | — (no reference `diff`) | `fuzz/diff_pair` (self-diff-empty over any loadable bytes) |
| `inspect` (M1) | `severity::tests::ranking_orders_error_above_ignore` | `inspect_model::element_ladder_run_order_snapshot` | `cli::inspect_human_report_exit_one_golden` | `quick::corpus_matches_manifests` (violations axis) | — | — (advisory, P4-D7) | — |
| diagram importers C4/Mermaid/image (M2) | `c4plantuml::tests::context_diagram_end_to_end`, `mermaid::tests::sequence_document_is_a_standalone_diagram`, `image::tests::png_imports_as_data_uri_image_view` | `golden_files::c4plantuml_context_file_golden` (+8 more, `tests/golden_files.rs`) | `cli::import_c4plantuml_sniffed_extension_stdout` | — | — | — (semantic bar F12; witness legs at M2) | `fuzz/import_sniff` (c4puml+mermaid lossy) |
| `!docs`/`!adrs` (M2 N-6) | `docs::tests::default_import_walks_top_level_sorted_with_order_and_trim`, `decisions::tests::adrtools_parses_id_title_date_status` | `p4m2_docs_adrs::docs_import_success_with_holders_exclude_and_images` | — | — | — | witness legs §P4-M2 (`TZ=UTC`) | — |
| WSD export (M2 N-5) | `wsd::tests::async_and_response_legs_pick_the_arrow_and_swap` | `export_wsd_goldens::wsd_goldens_byte_parity` | T-6 | `quick::corpus_matches_manifests` (wsd export section) | `full_export::export_is_deterministic_json_parity_and_fmt_parity` | oracle 9/9 byte-parity (M2) | — |
| `query` (M3 O-1) | `query::tests::view_subset_combinators_drop_extra_arms_like_the_reference` | `query_probes::element_ids_only_are_reproducible_from_the_json_twin` | `cli::query_table_rows_golden` | `quick::corpus_matches_manifests` (smoke-filter axis) | — | — | `fuzz/query_parse` (fixture ws + fuzzed filter) |
| `init` (M3 O-2) | `init::tests::workspace_templates_parse_clean` | `init_probes::scaffolded_workspaces_validate_and_match_the_json_golden` | `cli::init_creates_then_skips_then_forces` | — | — | — | — |
| `completions` (M3 O-3) | — | — | `cli::completions_bash_and_zsh_scripts_are_valid` | — | — | — | — |
| `generate system-landscape` (M3 O-4) | `generate::tests::landscape_shape_and_merge_semantics`, `generate::pathfilter::tests::unsupported_constructs_are_errors_not_silent_widening` | `generate_probes::landscape_matches_the_witnessed_merge` | `cli::generate_system_landscape_merges_into_the_output_file` | `quick::corpus_matches_manifests` (generate section — covers OXZ-5010, shrank the allow-list) | `full::generate_pair_merge_is_view_free_and_id_unique` | `cargo xtask oracle landscape` | `fuzz/pathfilter` (compile + fixed subjects) |
| `oracle` user command (M5a E-1) | `oracle::tests::removes_last_entry_and_fixes_comma` | `oracle_cli::gen_is_idempotent_and_verify_then_matches` (fake oracle) | `oracle_cli::verify_mismatch_and_missing_exit_one` | — | — | `oxidizr oracle verify` (real binary) | — |
| LSP (§P4-LSP) | `completion::tests::position_maps_back_through_utf16`, `hover::tests::identifier_wins_over_name_and_keyword`, `formatting::tests::range_widens_to_the_whole_statement`, `config::tests::api_options_ride_the_pipeline` | `transcript::handshake_transcript` (+4: `completion_transcript`, `hover_transcript`, `formatting_edit_idempotence_transcript`, `fast_edit_burst_converges`) | `version::version_flags_print_name_and_version` | — | — | — | — |
| `Layered` channel lanes (M5c E-3) | `layered::tests::channel_lanes_split_an_overlapping_bundle`, `layered::tests::star_concentration_gets_distinct_lanes`, `layered::tests::thousand_nodes_are_deterministic` | — (engine coords in no golden — E-3 stamp) | — | — | — | — | — |
| export-dialect comparators (M5b E-2) | `exportcmp::tests::mermaid_reordering_is_equal`, `exportcmp::tests::plain_plantuml_reordering_is_equal_labels_are_pinned` | `plantuml_goldens_byte_parity`, `mermaid_goldens_byte_parity` | — | — | — | `cargo xtask export-verify --format c4\|plantuml\|mermaid` | — |
| OXZ-code coverage gate (T1b) | `coverage::registry_matches_docs_table`, `coverage::allowlist_has_no_stale_entries` | — | — | — | — | — | — |
| CLI audit (all subcommands) | — | — | T-6 (≥1 stdout golden + ≥1 error-path exit test per `Command`) | — | — | — | — |
| core pipeline (P1–P3, reference row) | `run` of `oxidizr-dsl` statement engine (`engine::stmt_model::tests::element_grammar_errors`) | `m1_lossless::corpus_cst_is_lossless`, `golden_m3::m3_probe_workspace_writes_golden_bytes` | `cli::parse_errors_exit_1_with_reference_text_and_code` | `quick::corpus_matches_manifests` | `full::json_round_trip_is_byte_stable` | `cargo xtask oracle verify` | `dsl_parse`, `dsl_fmt`, `json_read` |

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
