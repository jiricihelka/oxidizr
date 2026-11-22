# tests/fixtures — importer golden-file corpus (P4T T-2)

Authored ORIGINALS (not vendored — `testdata/` stays the vendored corpus).
Consumed by `tests/golden_files.rs` as Category A `expect!` model digests;
every file here must be consumed by a test (`every_fixture_is_consumed`
pins the inventory). Re-bless the digests with
`UPDATE_EXPECT=1 cargo test -p oxidizr-import --test golden_files` — the
digests are OURS (semantic bar F12), never reference bytes.

| Fixture | Covers |
|---|---|
| `c4/context.puml` | C4-PlantUML context: `!$` variable, kwargs, multi-line call, `SystemQueue`, unknown-macro warn-and-skip, chrome (`!include`-free `skinparam`/`LAYOUT_*`/`@enduml`) |
| `c4/containers.puml` | nested boundaries, brace-on-next-line, `Container_Stub`, `BiRel`/`Rel_D`, `Rel_Index` chrome/skip reality |
| `mermaid/context.mmd` | C4-Mermaid flavour: comments, kwargs + positional tag slots, `Enterprise_Boundary`, `ContainerDb`, `Rel_Back`/`BiRel` |
| `image/tiny.png` | 1×1 PNG → data-URI `ImageView` (reference content-type table) |
| `image/icon.svg` | SVG → RAW content import (D16 class), file-name title default |
| `docs/…` | `DefaultDocumentationImporter` top-level walk vs recursive descent (`sub/03-nested.md`), md+adoc formats |
| `adrs/adrtools/…` | `AdrToolsDecisionImporter` id/title/date/status parse |
| `adrs/madr/…` | `MadrDecisionImporter` incl. front-matter stripping |
