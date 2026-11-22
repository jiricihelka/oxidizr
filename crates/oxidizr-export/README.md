# oxidizr-export

The exporter engine: turns a workspace model into text. The `diagram`
bean model (`Diagram`, `Legend`, exporters) plus the concrete dialects —
`plantuml` (plain and C4), `mermaid`, and the WSD
(`websequencediagrams`) writer — plus the theme loader/store and the
shared export-property cascade.

Depends only on `core` + `json`: exporters consume the model, never DSL
sources. Deep format detail: `agents/plan/build/EXPORT-C4-PLANTUML.md`.

Consumed by `oxidizr-api` (the `export` command).
