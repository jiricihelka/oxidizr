# plan/ — research & design notes

Quick-reference knowledge base for building oxidizr. Dense bullets/tables only; verify
claims against source before relying on them. When notes and the structurizr source
disagree, the source wins — update the notes.

## Files

| File                          | Contents                                                              | Read when                          |
| ----------------------------- | --------------------------------------------------------------------- | ---------------------------------- |
| `README.md` (this file)       | Index, conventions, errata, where the reference sources live           | First, every time                  |
| `FEATURES.md`                 | Structurizr feature inventory: model, DSL, JSON contract, compat traps | Before implementing any feature    |
| `DSL.md`                      | DSL grammar quick ref: tokens, statement forms, scopes, directives     | Writing/extending the DSL parser   |
| `JSON-FORMAT.md`              | Field-level JSON reference: keys, writer rules, schema-vs-code drift   | Writing/reading workspace JSON     |
| `FIXTURES.md`                 | Index of reference DSL/JSON test fixtures (future golden corpus)       | Building or extending test harness |
| `STRUCTURIZR.md`              | Reference repo map: modules, deps, key classes, build                  | Hunting for a class/behavior       |
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

## Conventions

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
