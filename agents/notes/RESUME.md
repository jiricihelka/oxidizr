# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M2-c dialect commit (tip of `main`, tag
`v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0**
  theme/json export byte-parity · **P2-M1 COMPLETE** (`199bc70`–`53d2e5e`) ·
  **P2-M2-a** D12 `be26b56` + `export_property` `d179653` + catalog
  `build/EXPORT-PLANTUML.md` §§1–13 `a12bd29`. **P2-M2-b COMPLETE** (this
  commit): `oxidizr-export::plantuml` — shared diagram engine: `Engine<'a>`
  (workspace + color scheme + `Frame::Step`/`Frame::Order`) drives a
  `Dialect` trait; `model.rs` `ElemRef` adapter; `mod.rs` constants/shape
  maps/`java_cmp_str`. All §14 order contracts, per-kind bodies, nested-group
  quirks, frames + legend LAST lifecycle, sequence path, `is_visible`,
  `type_of`, `break_text`. Java abort paths → `ExportError::ReferenceAbort`
  = **new OXZ-5009 EXPORT_ABORT** (diag const + docs row + pipeline arm +
  coverage REGISTRY + allow-list). 31 tests, MockDialect pins hook sequences.
  **P2-M2-c COMPLETE** (this commit): `plantuml/structurizr.rs` =
  `StructurizrDialect` — `StructurizrPlantUMLExporter` 757 +
  `plantuml/AbstractPlantUMLExporter` 464 writers ported: header (title/skinparams/
  includes/direction/teoz legs), `PlantUMLStyle` enum (base64 selectors,
  name-sorted class-pass `writeStyles` footer replace, per-class `toString`
  field orders), boundary/group/DN writers (synthesized `ElementStyle("")` →
  shadow ALWAYS false; line-711 icon→color bug mirrored), `createLegend`
  (width-200 rewrite, transparent rel lines, appended Legend style),
  `idOf`/`filter` `(?U)\W`, Java-substring group cuts. Engine fixes carried
  here: Result-converted body hooks + the MISSING `end_deployment_node_boundary`
  call (ref `AbstractDiagramExporter` 702, found via DN dump). 42 tests — 11
  pinned full-document/legend assertions + leg asserts.
- **Next:** **M2-d**: `export_cmd` plantuml dispatch leg, per-view + legend
  files, Category-A goldens, render smoke, oracle A/B over corpus
  (`build/PHASE_2.md` spec). Then M3 (C4-PlantUML + `xtask export-verify`
  semantic comparator), M4 (Mermaid), M5 (docs, proptest, tag `v0.2.0`).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — **USER DECLINED (2026-11-17): keep everything local**; the gate is
  documented push-pending, T0–T6 stay code-complete, do NOT re-ask. Vendored
  default theme still blocked (no DNS) — D3 interim rule (unknown name = keep
  working, unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D11 + M1/M2 decomposition + M2-b
  closure notes (`build/PHASE_2.md`); **`build/EXPORT-PLANTUML.md` §§1–14 is
  THE contract** — §14 = the engine iteration just ported; two contracts the
  pinned Java corrected during testing: `findGroups` sorts IN PLACE by id
  STRING (numeric `TreeSet` order never survives: `"10"<"2"<"9"<"abc"`), and
  the shallow-move leg writes `endGroupBoundary` AT the current indent first
  (first `G-` gets 2 spaces). `Dialect` hooks all tie engine/view/element to
  one `'a`; hooks reaching Java abort paths (`write_header`/`write_element`/
  `write_relationship`/`start_group_boundary`/`start_deployment_node_boundary`)
  are `Result`, the rest infallible; Java reference identity = id equality.
  MockDialect tokens: `HDR`/`FTR`, `E id`/`x id`, `G+`/`G-`, `S+`/`S-`,
  `C+`/`C-`, `N+`/`N-`, `B+ name`+indent / `end box`+outdent. Null frame =
  everything visible, no NPE; frames write ALL relationships; legend created
  LAST with last-body frame state. `IndentingWriter`: `to_string()` trims one
  trailing `\n`, negative indent pads nothing. `View::content()` ties borrow
  too short — engine has `content_of`; `Properties::get → Option<&str>` (use
  `.map(str::to_owned)`); group separator is a raw MODEL property, not the
  cascade; `java_split_len` no-match → 1, trailing empties dropped; `type_of`
  custom = raw metadata, instances → `""`, `: <tech>` only DN/IN/container/
  component. `System` name at `s.item.name`; `ViewBase::new(key, order)`;
  `ApiOptions` has 6 fields. P2-V3 settled (icon scale `0.5`);
  `EXPORT-PROPERTIES.md` is THE property/theme-chain contract; P1T-D1..D9;
  oracle = `$OXIDZR_STRUCTURIZR_BIN` (slow JVM; witnesses
  `/tmp/nix-shell.uni100/p2/`); coverage gate FAILS if a covered code stays
  allow-listed (5009 entry: remove when an export manifest covers it); NO
  NETWORK here; pinned Java `/tmp/nix-shell.uni100/opencode/structurizr/`
  `ad34036` — the reference decides ambiguities.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
