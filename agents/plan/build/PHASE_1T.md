# PHASE_1T.md — detailed plan: e2e regression & benchmark suite (interlude)

**Interlude phase between Phase 1 (v0.1.0) and Phase 2.** Product intent (user,
2026-11-17): Phases 1–4 build the exact Structurizr replica; *after* that we
modify and improve parts of the system (layout, architecture — today shaped like
the reference). This phase builds the regression net that makes those changes
safe: many close-to-real-world DSL workspaces, asserted as **use-cases** — valid
DSL compiles into the expected shape, invalid DSL errors with the expected code
set — NOT bytes (the `testdata/` goldens already gate the byte contract) and NOT
messages (those are explicitly allowed to change later). Plus benchmarks over
the same corpus, local-first, so perf regressions of refactors are visible.

User decisions recorded 2026-11-17 (questions tool): corpus = originals +
mutations first, licensed public DSLs later per fixture; assertions = exit class
+ OXZ code set, blessable; the suite MUST run in-process via a designed stable
e2e API (hundreds of DSLs × multiple tests each — "tens of thousands of
processes" is not an option) and parallelize; CI = quick per-PR (< ~2 min),
full nightly; benches are NOT run in CI (local dev tool); API lives in a new
`oxidizr-api` crate under the CLI; corpus floor ≈150 valid + ≈120 invalid; no
tag when the phase closes (no shipped-behavior change).

**Not goals.** No new user-facing command; no behavior change (T0 is proven
byte-neutral by the existing suite); `testdata/` and the Category-A/B byte
gates stay exactly as they are; this suite does not replace them — it covers
what they *cannot* (a future intentional deviation re-blesses 1T manifests,
while the golden gates deliberately keep failing until the contract docs move).

---

## P1T-D1 `oxidizr-api` crate — the stable e2e seam (keystone)

```
crates/
  oxidizr-api    headless pipeline: Request -> execute() -> Outcome; deps: core, dsl, json, diag
  oxidizr-cli    argv (clap), miette rendering, suggestions, exit-code shim; deps: api (+diag)
  oxidizr-e2e    test+bench harness over e2e/corpus (publish = false); deps: api
```

- Today `oxidizr-cli::run()` reads argv, builds a `Renderer`, and command fns
  print + return `i32`. T0 extracts the command bodies (`dsl2json`,
  `json2json`, `validate`, `fmt_cmd`, the `reconcile` module, UTF-8 `decode`,
  the production `json_loader` injection) into `oxidizr-api` as **values in /
  values out**; `run()` becomes: parse argv → `Request` → `execute` → render →
  `i32`. `run()` adds no logic — the invariant that keeps binary and suite in
  lockstep.
- Shape (draft, refined in T0; extension by addition only):

  ```rust
  pub enum ExitClass { Ok, Diagnostics, Io }        // 0 / 1 / 3; usage(2) never leaves clap
  pub struct Diagnostic { pub code: Code, pub severity: Severity, /* message access for humans */ }
  pub enum Input<'a> { File(&'a Path), Bytes(&'a [u8]) }   // Bytes = stdin semantics, no file context
  pub struct ApiOptions { pub strict: bool, pub keep_unknown: bool, pub env: BTreeMap<String, String> }
  pub enum Request<'a> { Dsl2Json(Input), Json2Json(Input), Validate(Vec<Input>),
                         Fmt { files: Vec<Input>, mode: FmtMode }, Reconcile { dsl: Input, layout: Input } }
  pub struct Outcome { pub exit: ExitClass, pub diagnostics: Vec<Diagnostic>,
                       pub bytes: Vec<u8> /* emitted JSON / formatted text; per-file for fmt */ }
  pub fn execute(req: Request, opts: &ApiOptions) -> Outcome;
  ```

- **Totality + purity contract** (this is what "stable e2e API" means): never
  panics, never reads process env/cwd/global state, no rendering, no stdout;
  env-var substitution takes the map explicitly (CLI passes the process env);
  `FmtMode::Write` is the only side-effecting mode (file rewrite, as today) and
  the e2e suite never uses it — it uses in-memory text. Thread-safe: everything
  `Send + Sync`, per-call state only (id counter lives in the builder — verify
  V-T1).
- Suggestions (`suggest.rs`) live in the API too (refinement below): they
  decorate `Issue::help` only (P1-D2), and the command bodies that apply them
  are API-side; miette stays the CLI's sole rendering concern.
  `Outcome` carries codes/severities, the CLI renders. Message strings stay
  accessible on `Issue` for humans — the e2e layer is simply forbidden
  from asserting them.
- Registered as `ADDITIONAL_FEATURES.md` **F9** (dev-facing; the CLI lib stays
  `publish = false`; parity paths unchanged per that file's rule 1).
- **T0 proof**: all existing tests (22 CLI e2e byte-goldens included) pass
  unmodified against the refactored code. Any golden drift = the refactor is
  wrong, never "re-bless the golden".
- **T0 landed 2026-11-17** (`5eedf99`…`2d2e159`) — refinements vs the draft
  sketch above, now authoritative for T1+:
  - `ExitClass` gained **`Usage`** (code 2): `fmt --write` on a stdin-semantics
    input is decided by the pipeline, not clap — the draft assumed usage(2)
    never leaves clap.
  - The `Outcome` shape is `exit + log: Vec<LogItem> + bytes: Option<Vec<u8>>`
    with `LogItem = Issue | OkLine | NoteLine` (ordered replay), not a flat
    `diagnostics` vec — commands interleave diagnostics, stdout chatter and
    stderr reports (validate, reconcile, fmt), and the byte contract pins the
    order. The CLI writes `bytes` to stdout first; a stdout write failure is
    the old `write_output` exit 3 and **suppresses the log** (the extracted
    reconcile code returned before its notes).
  - `Input = File | Bytes | Stdin` with **lazy** stdin via an injected
    `StdinReader` fn — eager reading in the argv shim would hang `fmt --write -`
    on an open pipe; the usage exit must fire before stdin is touched.
    `Bytes` carries stdin semantics (no engine-visible file context).
  - `ApiOptions.env` is `Option<BTreeMap>` (`None` = V-3 process env, the
    production CLI); the map rides into the engine as `EnvLookup`, since T0-d
    an **Arc newtype** (`Send + Sync`, captured map) — which made
    `ParseOptions` Clone-not-Copy.
  - Suggestions moved **into the API** (module `oxidizr_api::suggest`, CLI
    re-exports): help-line construction is diagnosis construction, and it
    happens where the parse results are — the draft's "stay CLI-side" was
    superseded by the T0-c extraction.

## P1T-D2 Corpus `e2e/corpus/` — one collection, tests + benches share it

- `e2e/` is a NEW corpus root, NOT `testdata/`: `testdata/` is the vendored
  byte-contract ("do not edit by hand", `.gitattributes`-pinned) — authored,
  blessable fixtures would corrupt that contract. `e2e/` is ours: freely
  editable, reviewed like code.
- Layout: `e2e/corpus/valid/<nn-feature>-<name>/workspace.dsl (+ include/extends
  children)`; `e2e/corpus/invalid/<nn-class>-<name>/…`;
  `e2e/corpus/mutated/…` (generated, COMMITTED — hermetic + diff-reviewable).
  Entry = the directory's `workspace.dsl`; every entry is run through all
  commands its kind allows + the benches.
- Feature axes for authored originals (≈150 valid): per P1 feature — model
  (people/systems/containers/components/groups), relationships (+bounded
  context, `->` dynamic pairs), deployment (nodes/instances/AWS forms), views
  (all 8 view types + filtered), styles + archetypes-ish properties,
  properties + `!ref`s, `workspace extends` (dsl + json), `!include` trees
  (multi-dir), `${VAR}` substitution, quoting/comment/encoding edges that are
  *legal*, big realistic multi-file composites (big-bank-plc scale). Depth over
  cleverness: realistic hand-written style is the point (user: "close to
  real-world examples").
- Provenance: originals (repo dual-license) + mutations from day one; public
  real-world workspaces ONLY later, per fixture, with license + attribution
  recorded in `e2e/PROVENANCE.md` (Apache-2.0/MIT/BSD-compatible else refused).
  Rejected for now: vendoring bulk third-party DSL (license review cost; frozen
  files must be forked to mutate anyway); re-asserting `testdata/dsl` here
  (duplicates the m4 burn-in and would fight its message-prefix register).

## P1T-D3 Manifests + `cargo xtask e2e bless`

- One sidecar per entry: `workspace.e2e.json` (serde_json is already vendored —
  no new dep), shape (draft):

  ```json
  { "dsl2json":  { "exit": "ok",    "elements": 12, "relationships": 9, "views": 4 },
    "validate":  { "exit": "error", "codes": [4002] },
    "fmt":       { "exit": "ok",    "idempotent": true } }
  ```

- Asserted: **exit class, sorted-deduped OXZ code set, structural counts**
  (elements incl. nested containers/components, relationships, views, styles;
  names/keys only where the fixture is about them). NEVER bytes, NEVER message
  text (user rule). Volatile exclusions: `structurizr.dsl.identifier` UUIDs,
  `lastModified*` (they never enter counts).
- `cargo xtask e2e bless [FILTER]` regenerates sidecars; the git diff IS the
  behavior-change review artifact (intentional deviation in a later phase =
  review + re-bless, tests don't rot as obstacles). Runner checks BOTH ways
  (m4_fixtures precedent generalized): unregistered drift fails; stale sidecars
  (fixture no longer errors / command list changed) fail.
- Sidecars are dev data, not shipped, not byte-frozen.

## P1T-D4 Invalid fixtures + error-code coverage gate

- Two sources: hand-authored per diagnostic class (grammar, unknown name
  reference, duplicate names, view-membership gaps, bad `extends`/`!include`
  targets, encoding failures, …) and **mutations** of valid originals.
- `cargo xtask e2e mutate [SEED]`: deterministic seeded std-only PRNG, ops each
  targeting a class — delete a `}` (OXZ-2003), drop a required token (OXZ-2001),
  rename one side of a relationship (dangling reference), duplicate a
  top-level name, break `${` … same seed ⇒ same bytes (V-T3). Output committed
  under `corpus/mutated/` with generated sidecars (blessed once, reviewed).
- **Coverage gate** (`#[test]`): every `Code` const the CLI can emit appears in
  ≥1 manifest; an explicit allow-list registers genuinely CLI-unreachable codes
  (with a reason). The catalog — not strings — is the contract.

## P1T-D5 Runner crate `oxidizr-e2e` + tiers (all Category A, in-process)

- `crates/oxidizr-e2e` (workspace member, `publish = false`): ordinary
  `#[test]`s calling `oxidizr_api::execute` in-process; fixtures fanned out
  with `std::thread::scope` (std-only; no rayon). Hundreds of entries × a few
  commands ≈ sub-second of real work — the "tens of thousands of processes"
  trap is structurally avoided (user requirement).
- **Quick tier** = default `cargo test --workspace` (rides the existing PR
  job — zero new CI): every entry, exit class + code sets + counts vs manifests,
  coverage gate. Budget < 60 s wall (V-T2 measures).
- **Full tier** = invariants (P1T-D6) gated by env `OXIDZR_E2E_FULL=1` —
  env-var gate, NOT `#[ignore]` (the repo has ignored expect-test gaps;
  `--include-ignored` would drag them in — inventory confirmed at T4, V-T4).
  Nightly `e2e.yml` (workflow_dispatch + schedule; kept out of `ci.yml` so its
  weekly Category-B schedule stays weekly — fuzz.yml precedent) exports the env
  and runs `cargo test --workspace`.
- Subprocess stays a SPARSE smoke (~6 entries via `CARGO_BIN_EXE_oxidizr`):
  exit class + non-empty output only — anchors "the binary wires the API the
  way tests exercise it" (bytes stay owned by `crates/oxidizr-cli/tests/cli.rs`).

## P1T-D6 Invariants (full tier) — self-consistency, immune to deviation

Per valid entry, everything checks the tool against ITSELF (a later intentional
change moves both sides together — that is exactly what makes this net survive
divergence): parse determinism (two runs → same counts + same codes);
`dsl2json` → `json2json` byte-stability (output₂ == output₁ — internal
consistency, not parity); `fmt` idempotence + `parse(fmt(x)) ≡ parse(x)`
(established M6-b technique: UUID line filter); self-`reconcile` (layout =
own JSON export ⇒ zero ambiguity, zero element graft).

## P1T-D7 Benchmarks — same corpus, local-first

- New criterion targets in `oxidizr-e2e/benches/` (`e2e_` prefix): whole-corpus
  validate/`dsl2json`/`fmt`/`json2json` over the REALISTIC workspaces +
  biggest-entry worst cases + `e2e_reconcile` realistic pair. M7's testdata-
  based targets stay (they anchor the committed M0/M7 baselines).
- Heavy corpus benches are gated by cargo feature `e2e-bench` on the
  `[[bench]]` target: CI's `cargo bench --workspace` (existing bench job)
  SKIPS them (user: benches are local dev, no new CI load); `cargo xtask bench`
  passes `--features e2e-bench` so the local mirror + `bench save <LABEL>`
  always include them. First snapshot: `cargo xtask bench save 1T`.
  Compare `cargo xtask bench cmp 1T` (critcmp). No auto-fail policy anywhere —
  TESTING.md §Benchmarks gains the 1T-corpus paragraph at T6.

## P1T-D8 Milestones (ordered, each ends green — quartet + commit + RESUME)

| MS | Deliverable | Exit gate |
|---|---|---|
| **T0** | **DONE 2026-11-17** (`5eedf99`/`52a83c1`/`075cb90`/`2d2e159`) — `oxidizr-api` extraction; `run()` shim; F9 registered; crate docs; refinements recorded in D1 | quartet green; **all existing tests unmodified** (byte-proof); V-T1 closed |
| **T1** | `oxidizr-e2e` crate; manifest reader/writer + compare; `xtask e2e bless`; 10 seed entries (valid+invalid, hand-authored); subprocess smoke; coverage test skeleton | bless round-trips byte-stably; quick tier green + measured runtime |
| **T2** | valid corpus to ≥150 entries across the D2 axes (multi-file composites incl.) | all manifests green; V-T2 re-measured (< 60 s) |
| **T3** | invalid corpus to ≥120 total + `xtask e2e mutate`; every emitted `Code` covered (allow-list registered with reasons) | coverage gate green; mutate determinism V-T3 closed |
| **T4** | invariant tier behind `OXIDZR_E2E_FULL` + `e2e.yml` nightly + V-T4 ignore inventory | full tier green locally + in a dispatched run |
| **T5** | `e2e_` benches (+ `e2e-bench` feature gate), `1T` baseline saved, TESTING.md §Benchmarks + WORKFLOW.md command table + `cargo xtask --help` updated | baseline committed; `bench cmp 1T` works; CI bench job still passes WITHOUT the feature |
| **T6** | close-out: add an e2e-suite section to TESTING.md, PROGRESS board+log, PHASE_1T DONE entries, notes, README.md (root) dev-section mention | quartet + actionlint green; **no tag** (user decision) |

Size-gate honored: >4 modules (cli, new api + e2e crates, xtask, corpus, docs)
— this file is the decomposition.

## P1T-D9 Verification list (empirical, before the code that depends on it)

- **V-T1** (before T0) — engine purity for parallel in-process use: no global
  state reachable from `parse_dsl`/`from_slice` (builder id counter per-call);
  a 4-thread stress parse of the M7 corpus is deterministic.
- **V-T2** (T1) — quick-tier wall time on the dev box for 10 entries; the
  ×25 extrapolation must land < 60 s, else batch manifests per directory.
- **V-T3** (T3) — `xtask e2e mutate 7` twice ⇒ byte-identical corpus.
- **V-T4** (T4) — inventory today's `#[ignore]`d tests (expect-test gaps seen
  in M7 runs); confirm the env-gate choice beats `--include-ignored`.
- **V-T5** (T5) — CI bench job (`cargo bench --workspace`, no feature) still
  green and FAST: feature-gated benches are listed-but-skipped, not compiled.

## Risks

- **API drift vs binary** (tests green, users broken): mitigated by the `run()`
  = shim invariant + subprocess smoke + the untouched 22 byte-e2e tests.
- **Mutations produce the "wrong" code** (one edit, two diagnostics): manifests
  record the FULL sorted set; bless review is the review step — no guessing.
- **Bless fatigue** (rubber-stamping diffs): manifests assert use-case shape,
  so a legit refactor blesses with tiny/no diffs; a diff touching exit class or
  counts of unrelated fixtures is the smell the reviewer must catch — record
  the review rule in the T6 note.
- **Corpus stalls "close to real-world"**: originals get PR-reviewed like code;
  the feature-axis table in D2 is the checklist; public DSLs can be added
  per-fixture later without schema change (provenance file is append-only).
