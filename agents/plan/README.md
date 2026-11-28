# agents/plan/ — research & design notes

Quick-reference knowledge base for building oxidizr. Dense bullets/tables only; verify
claims against source before relying on them. When notes and the structurizr source
disagree, the source wins — update the notes.

## Files

| File                          | Contents                                                              | Read when                          |
| ----------------------------- | --------------------------------------------------------------------- | ---------------------------------- |
| `README.md` (this file)       | Index, conventions, errata, where the reference sources live           | First, every time                  |
| `COMPAT.md`                   | The compatibility contract (hard rules) + source-of-truth map          | Before touching behavior/JSON/grammar |
| `WORKFLOW.md`                 | Commands, dev shell, CLI/config policy, code quality & style           | Working in the repo                |
| `TESTING.md`                  | Test categories A/B, golden corpus rules, benchmarks                   | Writing tests / regenerating goldens |
| `FEATURES.md`                 | Structurizr feature inventory: model, DSL, JSON contract, compat traps | Before implementing any feature    |
| `DSL.md`                      | DSL grammar quick ref: tokens, statement forms, scopes, directives     | Writing/extending the DSL parser   |
| `JSON-FORMAT.md`              | Field-level JSON reference: keys, writer rules, schema-vs-code drift   | Writing/reading workspace JSON     |
| `FIXTURES.md`                 | Index of reference DSL/JSON test fixtures (future golden corpus)       | Building or extending test harness |
| `STRUCTURIZR.md`              | Reference repo map: modules, deps, key classes, build                  | Hunting for a class/behavior       |
| `PLAN.md`                     | Phase roadmap (parsers → converters → GUI → CLI tools)                 | Starting any work / scoping scope  |
| `build/PHASE_1.md`            | Detailed Phase 1 implementation plan (all architectural decisions)    | Implementing Phase 1               |
| `build/PHASE_1T.md`           | Phase 1T interlude plan: e2e regression corpus + `oxidizr-api` + benches | Implementing the e2e suite        |
| `build/PHASE_2.md`            | Phase 2 plan: exporters (PlantUML ×2, Mermaid, theme); semantic parity | Implementing Phase 2               |
| `build/PHASE_2T.md`           | Phase 2T plan: e2e export coverage — manifest v2, export axis, invariants | Implementing the 2T interlude   |
| `build/PHASE_2R.md`           | Phase 2R plan: small-scope refactors — Java-isms out, bytes invariant     | Implementing the 2R interlude   |
| `build/PHASE_3.md`            | Phase 3 plan: egui viewer + layout editing, own layout engine          | Implementing Phase 3               |
| `build/PHASE_4.md`            | Phase 4 plan: diff/query/init/lsp/inspect/importers, release           | Implementing Phase 4               |
| `build/PHASE_4T.md`           | Phase 4T plan: feature × suite test matrix, P4 coverage gaps            | Implementing the 4T interlude   |
| `build/PHASE_4R.md`           | Phase 4R plan: idiomatic refactors, public-API churn allowed            | Implementing the 4R interlude   |
| `build/PHASE_4D.md`           | Phase 4D plan: docs reset — short human-first text, READMEs everywhere  | Implementing the 4D interlude   |
| `build/PHASE_5R.md`           | Phase 5R plan: large-scale idiomatic refactor → v0.5.0 (error texts loosened) | Implementing P5R            |
| `build/PHASE_6.md`            | Phase 6 plan: GUI arrow model-editing + help texts (F13+F14, D17)      | Implementing P6              |
| `build/PHASE_7.md`            | Phase 7 plan: auto-layout quality (dagre) + configuration (F15, D18, O2 superseded) | Implementing P7              |
| `DEVIATIONS.md`               | Agreed, compat-safe differences from the reference + Locked list       | Before implementing ANY behavior   |
| `ADDITIONAL_FEATURES.md`      | Registry of features Structurizr doesn't have (✅ agreed / 💡 ideas)    | Adding anything beyond parity      |
| `research/notes-features.md`  | Raw deep notes behind FEATURES.md (see errata!)                        | Deep dive on a specific feature    |
| `research/notes-structure.md` | Raw deep notes behind STRUCTURIZR.md                                   | Deep dive on repo internals        |

## Errata (verified corrections — trust these over older notes)

- JSON style key is `color` for BOTH element and relationship styles (DSL accepts
  `colour` too). The `colour`/`color` "asymmetry" in `research/notes-features.md` §1.8/§3.2
  is WRONG (verified: no `"colour"` in any repo JSON; no `colour` property in core).
- v6 views do NOT serialize `includedElements`/`excludedElements` — the geometry arrays
  are the inclusion lists (verified: no such fields in `View.java`/`ModelView`; absent
  from every test JSON). `includedElements` claims in research notes are legacy-era.
- `spring-petclinic/workspace.json` is a **pre-v6** output (`location`, `branding`,
  autoLayout `implementation`) — read-compat oracle, not a write golden.
- There is no public `structurizr/samples` repo; fixtures = monorepo test resources
  (see FIXTURES.md).
- The reference `export` command DOES wire `websequencediagrams` (verified in
  `ExportCommand.java`, pinned commit); only DOT and ilograph are library-only. An
  earlier planning note claimed otherwise — oxidizr still defers WSD export to
  Phase 4 (decision 2026-11-14, `build/PHASE_4.md` P4-D8).

## Conventions

- Behavioral deviations from the reference are decided and recorded in `DEVIATIONS.md`;
  new (non-parity) features are registered in `ADDITIONAL_FEATURES.md`. Read both before
  implementing anything user-visible.

- One topic per file; tables over prose; exact Java class names quoted as `Code`.
- Facts marked `(verified)` were checked directly against source at the pinned commit;
  everything else came from the research notes and should be re-checked when used.
- Cite the version a fact applies to when it changed recently (v6 = monorepo era).

## Reference sources (local clones)

Pinned: monorepo commit `ad34036` (2026-11-07) = library **v6.2.4**, app 2026.10.26.

```sh
git clone --depth 1 https://github.com/structurizr/structurizr        # Java monorepo (reference impl)
git clone --depth 1 https://github.com/structurizr/structurizr.github.io  # docs.structurizr.com markdown
```

Suggested location: `../structurizr` / `../structurizr-docs` (outside the oxidizr repo;
do not vendor megabytes of Java into this repo; test-fixture vendoring is decided later).

Key doc pages: `structurizr.github.io/dsl/71-language.md` (authoritative grammar &
defaults), `workspaces/file-types.md`, `commands/*`, `openapi/*`.
