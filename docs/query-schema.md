# `query` output schema (schema 1)

Machine-readable contract for `oxidizr query WORKSPACE --filter EXPR
--format json`. The document is oxidizr's own format — it is NOT
Structurizr JSON and never feeds the JSON byte contract.
Implementation: `crates/oxidizr-api/src/query.rs` (report layer); the
model-side evaluator is the DSL item-expression engine reached through
`oxidizr_dsl::query` — one parser, one evaluator, two consumers
(filtered views, `query`).

## Versioning policy

Identical to the diff/inspect schemas: the `schema` key carries the
version as a NUMBER; document-shape changes are additive-only within a
major oxidizr version; a breaking change bumps `schema`; consumers
ignore unknown keys.

## Document shape

`schema`, `filter` and `kind` are ALWAYS present, in that order, and
key order below is part of the contract (stable bytes). The class
sections are present per `--kind`: `element` adds `elements`,
`relationship` adds `relationships`, `view` adds `views`, and `all`
(the default) adds all three, in that order. Sections keep the ENGINE
RESULT ORDER (union `||` dedupes first-wins, intersection ` && `
preserves left order); views follow the workspace view order.
Provenance stamps never appear (the diff schema rule).

```json
{
  "schema": 1,
  "filter": "alice->bank",
  "kind": "all",
  "elements": [
    { "id": "1", "kind": "Person", "canonicalName": "Person://Alice",
      "tags": ["Element", "Person"] }
  ],
  "relationships": [
    { "id": "7", "kind": "Relationship",
      "canonicalName": "Relationship://Person://Alice -> SoftwareSystem://Bank (Uses)",
      "tags": ["Relationship"] }
  ],
  "views": [
    { "key": "landscape", "kind": "SystemLandscape", "title": null }
  ]
}
```

* `canonicalName` is the loader-agnostic identity — a `.dsl` and its
  JSON twin report the SAME names (`null` only for an item no canonical
  name can be built for).
* `tags` are the EFFECTIVE tag list (derived defaults plus stored,
  deduped first-wins). Views have NO tags — the `views` entries carry
  `key`/`kind`/`title` instead.
* `kind`: elements report the enum spellings (`Person`,
  `SoftwareSystem`, `Container`, `Component`, `CustomElement`,
  `DeploymentNode`, `InfrastructureNode`, `SoftwareSystemInstance`,
  `ContainerInstance`); relationships report `Relationship`; views
  report the JSON section names (`SystemLandscape`, `SystemContext`,
  `Container`, `Component`, `Dynamic`, `Deployment`, `Filtered`,
  `Image`, `Custom`).

## Human formats

* `table` (default) — one row per entry in the SAME order:
  `id··kind··canonicalName-or-key··[tags-csv]` (two-space separators;
  the tags column is dropped when empty; a `null` name prints empty).
* `ids` — one id (elements/relationships) or key (views) per line,
  pipeline-friendly, silent on zero matches.

## Exit codes

`query` is informational: **zero matches exits 0** (no bytes). A
THROWING grammar arm exits 1 with the official tooling's message,
verbatim (no new diagnostic codes); I/O problems exit 3; usage errors
(missing `--filter`) stay clap's 2.

## The query language

The DSL **item-expression grammar IS the query language**. The
model-side arms below are the full expression-grammar inventory
(`crates/oxidizr-dsl/src/engine/expr.rs`); arm prefixes match
CASE-INSENSITIVELY, values keep their RAW case (exact string
equality). Combinators: `" && "` is checked FIRST, then `" || "`;
each split takes exactly TWO arms — a third one is dropped.

| Arm | Matches | Notes |
|---|---|---|
| `<identifier>` | the registered item | `structurizr.dsl.identifier` (both loaders) |
| `element==X` / `element!=X` | identifier / expression, or ALL-but | `X` may itself be an expression |
| `element.type==T` | enum spelling, e.g. `SoftwareSystem` | unknown type ⇒ EMPTY (the catch-all arm, no throw) |
| `element.tag==A[,B…]` / `element.tag!=…` | EFFECTIVE tags, AND over the CSV | instance tags fall through to the static element |
| `element.technology==T` / `element.technology!=T` | Container/Component/DeploymentNode/InfrastructureNode/ContainerInstance | |
| `element.properties[NAME]==V` | property equality | no `!=` form exists |
| `element.group==G` | group membership | no `!=` form |
| `element.parent==P` | direct children of `P` | THROWS `The parent element "P" does not exist` |
| `<a>-><b>` / `<a>->` / `-><b>` / `-><a>->` | couplings | efferent / afferent expansion; THROWS on an unknown identifier |
| `relationship==X` | relationship identifier / expression | `relationship==*` = all |
| `relationship.tag==…!=`, `relationship.properties[…]==` | like elements | linked-relationship tag fall-through |
| `relationship.source==S` / `relationship.destination==D` | endpoint match | |
| `expr && expr` / `expr \|\| expr` | intersect / union | engine order preserved |

Views have no official query grammar (they are not model items). For
them `--kind view`/`all` documents an oxidizr SUBSET (an additional
feature, ours to define — never claimed as parity with the official
tooling): `key|title|description ==|!= VALUE` with the SAME
`" && "`/`" || "` two-arm split, case-insensitive arm prefix,
case-sensitive values. An arm outside the subset matches NOTHING —
the same fail-soft the catch-all arm applies to model expressions,
so a model filter lists no views and vice versa.
