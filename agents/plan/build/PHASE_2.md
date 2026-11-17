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

Java `getViewOrViewSetProperty` equivalents are read by all exporters
(`c4plantuml.stdlib`, enterprise name, `useAlphanumericShortIds`-style toggles, …).
Deliverable **`agents/plan/build/EXPORT-PROPERTIES.md`** (same pattern as WRITE-CATALOG): <!-- refs:ignore forward reference, created by P2 -->
exhaustive grep of property-name string literals in the four exporters → table
property → read site → default → cascade order (verify order in Java) → observable
effect. Implemented as one typed function `export_property(ws, view, name)` — no
scattered `properties.get("…")` calls.

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
  (P2-D8), P2-V1/V2 verifications recorded.
- **M2** Structurizr-PlantUML exporter complete (all supported view types, legend,
  boundaries/groups, properties); Category A goldens + render smoke (comparator comes
  in P4 — P2-D9).
- **M3** C4-PlantUML exporter (light/dark, stdlib property, sequence path); semantic
  comparator built + oracle run green (the phase's reference-facing milestone).
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
- [ ] P2-V2 per-exporter supported view-kind matrix + full property-name list + output
  micro-grammar (drives comparator design) — from source at pinned commit.
- [ ] P2-V3 icon/logo URL handling in generated PlantUML/Mermaid (pass-through? fetched?
  omitted?) — decide mirror behavior, keep export-time zero-network.
- [ ] P2-V4 `STRUCTURIZR_THEMES`/theme dir layout + `theme "<name>"` URL mapping
  (`static.structurizr.com/themes/<name>/`) — mirror lookup semantics.
- [ ] P2-V5 color-scheme defaults (light/dark constant values) in each dialect.
- [ ] P2-V6 legend diagram generation rules (when emitted, content per dialect).
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
