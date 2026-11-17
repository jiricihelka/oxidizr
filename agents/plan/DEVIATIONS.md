# DEVIATIONS.md — intentional deviations from Structurizr

Behavioral differences between oxidizr and the reference (Java) implementation, agreed
here BEFORE implementation. Purely additive features (things Structurizr does not have
at all) live in `ADDITIONAL_FEATURES.md`; this file only covers points where the
reference HAS a behavior and we deliberately differ.

The compatibility contract is absolute:

> **Contract**: for any input accepted by the reference, oxidizr must produce the same
> in-memory workspace and the same JSON output (byte-identical: alphabetical keys,
> NON_EMPTY omission, UTC-Z dates, 2-space indent, string ids). Deviations are allowed
> ONLY where the contract is silent: diagnostics/stderr/exit codes, CLI ergonomics, new
> commands, and behavior behind explicit opt-in flags. Anything else is a bug.

Reference pinned: monorepo `ad34036` (v6.2.4). Details in `agents/plan/`.
Ground rule for future changes: a new deviation needs (a) an entry below, (b) proof the
default output still matches the reference goldens.

## D1 — Rich diagnostics instead of first-error (default)

- Structurizr: parse aborts at the first error; `<msg> at line N of <file>: <line text>`,
  no column; unknown statement → `"X is not valid here"`; deprecated tokens fail mid-parse.
- oxidizr: all parse + validation errors collected and reported together, with
  line+column+snippet, "did you mean" suggestions for misspelled keywords, and warnings
  for deprecated-but-accepted tokens (`!ref`, `metadata`-as-`tags`, legacy JSON fields).
  `--strict` promotes warnings to errors.
- Compatibility impact: none (stderr/exit codes only). Exit codes must stay CI-sane:
  0 ok / 1 error / 2 usage.

## D2 — JVM-only directives are rejected cleanly

- Structurizr: `!script` (JSR-223), `!plugin` (JAR SPI), `!components` (bytecode/source
  analysis), `!impliedRelationships <fqcn>` execute arbitrary Java/JVM code.
- oxidizr: these tokens are recognized and rejected with a precise, actionable error
  (construct, file/line, why, suggested portable alternative — e.g. restructure with
  `!include` / `workspace extends`, declare components manually). Workspaces using them
  are, by definition, out of supported scope.
- Explicitly REJECTED alternative (do not revisit without new evidence): emulating
  `!script` with an embedded JS engine — Structurizr scripts target Nashorn/GraalJS and
  the Java API, so "same input, different workspace" is likely; silent divergence is
  worse than honest rejection.
- Roadmap (undated): out-of-process extension protocol (exec + JSON-RPC) as a native
  plugin story; must never change parse results for JVM-plugin-free workspaces.

## D3 — JSON round-trip: reference-exact by default, lossless opt-in

- Structurizr: reads unknown/legacy fields leniently, then silently DROPS them on write
  (e.g. `location`, `branding`, `enterprise` from v5 workspaces).
- oxidizr: default = exact reference parity (drops them too). `--keep-unknown` (off by
  default) preserves unknown AND known-legacy fields verbatim on JSON→JSON round-trips
  for migration tooling — "superset-preserving round-trip" mode. Output remains valid
  Structurizr JSON either way (readers ignore unknowns).

## D4 — (moved) Additive commands

New first-class commands are not deviations; they are registered in
`ADDITIONAL_FEATURES.md` (F1 `reconcile`, F2 `fmt`, F3 `diff`, F4 `oracle`). D4 is kept
as a tombstone so old references keep resolving.

## D5 — No implicit network access (default)

- Structurizr: fetches themes from `https://static.structurizr.com/...` during load/
  export; `!include <url>` fetches URLs; offline support is an env-var convention.
- oxidizr: never touches the network unless explicitly asked. Theme URLs needed for
  diagram export resolve offline-first: URL cache hit, installed name, or
  `--themes-dir`/`OXIDZR_THEMES_DIR`/`<ws-parent>/themes`/`STRUCTURIZR_THEMES` (P2-D3
  source chain); a URL that must be FETCHED requires `--allow-net` (else OXZ-5001).
  **Shipped as planned at P2-M1** (`agents/plan/build/PHASE_1.md` P1-D8). JSON output is
  unaffected in all cases (theme URLs are stored verbatim; resolution only ever matters
  for diagram rendering/export).
- `--allow-net` is **one global flag**, and it gates **theme URL fetches only**:
  `!include <url>` and `workspace extends <url>` stay rejected regardless of the flag
  (the P1-hardened behavior, `agents/plan/build/PHASE_2.md` P2-D3 item 3 — the
  reference needs its feature-gate flags for those and the fetch legs are Phase 4
  material). Deliberately **no env var** can enable network (agreed 2026-11-14; policy
  bundle in agents/plan/WORKFLOW.md §Dev environment / §CLI/config policy).

## D6 — Internal quality decisions (no behavioral surface)

- Immutable domain model; no exceptions mid-construction; validation aggregated at
  defined checkpoints (vs Java setters throwing `IllegalArgumentException`, producing
  partially built workspaces). Validation semantics themselves match the reference
  (rule catalogue: `agents/plan/FIXTURES.md` §C validation fixtures).
- Contract-first JSON: one source of truth for the JSON contract — the `oxidizr-json`
  serde mapping, pinned field-by-field by the write catalog (`agents/plan/build/PHASE_1.md`
  P1-D6) and drift-checked against goldens in CI — preventing Structurizr's chronic
  schema-lags-code failures (schema 6.1.0 vs code 6.2.4 at time of writing). (An earlier
  wording said "generated from one machine-readable definition"; PHASE_1.md P1-D5 chose
  serde derives on the model — the derives ARE the single definition.)
- Diagnostics-grade lexer/parser with a real CST feeding workspace construction, enabling
  D1, `fmt`, `diff` (vs Java's per-scope keyword dispatch over preprocessed strings).

## D7 — Documentation/ADR import deferred (temporary, Phases 1–3)

- Structurizr: `!docs`/`!adrs` import markdown docs and ADRs (4 tool formats) into the
  workspace `documentation` tree **at load time**, so exported JSON carries them.
- oxidizr (2026-11-14 rebalance): Phases 1–3 **parse the statement form** (valid args,
  spans) but reject the import with OXZ-5003 "documentation import arrives in Phase 4"
  instead of silently skipping. Workspaces using these directives are therefore
  reference-compatible only **from Phase 4 on** (importers: `build/PHASE_4.md` P4-D13);
  their fixtures are vendored but excluded from the P1–P3 parse/golden gates.
- Nature: sequencing deviation with a retirement milestone, not a permanent difference.
  Until then, never emit a workspace whose `documentation` silently lacks imported
  content — the loud error is the point.

## D8 — `HashMap` order emulation: two pathological edge cases (accepted)

- Structurizr: free-form `properties` maps are Java `HashMap`s; Jackson serializes them
  in hash-bucket order. oxidizr REPLICATES this exactly (`build/WRITE-CATALOG.md` §1) —
  the replication itself is Locked-territory (JSON byte parity).
- Two inputs are replicated by the general rule rather than Java's exact behavior:
  1. **treeified buckets** — ≥ 8 keys landing in one bucket at capacity ≥ 64 iterate in
     tree order there, insertion order here (needs a 64+-key property map whose keys
     collide in Java's low bits; no realistic workspace, and property keys are
     developer-authored);
  2. **capacity inputs with stripped keys** — reference-emitted workspaces carrying the
     four `structurizr.inspection.*` properties (which change the final capacity) are
     compared after the M0 golden-stripping rule, so capacity is computed from the
     stripped map, matching what the reference itself would produce for the same
     content it stores.
- Nature: bounded unreachable-in-practice divergence, discovered while writing the M3
  write catalog (2026-11-14); a property test pins the emulator for all normal maps.

## D9 — `workspace extends` deferred to M5 (temporary, lands mid-M4)

- Structurizr: `workspace extends <file|url>` loads the BASE workspace (DSL or JSON)
  and the extending file then ADDS to it (`WorkspaceParser.parse` → nested parser +
  `configureFrom`; for file bases the base's `structurizr.dsl` source is cleared —
  `DslUtils.clearDsl` — and the workspace becomes non-portable).
- oxidizr (2026-11-14, mid-M4 engine port): the statement parses through the
  reference's exact guards and grammar texts (`Too many tokens, expected: workspace
  [name] [description] or workspace extends <file|url>`, `Expected: workspace extends
  <file|url>`), but loading the base reports OXZ-5004 "workspace extends arrives in
  M5" instead of silently ignoring it. The loader (file-only DSL+JSON base,
  identifier re-registration, `clearDsl` + portable=false) is the `PHASE_1.md` M5
  line item; extends-using fixtures stay out of the parse gates until then.
- Nature: sequencing deviation fully retired at M5 — `workspace` arms otherwise
  (name/description/`model`) are delivered in M4; nothing silently diverges, the
  error is loud by design.

## D10 — `icon <file>`: SVG files portable, raster files rejected (permanent, JVM codec)

- Structurizr: element-style `icon <file>` embeds the image as a data URI via
  `ImageUtils.getImageAsDataUri`: SVG files are base64 of the raw bytes, but
  PNG/JPG/JPEG/GIF go through **`javax.imageio` decode + re-encode** — the
  embedded base64 is Java's PNG/JPEG encoder output, byte-for-byte, not the
  original file. Reproducing that byte-exactly from pure Rust std is out of
  scope (JPEG re-encode is lossy and encoder-specific).
- oxidizr (2026-11-14, M4 styles port): `icon` accepts data URIs and
  http(s) URLs (stored verbatim — the reference only *validates* URL icons, it
  never fetches them), and **SVG files** (raw-bytes base64 + `dslPortable=false`,
  byte-identical). PNG/JPG/GIF files are rejected with OXZ-5002 naming the
  `data:` URI fallback — never embedded as raw bytes, which would silently
  change the golden JSON. Reference fixtures using raster `icon <file>`
  (`dsl/test.dsl`) stay excluded from the parse/golden gates until the error is
  tolerated there.
- Nature: permanent portability deviation (std-only, WORKFLOW.md §Code quality); the
  workarounds (paste the data URI, or use SVG) are byte-compatible with the
  reference for everything except Java's re-encoded raster bytes, which we
  refuse to fake.

## D11 — Diagram/theme file imports deferred; raster `image <file>` rejected

- Structurizr: inside an image view, `plantuml`/`mermaid`/`kroki` render the
  diagram **at load time** (source text, file, URL, or the PlantUML export of a
  referenced `viewKey`) and embed the result as a data URI; `theme <file|url>`
  loads theme content from disk/network; raster `image <file>` goes through the
  JVM image codecs (D10 rationale).
- oxidizr (2026-11-15, M4 views port): all statement forms parse with the
  reference's exact grammar/error texts (`Expected: plantuml <source|file|url|viewKey>`,
  `Expected: kroki <format> <source|file|url>`, …); URL branches emit OXZ-5001
  (D5). The render/load branches (source, file, viewKey, theme file/url) report
  OXZ-5005 "… arrives in Phase 4 (D11)" — never a silent no-op. Raster
  `image <file>` is rejected OXZ-5002 (byte-exact Java re-encode is impossible
  std-only; D10 analog). **SVG** content files are byte-identical (raw-bytes
  base64 data URI) and fully delivered.
- Nature: the deferrals retire with the Phase 4 importers (same batch as D7,
  P4-D13); the raster rejection is permanent for the D10 reason. Bundled-theme
  name resolution stays out of scope (app-layer `InstalledThemes` feature; the
  name branch stores the name verbatim and the file-branch existence check is
  reference-exact).

## Locked — must NEVER deviate (for contributors and future agents)

Shared sequential integer id counter (elements+relationships, definition order) · exact
implicit tag stacks & order, tagless implied relationships · `${VAR}` pre-pass semantics
incl. env fallback behind the same feature default · DSL portability rules and
`structurizr.dsl`/`structurizr.dsl.identifier` retention · auto view keys `Prefix-NNN` ·
environment flattening (`environment` string per node) · JSON details: alphabetical keys,
NON_EMPTY omission, `color` key, comma-string tags, string ids / numeric workspace id,
UTC-Z dates, 2-space indent · lenient JSON reading (unknowns ignored) · default views
generation only where the reference does it (app layer), never in parser/library.

Anything contradicting this list is fixed by changing oxidizr, per agents/plan/COMPAT.md.
