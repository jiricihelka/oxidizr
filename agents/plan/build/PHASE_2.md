# PHASE_2.md — detailed implementation plan: exporters (diagram formats)

Scope per PLAN.md Phase 2, as decided: **exporters only** — Structurizr-PlantUML,
C4-PlantUML (light/dark), Mermaid, theme JSON. The three diagram importers
(C4-PlantUML, Mermaid, image→CustomView) moved to Phase 4; WebSequenceDiagrams too
(kept out of P2 despite the reference CLI wiring it — dynamic views are already covered
by the PlantUML sequence path; see DEVIATIONS errata note in `agents/plan/README.md`).

**Fidelity bar (agreed): semantic parity + render checks — NOT byte-parity.** Exported
text must declare the same entities/relationships/styles/includes as the reference
export (structure-based golden diff via the oracle) and render successfully with the
target tools. Formatting freedom exists; we still *aim* to mirror the reference output
because that is the cheapest way to pass the semantic comparator, but incidental text
differences are not gate failures. The JSON compat contract (DEVIATIONS.md Locked list)
is untouched by anything in this phase.

Reference sources: `structurizr-export/src/main/java/com/structurizr/export/`
(port each exporter 1:1 as a module — file-name correspondence kept on purpose so
oracle diffs are reviewable against Java) and `structurizr-application/.../ExportCommand.java`
(format strings, theme pre-load, output naming).

---

## P2-D1 Crate topology: `oxidizr-export` (new member)

```
crates/
  oxidizr-export   deps: core, json (theme.json de/ser), diag. NO dsl/ast deps.
```

- Public API mirrors the reference abstraction:

```rust
pub struct Diagram { key, definition: String, file_extension: &'static str,
                     frames: Vec<Diagram>, legend: Option<Diagram> }
pub trait DiagramExporter { fn export(&self, ws: &Workspace, theme: &Theme)
                              -> Result<Vec<Diagram>, ExportError>; }
pub trait WorkspaceExporter { fn export(&self, ws: &Workspace) -> Result<String, …>; }
```

- `IndentingWriter` equivalent: writes `write!` into a reusable `String` buffer
  (agents/plan/WORKFLOW.md perf rule — no per-line `String` allocations); exported once in `export`
  crate, used by all dialects (mirrors Java `com.structurizr.export.IndentingWriter`).
- `oxidizr-import` (P4) will be a sibling crate; nothing in P2 depends on it.

## P2-D2 Style resolution lives in `oxidizr-core::styles`

The single most reused piece of this phase: computing the **effective style** for an
element/relationship view from (theme styles ⊕ workspace `styles {}`, matched by tag,
workspace styles overriding theme, later style rules overriding earlier for the same
tag) plus color-scheme selection.

- Pure computation in `core` (no IO): `resolve(theme: &Theme, styles: &Styles,
  tags: &Tags, scheme: ColorScheme) -> ElementStyle` — consumes plain-data `Theme`
  types that also live in `core` (core stays dependency-free, P1-D1 unchanged).
- **theme.json de/serialization** in `oxidizr-json` (it is the serde home; theme format
  is read-only for us + written only by the `theme` export, see P2-D7).
- **Theme loading/caching/network** in `oxidizr-export::themes::ThemeStore` (P2-D3).
- Phase 3 GUI renders through this exact code path — one resolution implementation,
  so GUI and exporters can never disagree on styling.
- Unit-test matrix: tag-match precedence, multi-tag stacks, theme-vs-style override,
  `Icon`/logo URL resolution against theme base (FEATURES.md trap 10).

## P2-D3 ThemeStore: offline-first + `--allow-net` ships here (D5)

Resolution order for `themes { url … }` / `theme "<name>"`:

1. `--themes-dir <dir>` (same layout as `STRUCTURIZR_THEMES`: `<dir>/<name>/theme.json`);
   `OXIDZR_THEMES_DIR` env var honored when the flag is absent (flag wins).
2. Local cache: `<xdg-cache>/oxidizr/themes/<sha256(url)>/theme.json` (hand-rolled
   XDG/APPDATA/LocalAppData resolution — std + env only, per portability rule; one
   small `paths` module in `export`, justified cfgs commented).
3. Network fetch **only with `--allow-net`** (this flag is introduced in P2 and applies
   to theme URLs only; DSL `!include <url>` continues to work per P1 once flag set —
   D5 register updated at implementation). Without the flag: clear OXZ-5xxx error
   naming the URL and the `--themes-dir` escape hatch.
4. Default theme: **always available offline**, vendored `theme.json` + fonts/assets
   via `include_bytes!` in `export` (Apache-2.0 attribution, source version recorded
   in `testdata/README.md` alongside goldens).
   **Amendment 2026-11-17 (M1 research):** transport for item 3 = system `curl`
   via `std::process::Command` (std has no TLS; no external crates) — `curl`
   absent is a clean error, never a panic; the cache-key hash (item 2) is a
   hand-rolled FIPS 180-4 SHA-256; the install-source order (incl. the
   reference's `<workspace-parent>/themes` and `STRUCTURIZR_THEMES` fallbacks)
   is cataloged in `EXPORT-PROPERTIES.md` §6.2. Item 4 stays blocked on this
   machine's missing DNS: vendoring lands as soon as the user provides
   `static.structurizr.com/themes/default/theme.json`; interim behavior is the
   D3 rule (unknown theme name = keep working, styles simply unresolved).

Relative `icon`/`logoUrl` resolution against the theme's base URL/dir per reference
(verified behavior — catalog item P2-V3 covers what exporters do with icon URLs).

## P2-D4 CLI surface (`oxidizr export`)

- **Reference behavior (monorepo source + oracle, verified at M0 2026-11-17):**
  `structurizr export --workspace <file> --format <F> [--output <dir>]` — there is
  NO `--prefix` (the prefix is `structurizr-<id>` via `ExportCommand.prefix`, bare
  `structurizr` when the id is not positive), no `--view`, no `--light|--dark` flags
  (color is a `-light|-dark` **suffix** on the format string). Format strings are
  lowercased and matched against the registry: `json`, `theme`,
  `plantuml[/structurizr|/c4plantuml][-light|-dark]`, `mermaid`,
  `websequencediagrams`, plus the `static` branch and the `png`/`svg` branches
  (rejected: the reference build ships no Playwright). An unresolvable string takes
  the class-load attempt: logs `Unknown export format: <f>` and the command
  **exits 0**. The output directory defaults to the canonicalized workspace parent
  and is created **before** format dispatch. `ThemeUtils.loadThemes` runs only for
  `DiagramExporter` formats (the json/theme exports do NOT pre-load themes).
- oxidizr surface as landed (M0): `oxidizr export -w/--workspace <file>
  -f/--format <F> [-o/--output <dir>]` (format required — clap usage error, exit
  2). Unknown format mirrors the reference: `Unknown export format: <f>` note on
  stderr, exit 0 (the earlier "diagnostic with the valid list" is superseded — the
  reference decides). Known-but-unlanded exporters = OXZ-5007, exit 1.
  `--allow-net` / `--themes-dir` join at M1 with the `ThemeStore` (P2-D3).
- **Output file naming matches the reference** (verified):
  diagrams `structurizr-<workspaceId>-<viewKey>.<ext>`, legend `…-key.<ext>`,
  animation frames `…-<n>.<ext>` (frames carry the PARENT key); workspace exports
  (json/theme) write `<stem>[-theme].json` — the workspace file name up to its
  LAST `.`, `-theme` appended for the theme format only (supersedes the earlier
  "single `theme.json`" wording; a `.`-less file name is the reference's
  `StringIndexOutOfBoundsException` — oxidizr reports an I/O error instead).
  Rationale: drop-in for CI pipelines that glob the reference names.
- Workspace load reuses the P1 pipeline (extension auto-detect); theme pre-load
  happens before diagram export exactly where the reference does it (M1).
- Exit codes unchanged from P1-D8. `export` added to the mandated criterion bench list.

## P2-D5 Structurizr-PlantUML dialect (`plantuml/structurizr`)

Port `StructurizrPlantUMLExporter` + shared `AbstractPlantUMLExporter` logic:
per-view-type handling (system landscape/context/container/component/deployment,
dynamic as sequence, filtered), boundaries and groups as nested `rectangle`/
`Person_Boundary` blocks, per-element `skinparam<<Tag>>` styling generated from
resolved styles (P2-D2), relationship syntax with `[[url]]`, dashed types, `<<…>>`
stereotypes from tags, `legend` diagram support, `!theme`-free output (self-contained).
Auto-layout settings are ignored (target renderer lays out); geometry is NOT emitted.
Property reads go through the cascade of P2-D9 (e.g. enterprise-name prefix handling).
Which view kinds each dialect actually supports (e.g. landscape in C4 dialect) is
cataloged empirically — P2-V2 — do not port from memory.

## P2-D6 C4-PlantUML dialect (`plantuml/c4plantuml`, light/dark)

Port `C4PlantUMLExporter`; behaviors verified at the pinned commit, replicate exactly:

- Include selection: `c4plantuml.stdlib` property (view → viewset → workspace cascade,
  **default `"true"`**) selects `!include <C4/C4_…>` stdlib form vs raw
  `https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_….puml`
  URLs. Include set accumulates by view content: `C4` + `C4_Context`, plus
  `C4_Container` iff any container/container-instance, `C4_Component` iff any
  component, `C4_Deployment` for deployment views; dynamic views take the
  `C4_Sequence` path (`renderAsSequenceDiagram` condition ported verbatim).
- Dark scheme: `<style>` block with `BackgroundColor`/`FontColor` from the dark
  defaults before includes; light emits the light defaults (constants mirrored).
- Macro invocation generation (`Person(…`, `System_Ext(…`, `Container(…`,
  `Rel(…` incl. tagged variants `Person2(…` for styled elements), boundary macros,
  enterprise-name prefixing — all via the property cascade.

## P2-D7 Mermaid + theme exporters

- `MermaidDiagramExporter` port: C4 flowchart syntax generation, per-view handling,
  `linkStyle`/classDef from resolved styles. Feature matrix of supported view kinds +
  syntax details cataloged from the Java source during the port (P2-V2 covers it).
- `theme` export = port of `JsonWorkspaceThemeExporter` → `ThemeUtils.toJson(workspace)`:
  **exact semantics of the emitted theme JSON are a verification item (P2-V1)** —
  verify at the reference what it serializes (resolved theme vs synthesized-from-
  styles theme) and mirror; golden-checked semantically via the oracle.

## P2-D8 Export property cascade

Java `getViewOrViewSetProperty` equivalents are read by all exporters.
Deliverable **landed 2026-11-17**: `agents/plan/build/EXPORT-PROPERTIES.md`
(same pattern as WRITE-CATALOG) — exhaustive literal grep at the pin: property →
read site → default → cascade → observable effect, plus the view-iteration/
support matrix (§5), the theme-loading chain (§6) and the resolution constants
(§7). Source-verified corrections to the early wording: the cascade is TWO
levels + default (view → `views.configuration` → default; model properties are
NOT consulted, and no enterprise-name/short-id property reads exist at this
pin); match case-sensitivity differs per property (`plantuml.title`,
`mermaid.title`, `mermaid.icons` use case-SENSITIVE `equals`). Implemented as
one typed function `export_property(view, views, name, default)` in
`oxidizr-export` — no scattered `properties.get("…")` calls.

## P2-D9 Fidelity harness (replaces byte-parity goldens)

- **Semantic comparator — C4-PlantUML only in P2** (2026-11-14 rebalance):
  `xtask export-verify` (Category B, default-off `reference` feature, run sparingly)
  runs reference `structurizr export --format plantuml/c4plantuml` (GNU-style options;
  see agents/plan/TESTING.md §Test data & goldens) (via
  `$OXIDZR_STRUCTURIZR_BIN` discovery) and oxidizr over the fixture set; parses both
  outputs into a normalized fact model (macro calls + args, relationship tuples,
  style/skinparam directives, include lines; order-normalized per rules where the
  target syntax is order-insensitive) and diffs. Normalization whitelist is minimal and
  explicit (header comment lines only). ≈ 250 lines of tolerant line-grammar parsing in
  `xtask/src/exportcmp/` — its grammar table doubles as the C4 macro inventory that
  P4's importer builds on. Comparators for Structurizr-PlantUML and Mermaid move to
  **P4-D13**; until then those dialects rely on the two mechanisms below.
- **Own goldens** (Category A, all dialects): `testdata/exports/`, per dialect × <!-- refs:ignore forward reference, created by P2 -->
  view-kind matrix from fixtures — byte-diff against OUR OWN stored output; catches
  unintended drift of our formatting even though the contract is semantic.
- **Render checks** (Category B, skip loudly; all dialects): when
  `$OXIDZR_PLANTUML_BIN` (or `plantuml` on PATH) / `$OXIDZR_MERMAID_BIN` (or `mmdc`)
  exist, render the full export corpus and assert exit 0 + non-empty output file
  (smoke, not pixel comparison). For the non-C4 dialects this is the only
  reference-facing check in P2, so it runs in the reference job, not just
  opportunistically.
- proptest: exporters never panic on valid random workspaces; key-invariant/bench
  suites extended.

## P2-D10 Milestones (ordered, each ends green)

- **M0** `oxidizr-export` scaffold, `IndentingWriter`, `Diagram` types, CLI `export`
  command skeleton with format dispatch + file naming; theme.json de/ser in `json`;
  default theme vendored. **DONE 2026-11-17 (commit ca292e7)** — one deferral: the
  vendored default theme needs `static.structurizr.com` (no DNS on this machine)
  and the file is NOT in the pinned monorepo; the vendoring joins M1 with the
  `ThemeStore` (P2-D3 item 4), the P2-D3 D3 rule (keep every theme name) covers the
  interim.
- **M1** `core::styles` resolution engine + `ThemeStore` (offline dirs, cache,
  `--allow-net`, error UX) with full unit suites. EXPORT-PROPERTIES.md catalog first
  (P2-D8), P2-V1/V2 verifications recorded. Decomposed 2026-11-17 (>4 modules):
  - **M1-a** `EXPORT-PROPERTIES.md` catalog (P2-D8) + P2-V2/V4 evidence — **DONE
    2026-11-17** (this commit).
  - **M1-b** `oxidizr-core`: `Styles.themes: Vec<Theme>` (not serialized) +
    `find_element_style(tag|element, scheme)` / `find_relationship_style(…)
    ` resolution per `EXPORT-PROPERTIES.md` §7 + catalog §6.1-3 (defaults,
    tag-composition order, `copyFrom` rules, darker-stroke `#%06X`, linked-
    relationship tag chain) + unit matrix. Touches: core (+views.rs), tests.
  - **M1-c** `oxidizr-export::themes` `ThemeStore` + `paths` module (XDG/APPDATA
    cache, hand-rolled SHA-256) + installed-name registry + URL branch behind
    `--allow-net` (curl delegate) + vendored default theme IF the file is
    supplied (else D3 interim rule) + unit tests (offline fixtures). Touches:
    export, diag (reuses 5001), core (`Theme` reuse).
  - **M1-d** wiring: registry built in the pipeline BEFORE DSL parse
    (`AbstractCommand.installThemes` parity — parser's `add_theme` gains the
    installed-name branch); `loadThemes` equivalent in the diagram branch of
    `export_cmd`; CLI flags `--themes-dir`/`--allow-net` + `OXIDZR_THEMES_DIR`
    (`ApiOptions`); D5 register row; oracle A/B probe with a local
    `STRUCTURIZR_THEMES`-style install. Touches: api, cli, dsl (one branch),
    docs, PROGRESS/RESUME. **DONE 2026-11-17 (this commit)** — registry reaches
    the parser via `ParseOptions.installed_themes` (Arc-closure newtype, dsl
    crate stays FS-free per P1-D1); `add_theme` installed branch mirrors
    `Configuration.addTheme` (raw-token registry check, then trim + append
    WITHOUT dedup — only the URL branch dedups); `loadThemes` runs in
    `export_cmd` for `ExportFormat::is_diagram()` formats BEFORE dispatch
    (reference order: theme failures precede the format's OXZ-5007;
    `ThemeError::NetDisabled` → 5001, others → 5008); `oxidizr-api/tests/
    export.rs` pins all four legs. Oracle A/B (`/tmp/nix-shell.uni100/p2/m1d`):
    `themes` arrays byte-identical (`["probe", <url>]` with a
    `STRUCTURIZR_THEMES` install + `-f json`); the probe surfaced two
    PRE-EXISTING app-layer load-parity gaps (both also on the P1-era dsl2json
    path, corpus-immune because vendored fixtures define views and end without
    a final newline) → **M1-e**.
  - **M1-e** load-path parity, found by the M1-d oracle probe 2026-11-17 — the
    shared `AbstractCommand.loadWorkspace` mirror (dsl2json/json2json/export
    all go through it):
    1. **default views**: the reference creates them at the app layer when the
       workspace defines none (`AbstractCommand` 109-112 → `Views.
       createDefaultViews`); oxidizr creates none anywhere (Locked list: "default
       views generation only where the reference does it (app layer)" — this is
       a must-fix). Port needs the generation order/keys (`SystemLandscape-001`
       order 1, `SystemContext-NNN` per software system order 2, … witness in
       `m1d/ref/workspace.json`) + per-load call site + oracle A/B on a
       no-views workspace. **DONE 2026-11-17 (this commit)** — core
       `WorkspaceBuilder::create_default_views` (viewops, mirroring
       `ViewSet.createDefaultViews` 1191-1264: Java UTF-16 stable name sorts;
       container views get `TopBottom` auto-layout 100/50/50/vertices;
       deployment environments deduped first-seen then sorted, scoped per
       container-instance system, unscoped only for infrastructure-only
       environments); pipeline `apply_default_views` (`adopt`-round-trip,
       condition `!model.isEmpty() && views.isEmpty()` verbatim) on the dsl2json
       serialize, json2json parity leg and export loader. `validate` is NOT
       hooked: its violations mirror the reference steps that run BEFORE the
       load tail (`WorkspaceScopeValidator`, `DefaultInspector`) and the
       reference `ValidateCommand` never serializes views; the `keep_unknown`
       D3 leg stays verbatim (documented non-parity). A/B: `m1e/ws/
       workspace2.dsl` (name-disordered systems, containers/components,
       containerInstance + softwareSystemInstance subtlety, scoped +
       infra-only environments) and the `m1d` witness — both byte-identical to
       the reference modulo the per-process identifier UUIDs and the
       `structurizr.inspection.*` properties.
    2. **retained `structurizr.dsl` trailing newline**: the reference appends
       RAW lines as `readLine` yields them (a final file newline can never
       enter the retained source); oxidizr joins `source.split('\n')` — the
       trailing `""` survives, so files ending with `\n` get base64 `…Cg==`
       where the reference has `…fQ==`. Vendored fixtures end `\n}` (no final
       newline) so no gate could ever catch it. Mirror `readLine` semantics in
       `dsl_source_lines` (engine/mod.rs 1218). **DONE 2026-11-17** (`0759f96`:
       `compat::java_split_dsl_lines` mirrors Java `split("\\r?\\n")` — one CR
       per separator, ALL trailing empties dropped; base64 delta closed
       against `m1d/ref`).
- **M2** Structurizr-PlantUML exporter complete (all supported view types, legend,
  boundaries/groups, properties); Category A goldens + render smoke (comparator comes
  in P4 — P2-D9). Decomposed 2026-11-17 (>4 modules; sources at the pin:
  `export/plantuml/StructurizrPlantUMLExporter` 757 + `plantuml/
  AbstractPlantUMLExporter` 408 + `export/AbstractDiagramExporter` 848 +
  `AbstractExporter` 118 + 8 `PlantUMLStyle` classes):
  - **M2-a** micro-grammar catalog `EXPORT-PLANTUML.md` (the P2-V2 deferred
    half, from the sources above; P2-V5/V6 Structurizr-dialect halves get
    checkboxes + evidence here) + `export_property(view, views, name,
    default)` typed helper implementing P2-D8's two-level cascade in
    `oxidizr-export` + DEVIATIONS D12 row (P2-V3 decision: scale pinned
    `0.5`, zero-network). Touches: export (one helper), plan docs. **DONE
    2026-11-17** — D12 `be26b56`, `export_property` + 4 cascade tests
    `d179653`, catalog `EXPORT-PLANTUML.md` §§1–13 (§10 legend/P2-V6,
    §12 colors/P2-V5, §13 quirk register incl. the `findGroupStyle` 711
    icon←color upstream bug we must mirror); engine iteration section is
    appended with the M2-b port.
  - **M2-b** shared diagram-engine port in `oxidizr-export::plantuml`
    (`AbstractDiagramExporter` + `AbstractExporter` + `AbstractPlantUML
    Exporter` helpers): per-view export methods + iteration order,
    `writeElements` group/boundary nesting, `isVisible` matrix,
    `getRelationshipsInView`, `typeOf` terminology/metadata-symbols,
    `idOf` parent-chain + `filter` (Java `(?U)\W`) + instance `_n` ids,
    skinparam/include writers, sequence-diagram export path skeleton.
    Touches: export (+core only if terminology lookups are missing). **DONE
    2026-11-17** — `oxidizr-export::plantuml` = [`Engine`] (workspace +
    color scheme + untyped `frame` as `Frame::Step`/`Frame::Order`) driving
    a [`Dialect`] trait; per-kind bodies incl. the nested-group
    context-tracking quirks and the NO-blank-line deployment leg, frames +
    legend lifecycle (legend LAST, §14), dynamic sequence path with the
    canonical-name sibling insertion, `is_visible` matrix (CustomView
    always visible; kind-mismatched frame → `ExportError::ReferenceAbort`
    = new OXZ-5009, registered with the §14 cast-aborts); `type_of`,
    `break_text`, canonical names (`CanonicalNameGenerator`) and the
    `ElemRef` model adapter. Engine iteration contracts appended as catalog
    §14. Java reference identity mirrored by id equality (no `equals`
    overrides). MockDialect tests ×8 pin orders, quirk output, frame
    visibility and the abort message.
  - **M2-c** Structurizr dialect: `PlantUMLStyle` family (root/element/
    relationship/deploymentNode/boundary/group/legend — class selectors,
    Java UTF-16 name sort, HashSet dedup per the Java equals/hashCode),
    header (`title`, direction + ranksep/nodesep math, `hide stereotype`,
    `<style></style>` placeholder → footer `writeStyles` replace), element/
    relationship writers incl. the sequence path, legend diagram
    (`createLegend`). Touches: export.
  - **M2-d** dispatch + proof: `export_cmd` plantuml leg (per-view files +
    legend files + `Writing` lines, filenames per `ExportCommand`),
    Category-A goldens (oracle-generated, normalized), render smoke (skip
    when plantuml absent), oracle A/B byte compare over the corpus set
    (no DNS → the icon-scale `0.5` lands identically). Touches: api/cli,
    testdata, xtask, docs.
- **M3** C4-PlantUML exporter (light/dark, stdlib property, sequence path); semantic
  comparator built + oracle run green (the phase's reference-facing milestone).
  Decomposed 2026-11-17 (milestone spans > 4 modules):
  - **M3-a** recon + witness sweep: full-corpus oracle run `--format
    plantuml/c4plantuml` over the 43-fixture sweep set (witnesses
    `/tmp/nix-shell.uni100/opencode/m2d/sweep/*/ref-c4/`); source pinning of
    `C4PlantUMLExporter` at the pin (header: direction from autoLayout
    rankDirection, shared skinparams, ALWAYS the `<style>` root block with
    P2-V5 constants, include accumulation `C4`+`C4_Context` (+`C4_Container`
    iff containers/instances, +`C4_Component` iff components, +`C4_Deployment`
    iff deployment view; sequence → `C4_Sequence` only), the `c4plantuml.*`
    property register (legend=true, stereotypes=false, tags=false,
    stdlib=true, elementProperties/relationshipProperties=false, per-style
    sprite/shadow), footer `SHOW_LEGEND(…)` + `show|hide stereotypes`;
    base `createLegend` = null → NO `-key` files (witness-confirmed);
    `export(CustomView)` = null → custom views skipped. Touches: plan docs.
  - **M3-b** dialect port `oxidizr-export::plantuml::c4` (`C4PlantUmlDialect`
    on the M2-b `Dialect` trait): header legs + tags-section (when
    `c4plantuml.tags`: `AddElementTag`/`AddRelTag`/`AddBoundaryTag` with the
    Java `HashMap<String,…>` iteration order — String `hashCode`, portable,
    `javahash` pattern), boundary macros (`Boundary(group_N…`,
    `System_Boundary(<id>_boundary…`, `Deployment_Node(… (xN))` incl. the
    `", $borderThickness=\"1\")"` replace quirk), element macros (instances
    unwrap + `$tags` from the instance, `Container/Component{Db|Queue}`
    shape verbs, CustomElement skip), `Rel(…` with the `<order>: ` prefix,
    sequence `box "…\n<size:12>…` leg, `hide stereotypes`/`SHOW_LEGEND`
    footer, and the engine hook for the CustomView skip. Touches: export.
  - **M3-c** dispatch + A/B: `ExportFormat::C4PlantUml` arm (Engine +
    dialect; files per the shared `ExportCommand` loop), byte-compare vs the
    `ref-c4` witnesses across the corpus, root-cause every DIFF. Touches:
    api/cli.
  - **M3-d** proof: c4 goldens under `testdata/golden/plantuml-c4/` via
    `xtask oracle gen --plantuml --c4` + Category-A byte-parity test (extend
    the M2-d golden test); render smoke (stdlib form renders offline —
    plantuml 1.2026.9 bundles C4 stdlib); **semantic comparator**
    `xtask/src/exportcmp/` + `cargo xtask export-verify` (Category B,
    sparingly; the P2-D9 harness — macro-call/relationship/directive fact
    model, order-normalized, minimal whitelist; grammar table doubles as the
    P4 importer's macro inventory). Touches: xtask, testdata, api tests.
- **M4** Mermaid exporter + `theme` export; render checks wired in CI (skipping when
  tools absent); benches committed (export corpus baseline).
- **M5** hardening: proptest panic suite, docs (`oxidizr export --help`, agents/plan/errata
  fold-back), tag v0.2.0.

## P2-D11 Verification list (empirical, before dependent code)

- [x] **P2-V1** `ThemeUtils.toJson` exact output semantics — **VERIFIED
  2026-11-17** (M0; oracle witnesses `/tmp/nix-shell.uni100/p2/v1/out1,out3,
  out5,out6` + monorepo `ThemeUtils`/`Theme`/style sources at the pin):
  plain mapper (NOT the workspace one), `NON_NULL`→`NON_EMPTY` global
  inclusion (theme fields + empty style arrays hidden; empty theme = `{ }`),
  per-field `NON_NULL` on style fields; key order = Java declaration order
  (`name, description, elements, relationships, license`; style: `tag`,
  `colorScheme`, `properties` — the parent-field keys first — then the
  declared fields); name/description from `workspace.getName()/
  getDescription()`; `license` never set; `DefaultIndenter("  ", "\n")`
  pretty print, no trailing newline. Theme content = the LIVE style
  collections in storage order (sorted for DSL-built workspaces, DOCUMENT
  order for JSON-read ones — Jackson binds `Styles.elements` by FIELD; the
  TreeSet-replacing setter that sorts every other collection does not exist
  here). Style numbers pass through the JavaBean setters on read and CLAMP:
  `strokeWidth`→[1,10], `opacity`→[0,100] (both styles), relationship
  `position`→[0,100]. Pinned by `oxidizr-json/tests/theme_json.rs`
  (byte-exact witnesses) + `styles_parity.rs`; CLI `export -f theme`
  reproduces out1/out6 byte-for-byte. Spin-offs folded into WRITE-CATALOG
  §5/§6 (commit 6f6cc64).
- [x] **P2-V2** (matrix + properties part) per-exporter supported view-kind
  matrix + full property-name list — **VERIFIED 2026-11-17** (M1-a):
  `EXPORT-PROPERTIES.md` §1–§5 (shared `final` iteration order
  custom→landscape→context→container→component→dynamic→deployment, FilteredView
  never exported, WSD deployment = null-skip; 15 cascade + 3 style-level
  property literals with read sites, defaults and per-site match
  case-sensitivity; the cascade is 2 levels + default, not 3). The **output
  micro-grammar** half is deferred per dialect to its own port (M2/M3/M4) where
  the comparator consumes it — keeping it here would port syntax from memory.
- [x] **P2-V3** icon/logo URL handling — **VERIFIED 2026-11-17** (M2 recon,
  source): `AbstractPlantUMLExporter.isSupportedIcon` = icon non-empty AND
  starts with `http` (relative/theme-rewritten-relative icons never render as
  `<img:>`); `calculateIconScale` = `0.5` default, else
  `maxIconSize / max(width,height)` from `ImageIO.read(new URL(icon))` —
  the REFERENCE FETCHES THE IMAGE AT EXPORT TIME (network). Failures:
  `UnsupportedOperationException | UnsatisfiedLinkError | IIOException` →
  silent `0.5`; any other exception → `printStackTrace` then `0.5`. This
  machine has no DNS, so oracle A/B always lands on the reference's `0.5`
  fallback. DECISION: oxidizr pins `0.5` (export-time zero-network, per this
  item's wording) — DEVIATIONS D12: scale is network-dependent in the
  reference, deterministic-here equal; scale formatting = Java
  `Double.toString` shortest round-trip when a computed value ever lands.
  (Input side verified at M1-a: `loadThemes` rewrites relative icons to
  `baseUrl + icon` for URL themes only — catalog §6.1-3.)
- [x] **P2-V4** `STRUCTURIZR_THEMES`/theme dir layout + `theme "<name>"` URL
  mapping — **VERIFIED 2026-11-17** (M1-a, source): install = one-level scan of
  `<workspace-parent>/themes` else `STRUCTURIZR_THEMES` env, name = subdir
  containing `theme.json`; `default` → `static.structurizr.com/themes/default/theme.json`;
  unknown non-URL names at JSON load = `log.warn` only; load = URL (content-type
  checked) else installed file else SILENT SKIP; theme styles append to a
  non-serialized `Styles.themes` consulted at resolution time. Catalog §6.1;
  oxidizr source-order mapping §6.2. Runtime A/B probe **RUN at M1-d** (local
  `STRUCTURIZR_THEMES` install + `theme probe` + URL theme + `-f json`):
  `themes` arrays byte-identical ref-vs-oxidizr (witness
  `/tmp/nix-shell.uni100/p2/m1d`); the URL-fetch leg is not
  observable on this machine (no DNS) and is mirrored from source.
- [ ] P2-V5 color-scheme defaults (light/dark constant values) in each dialect.
  **Structurizr half VERIFIED 2026-11-17** (M2-a): `Styles.DEFAULT_*` =
  `#ffffff`/`#444444` light, `#111111`/`#cccccc` dark (`EXPORT-PLANTUML.md`
  §12). C4/Mermaid halves land with M3/M4. (Core-level constants pinned at
  M1-a — catalog §7; the per-dialect skinparam / `$`-default colors come with
  the M2/M3 ports.)
- [ ] P2-V6 legend diagram generation rules (when emitted, content per dialect).
  **Structurizr half VERIFIED 2026-11-17** (M2-a): emitted for EVERY view of
  the dialect (override never returns null; base does), content per
  `EXPORT-PLANTUML.md` §10 (DeploymentNode/Element/Relationship style passes,
  width 200, transparent self-loops, own `<style>` replace). C4/Mermaid halves
  land with M3/M4.
Each gets a checkbox + evidence in the implementing PR; surprises fold into `agents/plan/`
(errata process), per AGENTS.md working style.

## Risks

- R-1 semantic comparator is too weak/too strong (mitigation: start strict, whitelist
  only demonstrated-noise lines; render checks back-stop syntax validity).
- R-2 style-resolution divergence between P2 exporters and P3 GUI (mitigation: single
  `core::styles` engine, shared tests).
- R-3 C4-PlantUML stdlib drift upstream (raw-master URLs in reference output change)
  — we mirror the pinned commit; catalog records the pin date.
- R-4 theme vendoring license/size (mitigation: default theme only; others user-fetched).
- R-5 non-C4 dialect divergence until P4 comparators land (mitigation: own goldens +
  mandatory reference-job render checks in P2; C4 — the flagship format — is fully
  oracle-gated from day one).
