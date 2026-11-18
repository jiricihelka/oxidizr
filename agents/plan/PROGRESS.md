# PROGRESS.md — implementation work journal

Living notes for the implementer (survives context loss). Newest entries on top.
Authority order: `DEVIATIONS.md` (contract + Locked) > `build/PHASE_N.md` > this file
(journal/notes only — decisions belong in the phase files).

## Status board

| Phase | Milestone | State |
|---|---|---|
| P1 | M0 scaffold | **done 2026-11-14** |
| P1 | M1 ast+lexer | **done 2026-11-14** |
| P1 | M2 core model + validation | **done 2026-11-14** |
| P1 | M3a write catalog (P1-D6) | **done 2026-11-14** (commit 7848c5e) |
| P1 | M3b json writer + reader | **done 2026-11-14** (commit 0e77dc2) |
| P1 | M4 dsl parser + builder | **done 2026-11-15** (commit 41b70ca) |
| P1 | M5 workspace extends | **done 2026-11-16** (commits 9f174e6, c468854) |
| P1 | M6-a CLI surface | **done 2026-11-16** (log below) |
| P1 | M6-b formatter | **done 2026-11-16** (log below) |
| P1 | M6-c reconcile + validate polish | **done 2026-11-16** (commit 7351db1 + log below) |
| P1 | M6-d fuzz + CI nightlies | **done 2026-11-16** (log below) |
| P1 | M7 | **done 2026-11-16** (log below) — **Phase 1 complete, tagged v0.1.0 (local)** |
| P1T | e2e suite (interlude) | **COMPLETE 2026-11-17** — T0–T6 all done: API + harness + 300-entry corpus + full-tier invariants + `e2e_` benches + `1T` baseline + docs. Net caught 2 real bugs (oracle-fixed). No tag (user decision). Log below (`build/PHASE_1T.md`) |
| P2 | M0 export scaffold + theme/json dispatch | **done 2026-11-17** (commits 6f6cc64, ca292e7) — log below |
| P2 | M1 theme infrastructure | **COMPLETE 2026-11-17** (a `199bc70`–e-B `53d2e5e`) — log below |
| P2 | M2 Structurizr-PlantUML | **COMPLETE 2026-11-17** — M2-a (catalog + `export_property` + D12), M2-b (engine), M2-c (dialect), M2-d (dispatch + oracle A/B fix batch + 30-fixture plantuml golden corpus). Log below (`build/PHASE_2.md`) |
| P2 | M3 C4-PlantUML | **COMPLETE 2026-11-17** — M3-a (catalog `build/EXPORT-C4-PLANTUML.md` + witness sweep), M3-b (`plantuml::c4` dialect port, 2706209), M3-c (A/B: 30 SAME / 4 design-DIFF / 10 vacuous), M3-d (29-fixture c4 goldens + oracle verify green + P2-D9 comparator `xtask export-verify`). Log below |
| P2 | M4 Mermaid + theme goldens + CI render + bench | **M4 COMPLETE 2026-11-18** (M4-d: goldens + oracle legs + golden tests + render smokes + bench). Log below |
| P2 | M5 hardening + v0.2.0 | **COMPLETE 2026-11-18** — proptest export panic-suite (`export_never_panics.rs`), `oxidizr export --help`/README/diagnostics refresh, P2-V5/V6 verification items closed, full `oracle verify` green (4 json + 90 diagram + 6 theme), `bench save M4`. **Phase 2 complete, tagged v0.2.0 (local)** |
| P3 | viewer+layout | pending |

Commit cadence (user decision revised 2026-11-15, after M4): **commit every green
sub-piece**, wrapped up by a milestone commit (PROGRESS log + status board); keep
`agents/notes/RESUME.md` current at each commit; milestones touching >4 modules are
decomposed in the PHASE doc first. Questions only on plan-contradicting reality
(AGENTS.md working style).

## Environment facts (verified 2026-11-14)

- `cargo`/`rustc` on PATH = flake's pinned nightly: `cargo 1.102.0-nightly (ede6cefea
  2026-11-11)`, `rustc 1.102.0-nightly (8eb778d04 2026-11-12)`. The session PATH already
  carries the dev shell (rustc, cargo, structurizr, java-less oracle wrapper all work).
- Oracle on PATH: `structurizr` → nix-wrapped war 2026.10.26 (library 6.2.4).
  Invocation contract (verified): `structurizr export --workspace F --format json
  --output DIR` (GNU options only; `-o` is a directory; output file = input basename +
  `.json` inside it). Version line: `structurizr: 2026.10.26 / structurizr-*: 6.2.4`.
- Reference clone (Apache-2.0, pinned `ad34036`): `/tmp/nix-shell.uni100/opencode/structurizr`.
  DSL fixtures live in `structurizr-dsl/src/test/resources/dsl/` (84 top-level `.dsl` +
  subdirs `extend/`, `image-views/`, `decisions/`, `docs/`, `big-bank-plc/`,
  `spring-petclinic/`, `include/`… 113 `.dsl` in repo total — vendor only the dsl dir +
  client read-corpus + validation, see P1-D10).
  Read-corpus: `structurizr-client/src/test/resources/` (`backwardsCompatibility/`,
  `structurizr-36141-workspace.json`…). Validation fixtures: same resources dir
  `workspaceValidation/` (16 JSON).
- V-10 fixture (no views): `workspace-properties.dsl`.
- Reference download host works: `download.structurizr.com` (static.structurizr.com does
  NOT resolve — never use in docs).
- git remote: `git@github.com:jiricihelka/oxidizr.git` (nothing pushed yet as of M0).
- python3 is NOT on PATH; use
  `/nix/store/9zclbh8196695bbjs106wh8c4957n002-python3-3.13.12-env/bin/python3`.

## Working notes / errata found while implementing

- **Repo reorganization (2026-11-15, post-M4)**: all permanent agent files moved under
  `agents/` — `plan/` → `agents/plan/` (git mv, every `plan/` cross-ref in 30+ files
  rewritten), plus a new `agents/notes/` tree of short operational-recall notes
  (env / pitfalls / testing / process, README index per directory) to survive context
  compressions during long milestones. `agents/plan/reports/` (user-dropped CI/OS
  reports) stayed untracked; it was briefly lost when `git mv` removed the emptied
  `plan/` dir and was restored byte-identical from `~/.local/share/Trash` — lesson
  recorded in `agents/notes/pitfalls/git-mv-deletes-untracked.md`. Same day,
  follow-up: AGENTS.md slimmed to a per-task charter; permanent detail moved to
  `agents/plan/COMPAT.md` (contract + sources of truth), `agents/plan/WORKFLOW.md`
  (env, commands, CLI policy, quality/style) and `agents/plan/TESTING.md`
  (categories, goldens, benches); all `AGENTS.md §…` references across the repo
  repointed. Same day: M4 retrospective adopted three process rules — commit every
  green sub-piece (not one commit per milestone), the `agents/notes/RESUME.md`
  checkpoint ritual, and a milestone size gate (>~4 modules ⇒ decompose first).
  No code, JSON-contract, or Locked-list impact.
- (M0) Fixture path correction: `…/test/resources/dsl/…` (notes-*.md listed bare
  filenames) — vendor script must use the `dsl/` root.
- **V-10 RESOLVED (2026-11-14)** — reference app pipeline around every `export`:
  `AbstractCommand.loadWorkspace` = load → `WorkspaceScopeValidator.validate()` (hard
  error; fixtures failing validation can never have goldens) →
  `new DefaultInspector(workspace)` → `createDefaultViews()` if `!model.isEmpty() &&
  views.isEmpty()`.
  - Default views (V-10 proper): generated for **model non-empty + zero views**
    (`workspace-properties.dsl` has an empty model → none, which is why the first probe
    showed nothing). Shape per `structurizr-core/…/view/ViewSet.createDefaultViews()`
    (class renamed from `Views` in 6.2): landscape `addDefaultElements` + per system
    (sorted by name) context & container views + component view per container with
    components + deployment views per environment×system; empty key ⇒ auto-keyed
    (V-4 numbering). **`dsl2json` must mirror this** (app-parity is the oracle contract);
    keep the *library* generation-free (Locked list).
  - Inspection counters: `DefaultInspector` writes `structurizr.inspection.error/.
    warning/.info/.ignore` (violation counts) into workspace `properties` on EVERY
    load/export. Harness artifact, not file content; whole `structurizr-inspection`
    crate is out of scope to reimplement. Goldens strip these four lines
    (`cargo xtask` `normalize_reference_output`, incl. trailing-comma fixup when they
    sort last); json2json round-trips them as ordinary properties. DEVIATIONS-worthy —
    register there if user-visible behavior solidifies in M3/M4.
  - DSL parser defaults: workspace name/description default to the literal strings
    `"Name"` / `"Description"` (`WorkspaceParser.java:28`), overwritten by the
    `workspace` statement. Builder (M4) must start from these.
- **Properties map serialization = Java HashMap order (V-11, M3 blocker)**. Proof:
  workspace-properties export ⇒ `error, dsl, info, dslEditor, warning`; 36141 JSON→JSON
  ⇒ `info, ignore, error, warning, dsl`. Neither sorted nor insertion-ordered; order is a
  function of key hashes (+bucket chains). Plan correction: P1-D5's "BTreeMap sorts for
  free" is WRONG for byte parity of `properties` (any `properties` object: workspace,
  elements, relationships, views, …). POJO fields remain declaration-ordered (the
  golden's alphabetical top level is POJO field order, that part is fine). Options for
  M3 (decide there, recorded in WRITE-CATALOG): (a) replicate Java `String.hashCode` +
  HashMap bucket walk in a custom serializer, or (b) parity comparator canonicalizes
  key order of `properties` objects JSON-aware. (a) is compatible with byte-goldens;
  (b) weakens the gate. Empty object renders `{ }`, empty array `[ ]`, `"key" : value`.
- **Volatility RESOLVED — export is deterministic**: no timestamp is stamped by the app
  (earlier note about `lastModified` was wrong). `lastModifiedDate/. lastModifiedAgent`
  round-trip from JSON inputs verbatim; `lastModifiedUser: ""` omitted (empty-string
  omission); unknown keys (`revision` in cloud exports) dropped; compact input JSON is
  rewritten in Jackson pretty layout; numeric `workspace.id` preserved (36141 test).
  So read-corpus goldens need no normalization.
- `loadWorkspace` also `installThemes`(local `themes/` dir or `$STRUCTURIZR_THEMES`) —
  theme URLs are fetched during PNG/SVG export only, not `--format json`.
- **Windows CI failures — checkout EOL conversion RESOLVED (2026-11-15)**. Symptom
  (`plan/reports/windows_test_error.txt`, scratch file since deleted): the 6 byte-exact `expect!` CST snapshots in
  `crates/oxidizr-dsl/tests/m1_lossless.rs` failed on Windows with LF-vs-CRLF diffs and
  shifted byte offsets. Root cause is NOT the lexer (lossless corpus tests passed):
  stored blobs are LF, but the Windows checkout had `core.autocrlf=true`, which rewrites
  LF→CRLF at checkout. Fix: root `.gitattributes` — `* text=auto eol=lf` for sources,
  `testdata/** -text` (byte-exact in BOTH directions: fixtures/goldens/assets must never
  be EOL-converted, re-vendoring keeps reference bytes). `.gitattributes` outranks
  `core.autocrlf`, so checkouts are byte-stable on all 3 OSes. Verified by cloning with
  `-c core.autocrlf=true`: fixtures/goldens materialize CR-free, jar byte-identical,
  `m1_lossless` 9/9 green in that clone; `git add --renormalize` was a content no-op.
- **Local Windows leg via Wine (2026-11-15, dev-env decision)**. The dev flake now
  approximates the windows-latest CI leg on Linux: `cargo test --workspace --target
  x86_64-pc-windows-gnu` inside `nix develop` (shell sets the per-target `_RUNNER=wine`
  and a `_RUSTFLAGS=-L native=<libpthread stub>`). Verified: **259/259 green under
  Wine** at P1-M4. Three non-obvious pins, each commented in `flake.nix`:
  (1) `rust-mingw` must ride the rust-overlay `targetExtensions` override — host
  `extensions` rejects it (rustup installs it implicitly with the target);
  (2) nixpkgs' win32-threads mingw omits winpthreads, so std's `-l:libpthread.a` is
  satisfied by symlinking rust-mingw's own stub from `self-contained/` onto the link
  path (a whole-dir `-L` there would shadow the nixpkgs mingw libs);
  (3) the `pkgs.wine64` loader binary is named `wine`, not `wine64`.
  CI stays the authority; Wine does NOT simulate the Windows *checkout* (EOL/autocrlf —
  use the `core.autocrlf=true` clone above), NTFS semantics, or MSVC. macOS remains
  CI-only (no darwin cross from Linux).

## Milestone log

### P2-M5 — hardening + v0.2.0 (2026-11-18, COMPLETE)

Proptest panic suite, docs refresh, verification close-out and the Phase 2
tag. Decomposition: `build/PHASE_2.md` M5.

- **Panic suite**: `crates/oxidizr-export/tests/export_never_panics.rs` — 256
  proptest cases drive `WorkspaceBuilder` (valid by construction) through
  hostile names/titles, every view kind, group separators, styles, the
  `mermaid.*` gates and the autoLayout legs incl. the null-rank-direction
  abort; every landed format must never panic, `theme`/`json` additionally
  never error, and every `Ok` file set carries the naming invariants (no
  path separators, correct extension, unique).
- **Docs**: the stale `export --help` notes refreshed (CLI doc comments now
  say what landed when — only `websequencediagrams`/`static` raise
  OXZ-5007; `png`/`svg` are the build-rejected strings), `docs/diagnostics.md`
  OXZ-5007 row likewise, README status → Phase 2 complete + export
  quickstart lines. Plan errata checked — no open P2 fold-backs (the WSD
  errata row already reflects the Phase 4 deferral).
- **Verification**: P2-V5 (per-dialect color-scheme constants — C4
  `$`-defaults M3-a/`EXPORT-C4-PLANTUML.md`, Mermaid ALWAYS-Light M4-a §3)
  and P2-V6 (legend rules — C4/Mermaid `createLegend` = null, byte-confirmed
  by both sweeps) closed with evidence in `build/PHASE_2.md`.
- **Gates**: full `cargo xtask oracle verify` green before the tag —
  **4 json + 90 diagram + 6 theme golden set(s) match** (mermaid + theme
  legs included); `cargo xtask bench save M4` committed (17 dirs, the two
  `e2e_export_*` legs).
- **Tag**: `v0.2.0` (local only — never push, per the standing decision).
  Vendored default theme remains the only Phase 2 item deferred by
  environment (no DNS; D3 interim rule; `build/PHASE_2.md` M1 amendment).

### P2-M4 — Mermaid exporter + theme goldens + CI render + bench (2026-11-17/18, COMPLETE)

Decomposition M4-a..d: `build/PHASE_2.md`. THE contract: `build/EXPORT-MERMAID.md`
(the port mirrors the file, not memory).

- **M4-a** (this commit): catalog from the pinned Java (`MermaidDiagramExporter`
  402 lines + shared `AbstractDiagramExporter` base — the engine.rs legs are
  the BASE loops, so the port is a new `Dialect` + two engine deltas:
  mermaid's sequence form is a standalone document (NOT the PlantUML
  sequence walk) and the `.mmd` extension must thread the naming). Witness
  sweep (`/tmp/nix-shell.uni100/opencode/m4/sweep.sh`): 33/44 fixtures wrote
  ≥1 `.mmd` (10 oracle-load failures = C4 set; `text-block` loads but writes
  0 mermaid files — vacuous). Witness facts already pinned: no animation
  frames ever (`isAnimationSupported=false` — custom-view-animation = 2
  VIEWS, 2 files), no legend files, sequence `box` absent, CustomViews
  EXPORTED (base class, unlike PlantUML family).
- **M4-b + M4-c** (this commit): `oxidizr-export::mermaid` — full
  `MermaidDialect` port of the catalog (§1 case-sensitive property gates,
  §2 header with the null-rank-direction abort, §4–§7 boundaries/elements/
  rels incl. the 30-column `lines()` wrap and the DN-endpoint edge kill,
  §9 owned standalone `sequenceDiagram` via the new engine `Dialect` hooks
  `owns_sequence_document`/`sequence_document`, `mmd` extension through
  `create_diagram`). Dispatch: `ExportFormat::Mermaid` arm (Light scheme —
  the Java exporter has no scheme constructor); the two stale OXZ-5007 doc
  notes updated (`websequencediagrams`/`static` are the last unlanded).
  Tests: 10 unit (mermaid.rs) + 4 API (`export_mermaid.rs`: `.mmd` names,
  sequence gate, custom views exported, viewless exit-0) + format.rs arm.
  **M4-c byte sweep** (`/tmp/nix-shell.uni100/opencode/m4/sweep-ab.sh` vs
  the `ref-mermaid/` witnesses): **30 SAME / 3 DIFF / 11 vacuous-skipped** —
  the DIFFs are `bulk-operations` (D13 identity-hash rel-id fan-out, catalog
  §11, excluded from goldens) and the two D2 script fixtures (loader exit 1);
  ZERO port bugs — sequence fixtures (`dynamic`) byte-identical too.
- **M4-d** (this commit): proof corpus + tooling. `testdata/golden/mermaid/`
  = the 30 SAME fixtures copied byte-exact from the `ref-mermaid/` witnesses
  (raw oracle output — no needless oracle re-run); `testdata/golden/theme/`
  = 6 style-bearing fixtures (no theme URLs — D3) generated live with the
  new `xtask oracle gen --theme` leg and verified SAME against our binary
  (M0's theme export, now golden-pinned). xtask: `--mermaid`/`--theme` gen
  modes (a `GenMode` enum), `reference_export_plantuml` gains the extension
  parameter, verify gains the mermaid (`.mmd`) + theme legs plus a
  top-level-only JSON walk (the `theme/` subdir must not leak into it —
  same fix as `reader_m3`'s golden walk, which is contractually
  top-level); leg smoke: `gen --mermaid getting-started-short` regenerated
  the golden byte-identically (oracle determinism). Category-A:
  `export_mermaid_goldens.rs` (mermaid byte-parity ≥30 + theme byte-parity
  + `mmdc` render smoke with the P2-D9 LOUD skip — `mmdc` absent here).
  Benches: `e2e_export_plantuml_all`/`e2e_export_mermaid_all` (whole-corpus
  export through `execute`); baseline snapshot `bench save M4` follows.
  `PHASE_2.md` M4-d `refs:ignore` markers dropped (dirs exist).

### P2-M3 — C4-PlantUML exporter (2026-11-17, COMPLETE)

Decomposition M3-a..d + full Java fact catalog: `build/EXPORT-C4-PLANTUML.md`
(THE contract — the port mirrors that file) + `build/PHASE_2.md`.

- **M3-a** (43a8f79): catalog + C4 oracle witness sweep over the full corpus
  (44 fixtures; 34 produced witnesses, 10 oracle-load failures share the M2-d
  exclusions).
- **M3-b** (2706209): `oxidizr-export::plantuml::c4` — `C4PlantUmlDialect` on
  the M2-b `Dialect` trait (header with always-`<style>` defaults, `<C4/…>`
  stdlib includes, HashMap-order tags section via `put_entry`/`javahash`,
  boundary/group/element/Rel macro legs incl. the `$borderThickness` drop and
  `#ffffff`-bg-as-font quirks, `(xN)` instance unwrap, sequence box leg);
  engine hook `Dialect::exports_custom_views` (C4 = false: `export(CustomView)`
  is null → zero files, and `createLegend` is null → no `-key` legend files);
  `format.rs` C4 arm; API tests flipped off 5007.
- **M3-c** (A/B sweep, no commit — witnesses vs our CLI): 30 SAME / 4 DIFF /
  10 vacuous-skip. All 4 DIFFs are design-excluded fixtures, zero port bugs:
  `script-inline` + `script-in-dynamic-view` (D2, OXZ-5002), `text-block`
  (D11, OXZ-5005), `bulk-operations` (D13 `!elements` identity-hash fan-out
  order, permanent).
- **M3-d** (this commit, both legs): (1) goldens — `xtask oracle gen
  --plantuml --c4` + `oracle verify` c4 leg over `testdata/golden/plantuml-c4/`
  (29 sweep fixtures + the new `probes/c4-props.dsl` probe — the
  property-driven legs no vendored fixture reaches: `c4plantuml.tags/
  .stereotypes/.elementProperties/.relationshipProperties/.stdlib=false` +
  `plantuml.sequenceDiagram` box — BYTE-IDENTICAL to the oracle; RAW bytes
  throughout; `custom-view-animation` excluded C4-vacuous —
  CustomView-only workspace → zero C4 files, oracle exit 0). Category-A
  `c4plantuml_goldens_byte_parity` + C4 render smoke (3 goldens render
  offline — plantuml 1.2026.9 bundles the C4 stdlib) in
  `oxidizr-api/tests/export_plantuml_goldens.rs` (generalized to both
  corpora). `cargo xtask oracle verify` green (2026-11-17): 4 json + 59
  plantuml golden sets (30 plantuml + 29 plantuml-c4 at that run; the probe
  golden added after), 0 mismatches. (2) comparator — P2-D9
  semantic comparator `xtask/src/exportcmp/` (~230-line tolerant
  line-grammar: macro calls with quote/paren-aware arg split, skinparam /
  `<style>` / include / direction / box / arrow facts; per-file fact
  MULTISETS, order-normalized; whitelist = comment + blank lines only;
  unknown `Name(` lines parse under a `macro?` tag) driving
  `cargo xtask export-verify`. `C4_MACROS` = the macro inventory the pinned
  reference emits (Person/System/Container[Db|Queue]/Component[Db|Queue]/
  Deployment_Node, Boundary/System_Boundary/Container_Boundary, Rel,
  AddElementTag/AddRelTag/AddBoundaryTag/AddProperty/WithoutPropertyHeader/
  SHOW_LEGEND) — doubles as the P4 importer's macro inventory. Proof:
  `bulk-operations` (the D13 byte-DIFF fixture) compares SEMANTICALLY EQUAL;
  `shapes` / `dynamic` / `groups-nested` green. Plan note: "default-off
  `reference` feature" realized as a manual xtask subcommand — same intent
  (Category B, never on PR CI, needs the oracle binary), and xtask is
  dev-only so no cargo feature is needed.

### P2-M2 — Structurizr-PlantUML exporter (2026-11-17, COMPLETE)

Decomposition M2-a..d + recon inventory: `build/PHASE_2.md`. P2-V3 settled
during recon (icon scale pin → DEVIATIONS D12).

- **M2-d** (3 pieces, COMPLETE with this log): (1) dispatch leg —
  `ExportFormat::export` plantuml branch (per-view `structurizr-<id>-<key>.puml`
  + `-key` legend files per the reference `ExportCommand` write loop; viewless
  workspace = `The workspace contains no views` note + exit 0;
  `ColorMode::Default` → Light). (2) Oracle A/B divergence fix batch — the
  43-fixture full-corpus plantuml sweep (35→41 SAME after fixes, 3 DIFF =
  deviations) root-caused six families, all decided by the pinned Java and
  witnesses (details in WRITE-CATALOG §2/§5 + DEVIATIONS D13): deployment-view
  ancestor chain, darker-stroke lower-case hex, the `DynamicView`
  relationship re-sort (JSON writer + engine, all legs), `CustomView`
  verbatim title/description, `-/>` linked-id capture mid-removal, and
  `model.customElements` STORED order (DSL front-insert / reader TreeSet
  re-sort / writer sorts nothing). (3) Proof assets (this commit): 30-fixture
  plantuml golden corpus (`testdata/golden/plantuml/<fixture>/*.puml`, RAW
  oracle bytes, 168 files — the A/B SAME set minus vacuous viewless/error-abort
  fixtures) generated by the extended `cargo xtask oracle gen --plantuml`
  (`reference_export_plantuml`, no normalization — the diagram writer injects
  no harness artifacts); `oracle verify` gained the plantuml leg (dir-per-
  fixture byte compare); Category-A `oxidizr-api/tests/
  export_plantuml_goldens.rs` (byte-parity over the whole corpus + plantuml
  render smoke that skips when the binary is absent). Sweep witnesses:
  `/tmp/nix-shell.uni100/opencode/m2d/`. Deviation fixtures stay OUT of the
  corpus by design (D2 scripts, D11 text-block, D13 bulk fan-out, theme-URL
  fixtures offline-crash).
- **M2-b** (this commit): `oxidizr-export::plantuml` — the shared
  diagram-engine port (`AbstractDiagramExporter` + `AbstractExporter` +
  `AbstractPlantUMLExporter` helpers). `Engine<'a>{workspace, color_scheme,
  frame}` drives a `Dialect` trait; `model.rs` is the `ElemRef` model
  adapter (+ `element_by_id`, `owner_system_id`, canonical names); `mod.rs`
  the property constants, PlantUML shape maps and `java_cmp_str`. Ported:
  the §14 order contracts (numeric-id element `TreeSet`, `id/order`
  relationship set, name-sorted legs), per-kind bodies with the nested-group
  context-tracking quirks (1-char-context no-close, shallow-move ends AT the
  current indent) and the NO-blank deployment leg, animation frames + legend
  lifecycle (legend LAST), the dynamic sequence path (canonical-name sibling
  insertion), `is_visible` matrix, `type_of` terminology/metadata symbols,
  `break_text`. The Java cast/NPE aborts surface as
  `ExportError::ReferenceAbort` → **new OXZ-5009 EXPORT_ABORT** (registry +
  docs row + pipeline arm + coverage allow-list entry, unreachable via the
  corpus until export manifests exist). Two engine-ordering contracts were
  corrected by the pinned Java during testing — `findGroups` sorts elements
  IN PLACE by id STRING (the numeric `TreeSet` order never survives:
  `"10" < "2" < "9" < "abc"`), and the shallow-move leg writes
  `endGroupBoundary` before outdenting. 31 tests (8 MockDialect engine
  scenarios: exact hook sequences, quirk output, frame visibility, abort).
- **M2-a** (3 pieces: `be26b56` + `d179653` + `a12bd29`): D12 row `be26b56` (reference
  `calculateIconScale` FETCHES the icon at export time — `ImageIO.read(new
  URL(…))` — every failure path lands on `0.5`; oxidizr pins `0.5`,
  zero-network); `oxidizr-export::property::export_property` `d179653` —
  THE typed 2-level cascade (`view → views.configuration → default`, Java
  `getOrDefault` semantics: present-but-empty SUPPRESSES, never cascades;
  4 tests); catalog `build/EXPORT-PLANTUML.md` §§1–13 = the P2-V2 deferred
  micro-grammar half, line-exact from the pin: writer mechanics (2-space
  indent, literal `\n` escapes, `replace` first-hit), full header order
  (title sizes incl. the null-fontSize `null` literal, ranksep `/5` +
  nodesep `/10` INTEGER division, `<style></style>` placeholder), element/
  relationship/boundary/group/legend grammars with exact format strings,
  the sequence-diagram path (teoz, LinkedHashSet + boundary stack,
  actor/database stroke-color rewrite), style-footer layout (7 category
  passes over the name-sorted `HashSet`, base64 selectors, per-class field
  order), `idOf`/`(?U)\W` filter, P2-V5 (`#ffffff`/`#444444`, `#111111`/
  `#cccccc`) + P2-V6 (legend for every view) Structurizr halves, and the
  quirk register — including the `findGroupStyle` 711 upstream bug (icon ←
  color) we must mirror.

### P2-M1 — theme infrastructure: styles + ThemeStore + wiring (2026-11-17)

M1-a `199bc70`, M1-b `95130bb`, M1-c `169e685`, M1-d `eadf24b`, M1-e-A
`0759f96`, M1-e-B this commit — M1 complete; decomposition and decisions
(P2-D2/D3/D4/D8 + amendments): `build/PHASE_2.md`.

- **M1-a** `EXPORT-PROPERTIES.md` catalog: cascade is 2 levels + default (with
  property literals + per-site match case-sensitivity), view-kind matrix,
  theme chain §6, style constants §7. P2-V2 (matrix/properties half) and
  P2-V4 verified from the pinned sources.
- **M1-b** `oxidizr-core::styles`: `Styles.themes: Vec<Theme>` (NEVER
  serialized — the Java field has no getter/setter) +
  `find_element_style`/`find_relationship_style` resolution per catalog §6–§7
  (tag-composition order, `copyFrom` rules, darker-stroke = f64 `×0.7`
  truncation formatted `#%06X`, linked-relationship tag chain). 12 tests.
- **M1-c** `oxidizr-export::themes` `ThemeStore`: install source chain
  `--themes-dir` > `OXIDZR_THEMES_DIR` > `<ws-parent>/themes` >
  `STRUCTURIZR_THEMES` (first dir wins, one-level `theme.json` scan, env via
  injected closure for hermetic runs) + load mirror (URL → fetch; installed →
  file read with NO icon rewrite — the reference asymmetry; else silent skip)
  + `paths` (XDG cache dir + hand-rolled FIPS 180-4 SHA-256 cache keys) +
  diag OXZ-5008 + coverage allow-list entries. The `curl` fetch leg mirrors
  source (no DNS here; A/B lands with the M2 probes). 19 tests.
- **M1-d** wiring (`eadf24b`): installed-theme registry reaches the DSL
  parser via `ParseOptions.installed_themes` (Arc-closure newtype — dsl crate
  stays FS-free, P1-D1), built BEFORE parse (`installThemes` parity);
  `add_theme` installed-name branch (raw-token registry check, then trim +
  append WITHOUT dedup — only the URL branch dedups); `export_cmd` runs the
  `loadThemes` leg for `is_diagram()` formats BEFORE dispatch (reference
  order — theme failures precede the format's OXZ-5007; `NetDisabled` →
  5001, else 5008); CLI `--allow-net`/`--themes-dir` globals (D5: the flag
  gates theme URLs ONLY — `!include`/`extends` stay rejected, DEVIATIONS §D5
  row updated); `oxidizr-api/tests/export.rs` pins the four legs; README +
  diagnostics docs rows. Oracle A/B (m1d witness): `themes` arrays
  byte-identical vs the reference with a `STRUCTURIZR_THEMES` install.
- **Probe finding → M1-e (new scope, PHASE_2.md)**: the probe also exposed
  two PRE-EXISTING app-layer load-parity gaps, reproducible on the P1-era
  dsl2json path and corpus-immune (vendored fixtures define views and end
  WITHOUT a final newline): (1) the reference CREATES default views on
  no-views loads (`AbstractCommand` 109-112) — a Locked-list must-fix;
  (2) its retained `structurizr.dsl` base64 can never carry the file's final
  newline (`readLine` accumulation) while ours does (`split('\n')` trailing
  `""`). Fix both before M2.
- **M1-e-A** retained-DSL split (`0759f96`): `compat::java_split_dsl_lines`
  mirrors the reference entry `parse(String)` — `split("\\r?\\n")` consumes
  one CR per separator and drops ALL trailing empties — so the retained
  `structurizr.dsl` base64 never carries the file's final newline. Helper
  unit matrix + end-to-end retention test; m1d re-probe: base64
  byte-identical.
- **M1-e-B** default views (this commit): core
  `WorkspaceBuilder::create_default_views` (viewops) ports
  `ViewSet.createDefaultViews` (`ViewSet` 1191-1264): landscape view first;
  per software system sorted by Java UTF-16 name order (stable) a context
  view, a container view with `enableAutomaticLayout(TopBottom)` +
  `AutomaticLayout.DEFAULT_*` (100/50/50/true) when it has containers, and a
  component view per name-sorted container with components; per deployment
  environment (deduped first-seen from the top-level nodes, then sorted)
  scoped deployment views per software system holding CONTAINER instances
  (name-sorted — `softwareSystemInstance`s do NOT scope), or one unscoped
  view when the environment holds only infrastructure nodes. Pipeline
  `apply_default_views` mirrors the `AbstractCommand.loadWorkspace` tail
  (`!model.isEmpty() && views.isEmpty()` — model = people + systems +
  top-level nodes; views = all nine collections) via a `WorkspaceBuilder::
  adopt` round-trip, applied on the dsl2json serialize, the json2json parity
  leg and the export loader. NOT hooked: `validate` (its violations mirror
  the reference steps running BEFORE the tail; reference `ValidateCommand`
  never serializes views) and `keep_unknown` (D3, documented non-parity);
  no `reconcile` hook (no reference command). Oracle A/B (m1e witnesses):
  DSL leg (`workspace2.dsl` — name-disordered systems, containers/
  components, containerInstance + softwareSystemInstance subtlety, scoped +
  infra-only environments) and JSON leg (`noviews-base.json`) both
  byte-identical to the reference modulo the per-process identifier UUIDs
  and `structurizr.inspection.*`; the m1d witness is now FULLY closed
  (views included). E2E: 65 corpus manifests re-blessed (`xtask e2e bless`,
  views counts only — the drift IS the reference parity).

### P2-M0 — export scaffold + theme/json dispatch ✅ (2026-11-17)

Commits `6f6cc64` (compat parity fixes) + `ca292e7` (M0 body). Decisions and
P2-V1 verdict: `build/PHASE_2.md`.

- **P2-V1 VERIFIED** at the oracle (witnesses `/tmp/nix-shell.uni100/p2/v1/`):
  `ThemeUtils.toJson` = plain mapper, `NON_EMPTY` globals vs per-field
  `NON_NULL` style fields, declaration-order keys, workspace name/description,
  live style collections (DSL-sorted / JSON-document-order), setter clamps on
  read (`strokeWidth` [1,10], `opacity` [0,100], rel `position` [0,100]).
  Two read-path parity fixes fell out (style doc-order + clamps — 6f6cc64,
  WRITE-CATALOG §5/§6).
- `oxidizr-export` crate (P2-D1): `IndentingWriter` (reference defaults: 2
  spaces; bare `writeLine()`; `toString` drops ONE trailing newline; negative
  indent pads nothing), `Diagram`/`Legend`/`WorkspaceExport` beans +
  exporter traits, `ExportFormat` registry (exact `ExportCommand` keys,
  lowercased; color suffix only on the `plantuml*` families) and `naming`
  (workspace stem + `-theme`; `structurizr-<id>` diagram prefix, `-key`
  legends, numbered frames). 11 unit tests.
- `oxidizr-json::theme`: `theme_from_slice`/`theme_write`/`theme_to_vec`,
  byte-exact vs the out3/out6 witnesses (`theme_json.rs`).
- api `Request::Export` + `export_cmd` mirroring `ExportCommand` (load by
  extension, dir default = canonical workspace parent, mkdirs BEFORE dispatch,
  `Writing <path>` notes, unknown format → note + exit 0, unimplemented
  registry format → OXZ-5007); cli `export -w/-f/-o`; diag 5007 + docs row +
  coverage REGISTRY/allow-list.
- A/B evidence: `export -f theme` of `big-bank-plc.dsl` and of the
  document-order `order.json` probe are BYTE-IDENTICAL to the oracle
  witnesses; `-f json` output is a JSON read/write fixed point.
- Deferral: default-theme vendoring (needs network for
  `static.structurizr.com/themes/default/theme.json`; absent from the pinned
  monorepo) → joins M1 with the `ThemeStore`.

### P1T — e2e regression + benchmark suite (interlude) ✅ (2026-11-17)

Full plan, decisions P1T-D1..D9, landed refinements and V-T1..V-T5 verdicts:
`build/PHASE_1T.md`. Shape of the suite:

- **T0** `oxidizr-api` crate: the whole CLI pipeline callable in-process
  (`execute(Request, &Options) -> Output`) — the harness never shells out
  (except one subprocess smoke anchor).
- **T1–T3** corpus `e2e/corpus/`: 300 entries (156 valid / 24 authored-invalid /
  120 mutated via `cargo xtask e2e mutate`, seeded, byte-stable). Sidecars
  (`expected.toml`) assert USE-CASES: exit class + OXZ code set + shape counts
  — never bytes, never message text (`xtask e2e bless`). Coverage gate:
  ≥150/≥120 floor + 11/14 debt codes (allow-list {1001, 4002, 9001}).
- **T4** full tier (env `OXIDZR_E2E_FULL=1`, nightly `e2e.yml`): invariants
  over every valid entry — parse determinism, `dsl2json`→`json2json`
  byte-stability, `fmt` fixpoint + semantics (scratch-dir replay),
  self-`reconcile` identity. **Paid for itself on first run**: two REAL engine
  bugs, both settled with oracle verdicts — (1) style colours normalize on the
  DSL path too (`Color` port → `oxidizr-core::colors`, note
  `pitfalls/dsl-style-color-normalization.md`); (2) `fmt` verbatim replay for
  source-magic line ends (seed `0c3752…`, pre-existing at HEAD, note
  `pitfalls/fmt-line-end-guards.md`). Also resolved: `group { url }` IS
  reference-rejected — `testdata/dsl/group-url.dsl` is a negative fixture.
- **T5** six `e2e_` criterion benches over the corpus behind feature
  `e2e-bench` (CI's `cargo bench --workspace` skips them — local dev only);
  `1T` baseline committed, `cargo xtask bench` always opts in.
- **T6** close-out: TESTING.md §E2E suite + 1T bench paragraph, README dev
  line, this log. **No tag** (interlude; v0.1.0 stands).

Verification items V-T1..V-T5 all closed 2026-11-17 (details in PHASE_1T.md);
notable: zero `#[ignore]`d tests workspace-wide → env gate chosen over
`--include-ignored` (V-T4); feature-gated benches are not compiled without the
feature (V-T5).

### P1-M7 — hardening, benches, docs, v0.1.0 ✅ (2026-11-16)

**Phase 1 COMPLETE.** Last milestone: TESTING.md-mandated benches + first real
baseline + perf sanity + README quickstart + tag.

- Benches (criterion, all `harness = false`, corpus = vendored `testdata/`,
  UTF-8-only files — the latin-1 probe can't reach `&str` APIs): `oxidizr-dsl`
  `parse.rs` rewritten (`m7_dsl_parse_corpus` = every `.dsl`, plus
  `m7_dsl_parse_test.dsl`/`m7_dsl_parse_big-bank-plc.dsl` worst-case proxies)
  and new `fmt.rs` (`m7_dsl_fmt_corpus`); `oxidizr-json` `json.rs`
  (`m7_json_read_corpus` over golden+read-corpus+validation+probes —
  rejections count too; `m7_json_write_corpus` over the reader-accepted
  workspaces); `oxidizr-cli` `reconcile.rs` (`m7_reconcile_matched_big_bank` =
  same-workspace layout/full graft; `m7_reconcile_cross_corpus` = big official
  export vs unrelated DSL ≈ zero matches, claim/drop machinery at scale).
  Setup (read/parse) stays outside the timed loop.
- Perf sanity (informational per PHASE_1): whole-corpus DSL parse **≈ 11.5 ms**
  median release — the < 100 ms bar met with ~9x margin. JSON read corpus
  ≈ 1.0 ms, write ≈ 1.8 ms, fmt corpus ≈ 1.5 ms, reconcile 0.37–0.60 ms per
  match.
- Baseline: `cargo xtask bench save M7` → `bench-baseline/M7/` (9 groups,
  36 files; first real baseline — M0 kept as the cross-milestone anchor;
  context lines in `bench-baseline/README.md`).
- Root `README.md` written (repo had none): build, all five subcommands with
  flags/exit codes, compat + dev pointers. (`--help` itself landed with M6-a.)
- Trap (minor): criterion 0.5 keeps `estimates.json` beside `benchmark.json`
  per group — the `cmd_bench_save` copy expects exactly that layout, no
  change needed; medians for sanity checks come from `<group>/new/estimates.json`.

### P1-M6d — cargo-fuzz targets + CI nightlies ✅ (2026-11-16)

`fuzz/` is a SEPARATE cargo workspace (`exclude` in the root `Cargo.toml`):
the fmt/clippy/test quartet never sees the sanitizer targets; the pinned
nightly stays the toolchain for it (cargo-fuzz requires nightly — already
NIGHTLY.md's stated reason for the pin).

- Targets (fixture seed corpus per PHASE_1 §P1-D11): `dsl_parse` — the whole
  pipeline over lossy-UTF-8 bytes (beyond the CLI's UTF-8 gate the engine
  must still be panic-free); `dsl_fmt` — format any text + idempotence assert
  (fuzz sibling of the M6-b proptest invariant); `json_read` — the reader is
  total (rejects as values, never panics) and the writer's canonical form is
  a fixpoint (re-read + re-write reproduce bytes).
- `cargo xtask fuzz [SECONDS]` (default 10): builds all targets, stages the
  testdata fixtures into gitignored `fuzz/corpus/<t>/` (libFuzzer deposits
  discoveries beside the seeds), runs each target time-boxed
  (`-max_total_time` · `-timeout=10` · `-rss_max_mb=2048`). Crash = nonzero
  exit fails the job; artifacts under `fuzz/artifacts/`.
- CI: `.github/workflows/fuzz.yml` — nightly schedule + dispatch (kept OUT of
  ci.yml so the weekly Category B schedule stays weekly), 60 s/target,
  artifacts uploaded on failure.
- Trap (→ `env/fuzz-host-toolchain.md`): the dev shell exports the windows-gnu
  leg's `CC/CXX/AR/RANLIB` (mingw-w64); cc-rs obeys them even for HOST builds
  — libfuzzer-sys then compiles libFuzzer's *Windows* sources for Linux and
  dies. `cmd_fuzz` strips the four vars (CI env is clean; the strip keeps
  local == CI).
- Smoke: 107 DSL + 26 JSON seeds staged, all three targets clean
  (≈ 2.5–4k execs per 3 s target).

### P1-M6c — `reconcile` + validate polish ✅ (2026-11-16)

Two halves, two commits (reconcile 7351db1; suggestions with this log). No
official reference `reconcile` exists (structurizr org repos 404, reference
app has no such command) — the P1-D8/F1 text in PHASE_1 IS the contract.

- **reconcile** (`oxidizr-cli/src/reconcile.rs`, pure composition, no core
  changes): re-parsed DSL workspace is the model/membership authority; the
  layout JSON supplies geometry. Matching: `structurizr.dsl.identifier`
  exact → kind-prefixed canonical-name fallback (`WorkspaceBuilder::adopt`
  clones for `canonical_name_full`). Contested claim (two old ids → one new
  id): first claim REVOKED, new id poisoned — NEITHER remapped (OXZ-3002).
  Graft: element x/y, relationship routing, view paper/dimensions/
  automaticLayout, id-remapped animation steps (empty-after-remap dropped);
  missing counterparts → informational dropped lines. JSON emitted FIRST,
  even when ambiguous; report on stderr via new `Renderer::note`
  (`--quiet`-aware); ambiguity exits 1.
- Fixture trap: BOTH input paths reject duplicate element names (DSL parse
  `CoreError::DuplicateTopLevelName`, JSON hydrate validation), so an
  ambiguity fixture needs identifier-vs-canonical on one new id
  (`x="Foo"`+`y="Bar"` → `x="Bar"`) — `pitfalls/reconcile-ambiguity-
  fixtures.md`; kind prefixes (`Person://` ≠ `SoftwareSystem://`) keep
  cross-kind drops unambiguous.
- **validate polish**: `suggest.rs` decorates `Issue::help` with
  `did you mean "X"?` at the CLI boundary (P1-D2 — engine `DslError` and its
  message snapshots stay byte-identical, D1 freedom since help is stderr-
  only). Keyword candidates: parsed back out of the reference
  `Unexpected tokens (expected: …)` message (verbatim in it — no engine
  plumbing); identifier candidates: parsed workspace's
  `structurizr.dsl.identifier` values + space-free element names (anonymous
  elements are name-referenceable; volatile uuids never are). Case-
  insensitive Levenshtein, budget ≤4 chars → 1 edit, ≤8 → 2, else 3.
- Gates: +8 reconcile unit, +6 suggest unit, e2e 20→22 (identifier remap
  across id shift; contested contest; rendered suggestions in `validate`
  stderr). `run()` helper now tolerates EPIPE when the child exits before
  reading stdin (usage-error race — cli-e2e-pitfalls #6).

### P1-M6b — `fmt`: canonical formatter on the lossless CST ✅ (2026-11-16)

P1-D7 printer in `oxidizr_dsl::fmt` (NOT the CLI — P1-D1 topology; the CLI
only adds stdin/file plumbing + `--write`/`--check`), formatter never validates.

- Printer walks `cst::build` root children paired 1:1 (cursor) with the
  `preprocess::preprocess` logical lines; 4-space `INDENT`; brace depth from
  token VALUES (a quoted `"{"` opens a context in the reference too — quotes
  are stripped before its scope checks); blank runs compact to 1; whole-line
  comments re-indent, block-comment region interior + text-block raw +
  dropped EOF tail byte-verbatim; conservative unquoting (`unquote_value`:
  terminated, no `"`, no trailing `\`, no whitespace, not `#`/`//`/`/*`-
  starting, not `*/`-ending); LF terminators.
- Line-end guards (→ `agents/notes/pitfalls/fmt-line-end-guards.md`): `"""`-
  ending body → protective space; `\`-ending body → original bytes verbatim
  (unrepresentable — the reference always re-joins); content-final `\r`
  doubled before the LF (`split("\\r?\\n")` is the contract, `parse(File)`
  `readAllLines` is NOT); between-segment-after-terminator whitespace is the
  joiner indent `stripLeading` drops — segment-membership test, not a
  prev-leaf heuristic.
- Gates (`tests/m6_fmt.rs`): corpus idempotence + invariant-1 workspace
  equality (JSON bytes with the base64 `structurizr.dsl` line dropped — it
  legitimately carries the formatted source, P1-D7 invariant 3 — and UUID-
  shaped values redacted, `uuid_v4` is volatile) over ≥90 UTF-8 corpus
  members; proptest idempotence + logical-surface preservation (20 000 cases
  × 400-char DSL-flavoured input green; `.proptest-regressions` seeds kept).
- Two latent M1 bugs fixed on the way (`lexer::lex_comment` mis-tiled a
  comment joined onto an all-whitespace `\` segment; regression test
  `comment_after_all_ws_continuation_segments`).
- CLI `fmt`: default stdout · `--write` in place (only changed files,
  mtime-stable) · `--check` lists unformatted + exit 1 · `--write` `-`
  usage exit 2 · CLI test 15→20 e2e. `oxidizr-ast` dropped from CLI deps
  (formatter consumes the CST inside `oxidizr-dsl`). Refs scratch dir
  `agents/plan/reports/` pinned with a self-ignoring `.gitignore`.

### P1-M6a — CLI surface: dsl2json / json2json / validate ✅ (2026-11-16)

P1-D8 contract implemented end-to-end; `fmt`/`reconcile` keep their plan-ref
bails (M6-b/c).

- `oxidizr-cli`: commands wired to `parse_dsl` + `oxidizr_json` (engine
  `ParseOptions::json_loader` injected with `oxidizr_json::from_slice` —
  P1-D1 topology intact, `workspace extends <*.json>` works through the CLI);
  `-` stdin/stdout; strict UTF-8 (V-8 closed: no `--encoding`); canonicalized
  file path handed to the engine so reference `parse(File)` semantics hold.
- `report.rs`: `Issue` (+boxed `Snippet`) / `Renderer` over miette 7
  `GraphicalReportHandler`; `DslError` → reference exception text re-assembled
  (`<msg> at line N of <file>`) + `source_line` snippet; `JsonError::Parse`
  maps serde line/col to a byte offset; `NO_COLOR`/`--no-color` → ascii theme.
- Exit codes: 0 ok · 1 diagnostics (errors; `--strict` adds warnings) ·
  2 usage (clap) · 3 I/O (`OXZ-5006` incl. non-UTF-8, undetectable format).
- D3 `--keep-unknown`: new `oxidizr_json::roundtrip_keep_unknown` (`keep.rs`)
  overlays the input `Value` onto the writable `Json` tree — unknown,
  legacy (`revision`, `enterprise`, …) and `NON_EMPTY`-omitted members are
  re-appended verbatim after the known keys (sorted); arrays pair by
  `id`/`key`/`selector` identity, then compatibility (the writer re-sorts).
  `printer::Json::Raw` carries non-`i64`/null literals byte-verbatim. Idempotent;
  no-extras input reproduces the parity bytes exactly.
- New codes registered (`docs/diagnostics.md`): `OXZ-3001` validation,
  `OXZ-4001/4002` JSON read/write, `OXZ-5006` CLI I/O/policy.
- Gates: `tests/cli.rs` 15 e2e tests — golden byte-exact stdout, JSON-base
  loader injection, keep-unknown against read-corpus legacy keys, exit codes
  0/1/2/3, miette text checks (`OXZ-…`, reference text, snippet line); quartet
  green; `oracle verify` 4/4 (printer/writer untouched in parity output).
- Test-writing pitfalls worth remembering (→ `agents/notes/testing/
  cli-e2e-pitfalls.md`): `wait_with_output` needs explicit `Stdio::piped()`;
  `structurizr.dsl.identifier` is a fresh random UUID per parse (normalize it
  in stdin↔file comparisons); `lastModified*` are KNOWN round-tripping fields
  (`revision` is the legacy one); relative `workspace extends`/`!include`
  must not appear in stdin-vs-file fixtures (no file context — reference-
  faithful skip).

### P1-M5 — `workspace extends` (file + JSON bases) ✅ (2026-11-16)

Reality note first (PHASE_1 §P1-D11): everything else in M5's original scope
had already landed with M4, so M5 = `workspace extends` only; decomposed
M5-a/b/c.

- **M5-a** `core`: `WorkspaceBuilder::adopt` — replaces the workspace under
  construction (reference `WorkspaceParser.parse` REPLACES the object) and
  replays every element+relationship id through `IdGenerator::found`
  (`Model.add` seeds the counter on every add, `Model.java:274/286`);
  `Properties::remove` (Java `HashMap.remove` semantics — slot order is
  part of the write contract, V-11) + `remove_property`. Commit 9f174e6.
- **M5-b** `dsl` engine: `Parser::extend_from` per `WorkspaceParser.java`
  @ad34036 — URL sources → OXZ-5001 with the reference feature texts;
  non-file parse skips silently (`context.getFile() == null` quirk);
  `… could not be found` / `… should be a single file` guards; `.json`
  bases via injected `ParseOptions::json_loader` hook (topology kept: no
  `json` dep in `dsl`; CLI wires it, tests use it as dev-dep) with
  `registerIdentifiers` (elements AND relationships carrying
  `structurizr.dsl.identifier`); DSL bases via a child `Parser` whose
  register merges into ours (`DslParserContext.copyFrom` — verbatim-merge
  rationale in `idents.rs`), `configureFrom` (identifier scope, archetypes,
  constants/variables), then `adopt`; `clearDsl` + `dsl_portable = false`
  (extending file keeps no source — `DslTests` `getDsl == ""`);
  `extending_workspace = !model.isEmpty()` (people+systems+deploymentNodes)
  drives get-or-add-by-name reuse in the four element parsers
  (`isExtendingWorkspace` branch; the registration collision guards
  inherited from M4 reproduce the reference throw-for-throw).
- **M5-c** gates: OXZ-5004 → tombstone (`docs/diagnostics.md`, constant
  removed); 11 `EXTENDS_DEFERRED` burn-in rows retired — 9 file fixtures
  parse CLEAN, the 2 URL fixtures re-registered under D5 (OXZ-5001,
  `Extends via HTTPS are not permitted`); `m5_extends.rs` 7 Category A
  tests (layering, hierarchical `configureFrom` witness, JSON-base
  identifiers, 3-level chain, no-retention + id continuation, loader-absent
  guard, file guards, JSON-base element reuse); golden `extend/4.dsl`
  byte-exact (`m5_goldens.rs`).
- `xtask oracle`: normalization drops the empty `properties` shell left by
  inspection-property deletion; `find_fixture` resolves JSON fixtures
  (`oracle verify` worked on `m3b-defaults` for the first time since M3 —
  all 4 goldens verify green); `agents/notes/testing/oracle-app-harness.md`
  records why viewless fixtures cannot be goldens (V-10 `createDefaultViews`).

Quartet green (`cargo xtask ci`); `oracle verify` 4/4; nothing pushed.

### P1-M4 — `oxidizr-dsl` engine: model + views + styles + deployment + dynamic ✅ (2026-11-15)

- The engine (`crates/oxidizr-dsl/src/engine/`) is a faithful port of the
  reference single-pass parser: per logical line → tokenize → `${}`-substitute
  → `id =` strip → the ordered `else if` chain over the context stack. The arm
  ORDER is the compatibility contract — every arm carries its
  `StructurizrDslParser.java` line number (@ `ad34036`); D1 per-line error
  recovery truncates the failing line's token tail and keeps parsing.
- Lint policy for the port: mechanical lints get real fixes (`strip_prefix`,
  let-else, `clamp`, merged arms, …); faithful-port shapes get justified
  `#[allow]`s (`too_many_lines` on the chain sections and the per-context
  permitted-token tables, `match_same_arms` where distinct reference classes
  collapse, the `tok.rs` keyword table kept whole for M5+). Workspace-wide
  `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- **Burn-in harness**: `crates/oxidizr-dsl/tests/m4_fixtures.rs` walks every
  `.dsl` under `testdata/dsl` (107 files); each must parse CLEAN or match the
  `KNOWN_ERRORS` register (first-error OXZ code + message prefix), checked in
  BOTH directions so retired deferrals must be pruned here. 58 clean +
  48 registered: 11×OXZ-5004 `workspace extends` (retires at M5), 4×OXZ-5003
  D7 docs/ADRs (P4), 3×OXZ-5005 D11 plantuml imports (P4), 2×OXZ-5001 D5 URLs,
  8×OXZ-5002 D2/D10 JVM rejections, 12 reference-asserted failures with pinned
  messages (`DslTests.java`), 7 include/extends component files, and
  `image-view.dsl` (corpus vendors no `image.png`). `iso-8859.dsl` has its own
  Latin-1 test mirroring `test_ISO8859Encoding` (`Namé` loads).
- Bugs the burn-in caught (fixed + unit-pinned):
  1. `WorkspaceBuilder::dsl_uses` rejected ANY second relationship between the
     same pair. Reference: `uses()` returns null (→ the "already exists"
     throw) only on same destination AND same description — different
     descriptions add fine (`filteredviews`, `bulk-operations`,
     `dynamic-view-with-explicit-relationships`).
  2. `!element <deploymentEnvironment> {` opened no context. The DSL-package
     `DeploymentEnvironment extends Element` shim registers under the `de =`
     identifier and the finder ladder pushes a fresh
     `DeploymentEnvironmentDslContext(name)` (`deployment-environment-empty`).
- Reference-quirk errata, each verified against Java @ `ad34036` and pinned by
  engine tests (all byte-compatible with the reference; NOT deviations):
  - view-definition / `animation` / `terminology` arms push their context
    UNCONDITIONALLY (a no-`{` definition still swallows following lines).
  - `endParallelSequence(!relationships)` (`DynamicViewParallelSequenceDslContext
    .end()`): a NON-empty parallel block DISCARDS the child counter — the next
    top-level step reuses the number (orders `["1","2","2"]`); an empty block
    burns one number instead.
  - a `dynamic <system>` view can NOT add the scope system itself
    ("`X is already the scope of this view and cannot be added to it.`");
    unscoped `dynamic *` steps accept only Person/SoftwareSystem; pair steps
    require an EXISTING model relationship.
  - explicit `N:` step orders burn one sequence number (the next implicit step
    continues from the counter, not from `N`).
  - `a -> b {` inside a dynamic view consumes `{` as the description
    (JSON: `"description": "{"`).
  - svg content inside `light { }`/`dark { }` blocks stores PLAIN `content`
    (the reference never fills `contentLight`/`contentDark` from the DSL).
  - `theme <file>` and `image <file>` file branches need a file-based parse
    (`ParseOptions::file*`); against a fragment the reference NPEs and we emit
    the same failure path.
  - `terminology relationship Connects to` stores `tokens.get(1)` verbatim —
    the unquoted second word is dropped (`Connects to` → `Connects`).
  - `InstalledThemes` is app-layer: `theme <name>` stores the name verbatim;
    the file-branch existence check is reference-exact (`Theme X does not
    exist at <dir>/X`).

### P1-M3b — `oxidizr-json`: byte-parity writer + lenient reader ✅ (2026-11-14)

`oxidizr-json` per the amended P1-D5: hand-rolled writer (`Json` tree → Jackson-shaped
printer + `javahash` `HashMap` emulator) and a `serde_json::Value`-walk reader (with
`preserve_order` — document-order property maps are the precondition for byte-stable
round-trips). Every WRITE-CATALOG §1–§5 rule is explicit code (sort functions, inclusion
per class, computed view names, instance name suppression); `Workspace → bytes` and
`bytes → Workspace` now bracket the compatibility contract:

- **Writer goldens byte-identical**: `m3-order.json` + `workspace-properties.json`
  re-written from the public core API (key tables, all §5 sorts, HashMap property order,
  `{ }`/`[ ]`, no trailing newline). `golden_m3.rs`.
- **Reader**: §6 leniency matrix (unknown-ignore, coercions, aliases incl. the
  document-order `enterpriseContextViews` replacement, `theme`→`themes` + `addTheme`
  dedupe), hydrate-equivalent validation (`JsonError::Validation` carrying the FIRST
  reference message), instance `name`/`description` derived after the link checks.
  `read-corpus/` (5) loads; `reader_m3.rs` + 15 tests.
- **Reader probes vs the oracle (2026-11-14)** — committed as
  `testdata/probes/m3b-defaults.json` + its golden (read→write byte-identical witness;
  probe inputs must stay re-readable — a healthCheck with null `name`/`url` NPEs the
  reference READ via `HttpHealthCheck.hashCode()` on Jackson's first HashSet insert).
  Settled: healthCheck `headers` write in **HashMap bucket order** replayed from
  TreeMap-sorted insertion (`getHeaders()` copies — writer uses `javahash::entry_order`;
  `{"X-A","X-B"}` writes `X-B` first); `name`+`url` required on read; `interval`/
  `timeout` read defaults 0/0 (60 is DSL-only); `automaticLayout` read defaults
  0/0/0/false + NULL `rankDirection`, and a PRESENT-but-unparsable `rankDirection`
  throws `A rank direction must be specified.` (the only guarded enum setter);
  style colors normalized by the `Color` setters — vendored 148-name table
  (`colors.rs`), `#rrggbb` lowercased, else `<x> is not a valid hex colour code or
  HTML colour name.` (M4/M5 DSL side must apply the same normalization).
- **Core faithfulness**: `AutomaticLayout.rank_direction`, `FilteredView.mode`,
  `Section.format`, `Decision.format`, `User.role` became `Option` (null bean fields —
  unknown/absent JSON enums bind null, the writer omits the key; builder wraps DSL
  values in `Some`). `users` DO round-trip (old catalog claim corrected in §6 errata).
- **Validation-corpus erratum**: per the reference `WorkspaceRulesValidationTests`,
  15/16 fixtures throw and `TopLevelDeploymentNodeNamesAreNotUniqueButTheyExistIn…`
  must LOAD (uniqueness is per name+environment). The harness now pins every first
  message against the Java test strings verbatim.
- **proptest**: `to_vec → from_slice → to_vec` byte-stable for random property maps
  (the `H(H(I)) == H(I)` idempotence behind JSON→JSON stability).
- Deferred to M6 (CLI milestone): the `--keep-unknown` capture and the `json2json`
  command wiring (D3 opt-in, no default-parity effect) — noted on the PHASE_1 M3 line.
- Gates: 113 tests green, `cargo fmt --check`, `cargo clippy --workspace --all-targets
  -- -D warnings` clean.

### P1-M3a — write catalog (P1-D6) + core model alignment ✅ (2026-11-14)

`agents/plan/build/WRITE-CATALOG.md` is the authoritative JSON write contract (resolves V-1,
V-2, V-11). Built by (a) reading `structurizr-client` `io/json/JsonWriter`/`AbstractJson*`
and every `structurizr-core` POJO getter (Jackson `SORT_PROPERTIES_ALPHABETICALLY` +
`NON_EMPTY`), and (b) a single oracle probe `testdata/probes/m3-order.dsl` exported
twice (`cmp` byte-identical → deterministic) whose raw output is the byte witness
`testdata/golden/m3-order.json`.

Headline findings (all verified, not recalled):

- **V-11 resolved: replicate Java `HashMap` bucket order exactly** — free-form
  `properties` are NOT sorted. Core `properties` moved from `BTreeMap` to a new
  insertion-ordered `oxidizr_core::properties::Properties` (the writer replays insertion
  order through a `HashMap` emulator). Two pathological edge cases accepted (tree bins,
  inspection-key capacity) → `DEVIATIONS.md` D8.
- **V-2 resolved: primitive ints/bools are NEVER "empty"** — `x`/`y` (`0`),
  `instanceId`, `enterpriseBoundaryVisible` (`true` default), `*BoundariesVisible`
  (`false`) are ALWAYS written. `generatedKey` only when `true` (`NON_DEFAULT`);
  RelationshipView `response` (`NON_NULL`) writes `false`.
- Computed view `name` is always written (exact strings per kind, §3.1); instances
  serialize with NO name/description; DynamicView `deploymentEnvironment` is invisible
  both directions (private, no Jackson accessors); `filtered` legacy `filter` key is
  read-IGNORED (no setter).
- Array order (V-1): element arrays numeric-id ascending; view lists `(order, key-ci)`;
  relationship views `id/order`; filtered tags & style sets UTF-16 `String.compareTo`.
- Core model aligned to the catalog: `Properties` everywhere; `metadata` on
  CustomElement; static-view `animations`; landscape/context `enterpriseBoundaryVisible`
  default **true**; dropped `RelationshipView.technology`, `FilteredView.legacy_filter`;
  `@JsonIgnore` on ANY accessor suppresses the whole Jackson property (the mystery of
  the invisible `name`/`mergeFromRemote` getters, resolved).
- DSL grammar discoveries logged in WRITE-CATALOG §8 (filtered/deployment view
  signatures, `deploymentGroup` declaration vs reference semantics, no async arrow,
  Landscape scope rejects containers, parent↔child relationships throw at parse).

`xtask oracle` fixes surfaced by the new golden: `normalize_reference_output` preserves a
missing trailing newline (M0 goldens had an artifact `\n` — the reference emits none), and
`find_fixture` now also scans `testdata/probes`. Both goldens regenerated EOF-correct.

### P1-M2 design notes (2026-11-14, from reference source `ad34036`)

Core-model facts verified before coding (`agents/plan/FEATURES.md` corrections inline
in its tables where noted):

- **Ids are strings in the model**, not ints: `ModelItem implements Comparable`
  with `Integer.parseInt` both → numeric compare, else string compare;
  `Model.getElement(id)` is a Java `HashMap<String,…>` (TEXT equality lookup);
  legacy workspaces may carry non-numeric ids and
  `SequentialIntegerIdGeneratorStrategy.found()` silently ignores those.
  → `core::ids::Id { text: Arc<str>, num: Option<i32> }`; Eq/Hash/Ord = text;
  numeric view cached for the counter bump and the M3 TreeSet-order comparator.
- **Tags**: `getTags() = LinkedHashSet(getDefaultTags() ++ stored)`; the stored
  field keeps only custom tags in memory but JSON loads re-store the defaults
  (harmless dedup on write). `setTags(csv)` = `split(",")` NO trim (bug);
  `addTags` trims. Relationship defaults are STATEFUL: linked rel → NO defaults;
  else `Relationship` (+`Synchronous`/`Asynchronous` if interactionStyle set).
  Instances (`SoftwareSystemInstance`/`ContainerInstance`): NO inherited-parent
  tags in v6 — FEATURES §3 "parent system's tags + …" is OUTDATED; the ctor
  stores just the instance tag, and JSON load keeps exactly the tags string.
- **Canonical names are URI-style** (FEATURES §2 `Person[X]` table is OUTDATED):
  `Person://X`, `SoftwareSystem://S`, `Container://S.C`, `Component://S.C.K`,
  `Custom://X`, `DeploymentNode://ENV/A/B`, `InfrastructureNode://ENV/A/B/N`,
  `SoftwareSystemInstance://ENV/A/B/System[instanceId]`,
  `ContainerInstance://ENV/A/B/Sys/Container[instanceId]` (note: the node-chain
  drops the `DeploymentNode://` prefix; instanceId is the instance's OWN int,
  the static link is `softwareSystemId`/`containerId` — FEATURES §2 "instances
  link via instanceId" is OUTDATED). Every name part has `.` and `/` REMOVED
  (`formatName`) before joining; static separator `.`, deployment separator `/`.
- **v6.2.4 field truth** (JSON-FORMAT "verify" items): no `background` on View;
  ElementView = `{id,x,y}` only (+nothing — no width/height/z/orientation);
  RelationshipView = `{id, description, url, order, response, vertices[],
  routing, jump, position, properties}`; `numberOfInstances`/`dependencies`
  DO NOT EXIST; health checks live on INSTANCES only (not nodes/infra nodes);
  DeploymentNode has `instances` (int in JSON), `technology`, `deploymentGroups`,
  `platform`/`usage` DO NOT EXIST; animations only on DeploymentView+CustomView;
  FilteredView `tags` is a TreeSet (serialized ALPHABETICAL); workspace-level
  `configuration` = `{scope?, visibility?, users?}` (v6 `WorkspaceConfiguration`,
  legacy `branding`/`styles` inside top-level `configuration` in old goldens are
  IGNORED by the reader — `views.configuration` is the real styles home);
  `ImageView` extends View (NOT ModelView — no dimensions/elementViews);
  DynamicView has `elementId` + `deploymentEnvironment` + `externalBoundariesVisible`.
- **Validation catalog** = `Model.hydrate()` then `ViewSet.hydrate(model)`
  (messages + order transcribed in `core::validate`, fixtures
  `testdata/validation/*.json` + expected messages from
  `structurizr-client` `WorkspaceRulesValidationTests`). Model checks run in
  hydrate order: unique element ids → unique relationship ids → instance static
  links → person/system name uniqueness → containers → components → top-level
  nodes (per environment) → recursive children → rel description uniqueness
  (per source, by (destination, description)) → linked-rel ids exist. ViewSet
  checks: view-key uniqueness (walks a TreeSet ordered by (order, key-ci), the
  `ViewKeysAreNotUnique` fixture relies on distinct `order` to survive the set) →
  per-view-type element links → element/relationship view entries exist →
  filtered base view exists and is static/deployment → image view element.
  First violation aborts (reference throws); we collect ALL (D1) but keep rule
  ORDER stable.

### P1-M2 — core domain model + validation catalog ✅ (2026-11-14)

Landed (`cargo test -p oxidizr-core` green on the first run — the design notes
above held, no errata):

- `crates/oxidizr-core`: ten modules (`ids`, `tags`, `canonical`, `model`,
  `views`, `docs`, `configuration`, `error`, `validate`, `builder`) over a
  dependency-free core (P1-D1; `serde_json` is a **dev**-dependency for the
  fixture test only). `lib.rs` hosts the workspace default name/description
  consts plus `DEFAULT_ENVIRONMENT`/`DEFAULT_INSTANCES`.
- 20 unit tests + `tests/validation_catalog.rs`: a deliberate mini-reader over
  `serde_json::Value` (flattens nested `relationships` arrays like
  `JsonReader` + `Model.hydrate` do — M3 replaces it) loads all 16 vendored
  `testdata/validation/*.json` fixtures; each asserts the EXACT
  `WorkspaceValidationException` message `WorkspaceRulesValidationTests`
  expects, the "different environments" fixture asserts NONE, and
  `every_fixture_is_covered` fails if a fixture appears without an expectation.
- Full gates: 78 workspace tests, `cargo fmt --check`, clippy `all`+`pedantic`
  under `-D warnings` all clean.

Facts verified against the reference **while coding** (all now encoded in
`validate.rs`/`builder.rs` with source references):

- Validation catalog = `Model.hydrate()` then `ViewSet.hydrate(model)`;
  per-view check order custom → landscape → context → container → component →
  dynamic → deployment → filtered → image; filtered views get NO
  element/relationship entry checks; every violation is collected (D1) and the
  FIRST message is what the reference throws.
- Exact strings verified (fixtures assert them): `A view with the key key
  already exists.` (key unquoted), the filtered-view base-view message
  (`(key=SystemContext)` unquoted), the instance-link messages
  (`(id=1) … that does not exist in the model`).
- Multiple violations: the reference's exact winner depends on Java `TreeSet`
  iteration order — with one violation per fixture our first message is
  byte-identical; noted in `validate.rs` header for multi-violation work.
- Builder API names follow the reference (`add_person`, `add_relationship`, …);
  `IdGenerator::generate` (not `next`) to avoid `should_implement_trait`;
  the interim view-order rule is `views.size()+1` (V-4 revisits it in M4).

### P1-M1 — DSL lexer + lossless CST ✅ (2026-11-14)

`oxidizr-diag` (Span/FileId/Code/Collector), `oxidizr-ast` (lossless
`Cst`/`Node`/`NodeKind` + byte-coverage `verify_lossless`), and the
`oxidizr-dsl` front stages: `preprocess` → `lexer` → `cst` (M4 adds the
statement parser on top).

Pipeline shape (P1-D3 realized):
- `preprocess::preprocess(source) -> Vec<Logical>` — the reference's
  `preProcessLines`: physical lines are byte-spanned (`\r?\n`, EOF),
  `\\`-continuations join into segment lists, text blocks fold into a
  `TextBlock` raw region, EOF tails of unfinished groups are marked `dropped`
  (the reference discards them).
- `lexer::lex_line/comment/blank` — bug-compatible `Tokenizer.java` over the
  UNtrimmed bytes: token VALUES equal the Java tokenizer's, leaves tile every
  physical byte (whitespace/newline/continuation/BOM trivia; token hull leaves
  include quote delimiters, multi-segment tokens produce multi-leaves with
  exact pieces).
- `cst::build` — classification + block-comment state machine (see checks
  below) → flat lossless tree `Root > line nodes > leaves`; `cst::LineIndex`
  gives (line, char-col) for diagnostics (D1).

**Reference-semantics corrections found during M1** (verified against
`ad34036` `StructurizrDslParser.preProcessLines` + `Tokenizer.java`; the
earlier notes in this file were wrong on all four — `agents/plan/DSL.md` must be
re-checked against these when M4 documents statements):
1. **Continuation joins keep indentation**: only the FINAL completing line is
   `stripLeading`ed; middle lines append verbatim (join of
   `a \` / `  b \` / `  c` is `a   b   c`).
2. **Logical lines are emitted under the LAST physical line's number** — the
   parse loop flushes with its already-incremented `lineNumber` (multi-line
   statement errors point at the closing line).
3. **Text-block values include the opener line's post-`"""` text**: the
   reference buffers the WHOLE opener line, so `split(TEXT_BLOCK_MARKER)`
   leaves `parts[1]` = tail-after-first-`"""` + dedented middle lines each +
   `\n`; then `substring(0, len-1)` drops the last char. Value = that.
   Throwing inputs (empty value, middle line shallower than first-line indent)
   are modeled as `text_block_value() -> Option<String>` returning `None`
   (M4 will report them as diagnostics instead of Java's
   StringIndexOutOfBoundsException — DEVIATIONS candidate).
4. **Text-block checks precede blank/comment checks** in the reference loop,
   and the whole-line-comment pattern is `^\s*(//|#).*$` (`#` included, like
   scripts — `agents/plan/DSL.md` "no `#` comments" is WRONG; recorded below).

Other verified facts encoded in the code:
- Block-comment machine check ORDER is load-bearing: whole-line `/* … */`
  neither opens nor closes a region (so it does NOT close an open one); a
  line whose FIRST token (after `x =`-assignment strip, reference L431)
  starts `/*` opens; the next line whose TRIMMED text ends `*/` closes; a
  `//`-comment line's `*/` never closes; unclosed `/*` swallows to EOF.
- Tokenizer: quote chars stripped; `\"` = prev-char-backslash lookbehind with
  the backslash KEPT in the value; `"` mid-bare-token is ordinary; EOL pushes
  the open token silently (unterminated `"` loses its opener, no error);
  `#`/`//` are ordinary tokens mid-line; `String.trim` (≤ U+0020) vs
  `Character.isWhitespace` sets both ASCII here (compat.rs notes the Unicode
  edge).
- BOM (U+FEFF) is stripped per logical line (reference) — covered as a `Bom`
  trivia leaf; `iso-8859.dsl` is the one non-UTF-8 fixture: M1 lexes `&str`,
  byte-level decoding is the M5 loader (V-8), pinned by a test.

D1 deviation (already locked): parser errors will carry LINE + COLUMN (the
reference emits no column); M4 wires `Collector` into the statement parser.

**Gates**: `cargo fmt --check` · `clippy -D warnings` clean · `cargo test
--workspace` = 54 tests green (dsl 34 unit + 9 integration incl. six
expect-test snapshots over multi-line/text-block/utf8/workspace-with-bom*/
multi-line-with-error + two whole-corpus properties: every UTF-8 fixture's CST
is byte-lossless, tokens deterministic & char-aligned).
xtask/clippy note: workspace lints catch `push_str(&format!…)`
(clippy::use_format) and `unused dependency` manifest warnings are NON-BLOCKING
(they stay until M2–M4 use the deps; `-D warnings` does not fail on manifest
warnings — verified).


### P1-M0 (2026-11-14, in progress)

Scope (P1-D11 M0): workspace + 7 crates, `rust-toolchain.toml`+`NIGHTLY.md`, LICENSE
files + metadata, `docs/diagnostics.md` stub, lints, CI (3-OS + bench + coverage +
reference jobs), criterion skeleton + committed baseline, xtask oracle v0 (gen ONE
fixture) + `vendor`, testdata vendored with attribution. flake.nix: DONE (9d0498e).

- Pin: rust-toolchain.toml channel `nightly-2026-11-13` == the flake-provided toolchain
  (rustc version string lags the channel date by 1 day: `…(8eb778d04 2026-11-12)`).
  Both bumped together at milestones, register in NIGHTLY.md.
- rust-version = 1.85 (edition-2024 floor; no nightly features used in P1 code so far —
  see NIGHTLY.md).
- `.cargo/config.toml`: `[alias] xtask = "run --package xtask --"` (matklad pattern)
  makes the mandated `cargo xtask …` spelling work; committed (dev tooling, not product
  config).
- Toolchain quirk (nightly-2026-11-13): cargo now emits manifest-level `unused
  dependency` WARNINGS for declared-but-unused deps (skeleton crates; P1-D9 set declared
  up front). Not errors — `-D warnings` unaffected; each retires when its milestone
  lands the code. Skeleton libs carry `#![allow(unused_crate_dependencies)]` with
  remove-when notes for the rustc lint side.
- xtask fixes made while running it for real: `Path::extension()` has no dot (walk_files
  `.dsl` comparison — counted 0 fixtures); `scan_deferred` must be lossy (iso-8859.dsl
  is not valid UTF-8, V-8).
- Clippy surfaces that needed the honest fix: clap default rename would be
  `dsl2-json` → `#[command(rename_all = "lowercase")]` on the `Command` enum (surface
  names are Locked); `doc_markdown` wants backticks around `PHASE_1`, `NO_COLOR` etc.
- Vendored (107 `.dsl` + 21 JSON): deferred-for-D7 list is exactly
  `dsl/big-bank-plc/model/internet-banking-system/details.dsl`, `dsl/decisions/workspace.dsl`,
  `dsl/docs/workspace.dsl`, `dsl/include/model/software-system/model.dsl` (cross-checked
  vs raw grep). Validation corpus = 16 JSON. read-corpus workspaces carry round-tripping
  `lastModifiedDate*` (deterministic — no golden normalization needed).
- V-10 golden: `testdata/golden/workspace-properties.json` generated through the oracle
  (normalized: 4 `structurizr.inspection.*` lines removed, last-entry comma fixed —
  exercised for real: they DO sort last here) + `cargo xtask oracle verify` → ok.
- M0 gates: fmt / clippy(-D warnings) / test all green on the pinned toolchain; bench
  baseline committed under `bench-baseline/M0/`.

### P1-M1 design notes — SUPERSEDED

The pre-implementation notes written from `ad34036` before coding contained four
errors (indentation-stripping, first-line numbering, text-block value bounds, and
`#`-comment scope), all corrected in the **P1-M1 milestone log above**, which is
authoritative for the implemented semantics. The verified facts are encoded and
tested in `crates/oxidizr-dsl/src/{preprocess,lexer,cst}.rs`; see also
`agents/plan/DSL.md` §1 "M1-verified corrections".

Kept (unverified, M4 must confirm against source before relying on them): the
`${VAR}` pre-pass is a per-token, forward single pass over constants + variables +
text blocks with regex `\$\{[a-zA-Z0-9-_.]+?}` (defs must precede uses; unknown
names stay literal unless the env feature is on — V-3 `--env`); text-block values
substitute recursively, constants/variables do not.
