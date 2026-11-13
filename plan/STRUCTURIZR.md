# structurizr/structurizr — repository structure map

Pinned @ commit `ad34036` (2026-11-07). Monorepo of the Java reference implementation +
the unified application. Library version **6.2.4**; app version calver `2026.10.26`
(`app.revision`). Java 21, Maven (wrapper 3.9.11), **Apache-2.0 repo-wide** (no
per-module overrides). ~986 java files / ~103k LOC. No ANTLR anywhere (DSL parser is
hand-written). CI: `./mvnw -B -DexcludedGroups=IntegrationTest package`.

## 1. History (needed to navigate old docs/issues)

- ≤5.0.3 era: Gradle repo `structurizr/java` (+ separate `structurizr/cli`, `/lite`,
  `/ui`, `/onpremises`, standalone MCP). Last java release 5.0.3 (Nov 2025); repo
  archived 2026-03-28.
- "Structurizr vNext" (2026-02-01): everything folded into this Maven monorepo → library
  6.0.0 + app 2026.02.01; `structurizr local` replaces Lite, `server` replaces
  on-prem; one `structurizr.war`. Old repos = archived stubs.
- Two changelogs at root: `changelog-library.md` (semver), `changelog-application.md`
  (calver) — useful for "when was X removed" (e.g. 6.0.0: branding, `theme default`,
  Graphviz autolayout in core; element `location` removed; 6.2.1: `element.group==`).
- Maven `groupId` stays `com.structurizr`; artifact ids unchanged across 5.x→6.x.

## 2. Modules (Maven reactor; LOC = main java)

| Module          | LOC   | Depends on (compile)               | Role / key classes |
| --------------- | ----- | ---------------------------------- | ------------------ |
| `-annotation`   | 70    | —                                  | 5 marker annotations for component finder |
| `-core`         | 14.3k | jackson-annotations only           | Domain model. `Workspace`/`AbstractWorkspace`, `model/` (`Model`, `Element`+subtypes, `Relationship`, `Tags`, `CanonicalNameGenerator`, `SequentialIntegerIdGeneratorStrategy`, `HttpHealthCheck`), `view/` (`ViewSet`+view classes, `ElementView`/`RelationshipView`, `Styles`, `AutomaticLayout`, `Configuration`), `validation/`, `util/` (`SystemLandscapeGenerator`). Jackson annotations live here; **no JSON I/O code** |
| `-client`       | 2.3k  | core (+jackson-databind, httpclient5) | **JSON reader/writer** `io/json/JsonReader`/`JsonWriter` (alphabetical keys, NON_NULL+NON_EMPTY, UTC date fmt, 2-space indent); REST API client (`api/WorkspaceApiClient`, `X-Authorization`), `view/ThemeUtils`, `http/HttpClient`, encryption, `WorkspaceUtils.loadWorkspace` |
| `-dsl`          | 11.5k | client, import, export, component  | `StructurizrDslParser` (API: parse file/string → workspace), `Tokenizer`, `StructurizrDslTokens` (all keywords), one `XxxParser`+`XxxDslContext` per construct (registered via reflection in `DslParserContext`), `IncludeParser`, `ArchetypeParser`, `ScriptParser`, `ImpliedRelationshipsParser`, `ExpressionParser`; errors = `StructurizrDslParserException` (line+file, no column) |
| `-export`       | 5.1k  | core (test: client)                | `Exporter`/`DiagramExporter`/`Diagram`; PlantUML (Structurizr + C4 dialects), Mermaid, DOT, WebSequenceDiagrams, ilograph, theme exporter |
| `-import`       | 1.6k  | client                             | decisions (adr-tools/MADR/log4brains), diagrams (C4-PlantUML, Mermaid, kroki, image), docs importers |
| `-autolayout`   | 0.75k | export                             | `GraphvizAutomaticLayout` — shells out to `dot`, reads coords back from SVG |
| `-component`    | 2.1k  | core (+bcel, javaparser)           | Java component discovery for `!components` (bytecode + source) |
| `-inspection`   | 1.7k  | core                               | `Inspector`, ~30 `*Inspection`s, severities, `SummaryReporter` |
| `-application`  | 11.8k | dsl+inspection 6.2.4               | **parent = spring-boot-starter-parent 4.1.2**, war; commands (`com.structurizr.command.*`: playground local server mcp export generate merge validate inspect push pull …), Spring AI MCP server, static JS UI bundles, Lucene search, Playwright png/svg. Not a Central library |

Dependency chain: `core → client → {export → autolayout; import}`; `dsl → client+import+
export+component`; `application → dsl+inspection`.

**Not modules** (data dirs): `structurizr-json/structurizr.yaml` = OpenAPI 3.0 schema
(info.version 6.1.0 — lags code); `structurizr-themes/` = 16 theme packs (theme.json +
icons; PNGs are third-party copyright!).

## 3. Where things live (lookup table)

| Need                                   | Path |
| -------------------------------------- | ---- |
| Any DSL keyword/behavior               | `structurizr-dsl/src/main/java/com/structurizr/dsl/` (tokens file first) |
| Canonical DSL grammar & defaults       | docs clone `dsl/71-language.md` (+ `06-archetypes.md`, `31-includes.md`, `32-workspace-extension.md`, `51-docs.md`, `52-adrs.md`, `91-faq.md`) |
| JSON read/write semantics              | `structurizr-client/src/main/java/com/structurizr/io/json/` + Jackson annotations in core classes |
| JSON schema                            | `structurizr-json/structurizr.yaml` (verify against code!) |
| Default tags / canonical names / ids   | `structurizr-core/.../model/{Tags,CanonicalNameGenerator,SequentialIntegerIdGeneratorStrategy}.java` |
| Default-view generation                | `structurizr-core/.../view/ViewSet.java#createDefaultViews` (called from application `AbstractCommand`) |
| Export format table (CLI)              | `structurizr-application/.../command/ExportCommand.java`; docs `commands/04-export.md` |
| Theme URL constants                    | `Configuration.STRUCTURIZR_CLOUD_SERVICE_THEMES_URL` (core), `ThemeParser.DEFAULT_THEME_URL` (dsl) |

## 4. Fixtures (future compat goldens)

- `structurizr-dsl/src/test/resources/` — **107 `.dsl` fixtures** (84 top-level kebab-case
  names per feature: archetypes, deployment-*, dynamic*, include-*, script-*,
  hierarchical-identifiers, text-block, utf8/iso-8859, unexpected-tokens-* …) + nested
  workspace dirs: `spring-petclinic/` (has golden `workspace.json` incl. base64 DSL +
  implied rels), `big-bank-plc/`, `include/`, `docs/`, `decisions/{madr,log4brains,
  adrtools}`, `extend/`, `image-views/`, scripts (js/groovy/kts/rb), a plugin jar.
- DSL tests assert in-memory state, NOT golden JSON (only 3 .json there). Real golden
  JSON: `structurizr-export/src/test/resources` (`amazon-web-services.{dsl,json}`,
  `big-bank-plc.json`, `groups.{dsl,json}`) and `structurizr-client/src/test/resources`
  (`backwardsCompatibility/*.json` ×4, `workspaceValidation/*.json` ×16,
  `structurizr-36141-workspace.json`).
- ~10 DSL tests + client API tests tagged `IntegrationTest` (need Docker/network) —
  excluded in CI.

## 5. Build & run

```sh
./mvnw -B -DexcludedGroups=IntegrationTest package    # whole repo (what CI runs)
java -jar structurizr-application/target/structurizr-1.0.0.war <command>   # or ./structurizr.sh
# e.g. … local <dir> | export -w ws.dsl -format json | validate | mcp -dsl
```

- Libraries publish to Maven Central (`com.structurizr:structurizr-{core,dsl,...}:6.2.4`);
  app ships as war/docker `structurizr/structurizr` (calver tags) at download.structurizr.com.
- Docs site repo: `structurizr/structurizr.github.io` (own LICENSE). Old lite/ui were MIT.
- Beware: Maven Central has very old GPL-era artifacts; current monorepo = Apache-2.0.
