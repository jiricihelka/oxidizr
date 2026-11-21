# `query` output schema (schema 1)

Machine-readable contract for `oxidizr query WORKSPACE --filter EXPR
--format json` (§P4-M3 O-1, F6). The document is oxidizr's own format —
it is NOT Structurizr JSON and never feeds `oxidizr-json` (the byte
contract is untouched, P4-D7). Range owner: `crates/oxidizr-api/src/
query.rs` (report layer); the model-side evaluator is the pinned
`ExpressionParser` port reached through `oxidizr_dsl::query` — one
parser, one evaluator, two consumers (filtered views, `query`).

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

* `canonicalName` is the loader-agnostic identity (`diff` D-4 rule —
  a `.dsl` and its JSON twin report the SAME names; `null` only for an
  item no canonical name can be built for).
* `tags` are the EFFECTIVE tag list (`getTags()`: derived defaults plus
  stored, deduped first-wins). Views have NO tags — the `views` entries
  carry `key`/`kind`/`title` instead.
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
THROWING grammar arm exits 1 with the reference message (no new
diagnostic codes); I/O problems exit 3; usage errors (missing
`--filter`) stay clap's 2.

## The query language (P4-V4 inventory — CLOSED 2026-11-21)

The DSL **item-expression grammar IS the query language** (P4-D3). The
model-side arms below are the pinned `ExpressionParser` token inventory
(`StructurizrDslExpressions` @ monorepo `ad34036`, ported 1:1 in
`crates/oxidizr-dsl/src/engine/expr.rs`); arm prefixes match
CASE-INSENSITIVELY, values keep their RAW case (Java `equals`).
Combinators: `" && "` is checked FIRST, then `" || "`; Java `split` +
`[0]`/`[1]` means exactly TWO arms — a third one is dropped.

| Arm | Matches | Notes |
|---|---|---|
| `<identifier>` | the registered item | `structurizr.dsl.identifier` (both loaders) |
| `element==X` / `element!=X` | identifier / expression, or ALL-but | `X` may itself be an expression |
| `element.type==T` | enum spelling, e.g. `SoftwareSystem` | unknown type ⇒ EMPTY (base parser `default:`, no throw) |
| `element.tag==A[,B…]` / `element.tag!=…` | EFFECTIVE tags, AND over the CSV | instance tags fall through to the static element |
| `element.technology==T` / `element.technology!=T` | Container/Component/DeploymentNode/InfrastructureNode/ContainerInstance | five-stream reference order |
| `element.properties[NAME]==V` | property equality | NO `!=` form exists in the reference |
| `element.group==G` | group membership | no `!=` form |
| `element.parent==P` | direct children of `P` | THROWS `The parent element "P" does not exist` |
| `<a>-><b>` / `<a>->` / `-><b>` / `-><a>->` | couplings | efferent / afferent expansion; THROWS on an unknown identifier |
| `relationship==X` | relationship identifier / expression | `relationship==*` = all |
| `relationship.tag==…!=`, `relationship.properties[…]==` | like elements | linked-relationship tag fall-through |
| `relationship.source==S` / `relationship.destination==D` | endpoint match | |
| `expr && expr` / `expr \|\| expr` | intersect / union | engine order preserved |

Views have NO reference expression grammar (they are not model items).
For them `--kind view`/`all` documents an oxidizr SUBSET (this is F6
additional-feature surface, ours to define — never claimed as
reference parity): `key|title|description ==|!= VALUE` with the SAME
`" && "`/`" || "` two-arm split, case-insensitive arm prefix,
case-sensitive values. An arm outside the subset matches NOTHING —
the fail-soft the base parser's `default:` arm applies to model
expressions too, so a model filter lists no views and vice versa.
