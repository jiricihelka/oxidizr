# oxidizr-import

Diagram importers: one diagram file in, a fresh workspace out. The
`image` leg wraps a PNG/JPG/GIF/SVG into a single `ImageView`; the
C4-PlantUML and Mermaid legs parse diagram sources back into the model
(their macro grammars are documented here, not imported).

Warning-and-skip: unrecognized constructs ride `Imported::warnings`, they
never abort the import. `regexlite` is the small std-only regex engine
the documentation/ADR import filters use; the `docs`/`decisions` modules
back the DSL `!docs`/`!adrs` blocks.

`ImportFormat` (with `ImportFormat::sniff` for the file extension) is the
public format enum the pipeline consumes.

Consumed by `oxidizr-api` (the `import` command) and `oxidizr-dsl`
(documentation blocks).
