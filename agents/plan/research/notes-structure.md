# Structurizr monorepo — structural notes (raw)

Source: /tmp/nix-shell.uni100/opencode/structurizr (github.com/structurizr/structurizr)
Clone HEAD: `ad34036` "Fixes structurizr-dsl tests - #140." dated **2026-11-07** (shallow clone, 1 commit, no tags).
Docs clone: /tmp/nix-shell.uni100/opencode/structurizr-docs.

---

## 1. ROOT

### pom.xml (aggregator)
- groupId `com.structurizr`, artifactId `structurizr`, version **6.2.4**, packaging `pom`.
- Java: `maven.compiler.release=21` (all libs; app also 21).
- License (whole reactor): **The Apache License, Version 2.0** (LICENSE file = Apache-2.0 text).
- `<dependencyManagement>`: only `org.junit:junit-bom:5.11.0` (import); root adds junit-jupiter-api + junit-jupiter-params as test deps for every module.
- Build plugins at root (apply to all modules): maven-source-plugin (attach-sources), maven-javadoc-plugin (attach-javadocs), `org.sonatype.central:central-publishing-maven-plugin:0.9.0` (publishingServerId `central`), maven-gpg-plugin (sign at verify). distributionManagement → `https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/`. ⇒ every reactor module is staged for Maven Central publishing (the app module is in the reactor too; whether it is actually released is a release-process question, not visible in repo).
- Modules (order in root pom, lines 159–170):
  structurizr-annotation, structurizr-autolayout, structurizr-client, structurizr-core, structurizr-component, structurizr-dsl, structurizr-export, structurizr-import, structurizr-inspection, structurizr-application.
  ⇒ **structurizr-json and structurizr-themes are NOT Maven modules** — data/doc-only dirs:
  - `structurizr-json/README.md` + `structurizr-json/structurizr.yaml` (1623 lines): an **OpenAPI 3.0 document** ("workspace API"), `info.version: 6.1.0`, defining `GET/PUT /workspace/{id}` etc., with `components.schemas.Workspace` — this is the authoritative published definition of the JSON format (docs: docs.structurizr.com/openapi). Schemas: Workspace (id:number/long, name, description, version, ...), and the usual Person/SoftwareSystem/.../View/Style types.
  - `structurizr-themes/`: 16 theme dirs (amazon-web-services-2020.04/2022.04/2023.01/2025.07, google-cloud-platform-2025.09/v1.5, kubernetes, microsoft-azure-2019.09/2020.07/2021.01/2023.01/2024.07/2025.11, oracle-cloud-infrastructure-2020.04/2021.04/2023.04), each a `theme.json` + PNG icons. README: "All theme.json files … Apache 2.0. All PNG icons are copyright of their respective copyright holders."
- Maven wrapper: `mvnw`/`mvnw.cmd`, `.mvn/wrapper/maven-wrapper.properties` → Maven **3.9.11** (only-script distribution).
- CI: `.github/workflows/maven.yml` only — JDK 21 temurin, `mvn -B -DexcludedGroups=IntegrationTest package --file pom.xml` on push/PR to main.
- Helper scripts at root: `structurizr.sh` = `java -jar structurizr-application/target/structurizr-1.0.0.war $@`; `mcp.sh` = `java -jar structurizr-mcp/target/structurizr-mcp-1.0.0.war $@` (stale reference to the old standalone MCP repo).
- README.md: points at playground.structurizr.com + docs.structurizr.com; no build info.

### Changelogs / consolidation story
- Two changelogs: `changelog-library.md` (95 lines) and `changelog-application.md` (234 lines).
- **changelog-library.md** = library (Structurizr for Java) releases, per-module entries. Starts at **v6.0.0 (1st February 2026)** — the monorepo launch ("General: Requires Java 21"; core removes branding + Graphviz auto-layout; dsl removes `branding` keyword, adds installed themes, removes `theme default`). Later: v6.1.0 (6th Mar 2026, dynamic perspectives), v6.2.0 (18th Apr 2026, deployment groups), v6.2.1 (15th May 2026, `element.group==` expressions), v6.2.2 (28th Jun 2026), v6.2.3 (17th Sep 2026), v6.2.4 (26th Oct 2026).
- **changelog-application.md** = the application ("local/server/playground/export/…" commands), calendar-versioned: **2026.02.01**, 2026.03.06, 2026.04.19, 2026.05.16, 2026.05.22, 2026.06.28, 2026.09.19, 2026.10.26, plus `unreleased` sections (local fix #137; **"Integrates the previously standalone MCP server via the new mcp command"**). v6.x library bumps are called out in app entries ("Updates Structurizr for Java library to v6.2.1").
- The changelogs themselves do NOT narrate the repo consolidation (they start fresh at the monorepo). Corroborating facts (fetched from GitHub):
  - `structurizr/java` (Gradle, Apache-2.0, 1.1k stars): last commit 2026-02-01; README: "The code in this repo has been moved to https://github.com/structurizr/structurizr"; **archived by owner on 2026-03-28**. Its old changelog (fetched): v2.0.0 (22 Feb 2024) → 2.1.x → 3.0.0 (19 Sep 2024) → 3.1.0 (4 Nov 2024) → 3.2.0/3.2.1 (Dec 2024) → 4.0.0 (28 Mar 2025) → 4.1.0 (28 May 2025) → **5.0.0 (28 Oct 2025)** → 5.0.1–5.0.3 (Nov 2025) ⇒ monorepo **skips from 5.0.3 to 6.0.0** and switches Gradle→Maven, Java 17→21.
  - `structurizr/cli` README: "The Structurizr CLI will not receive any further updates – please migrate to the new consolidated tooling. See **Introducing Structurizr vNext** (patreon.com/posts/146923136) and docs.structurizr.com/commands." Last activity 2026-02-01.
  - `structurizr/lite` README: same "no further updates … see Introducing Structurizr vNext … docs.structurizr.com/local" (Lite's `STRUCTURIZR_WORKSPACE_PATH`, auto-sync explicitly removed in app changelog 2026.02.01).
  - `structurizr/ui` (MIT): README unchanged; its code now lives as prebuilt JS bundles in `structurizr-application/src/main/resources/static` (structurizr-*.js). `structurizr/onpremises` (MIT, closed-ish extensions model) still a separate repo.
  - Net story: "Structurizr vNext" consolidation (announced via Patreon, early Feb 2026): library (java), CLI, Lite, Server/onpremises UI and the MCP server all folded into `structurizr/structurizr` as one Maven build producing library jars + one `structurizr.war` with commands (`local` replaces Lite, `server` = open-core on-prem, `playground`, `mcp` added unreleased 2026).

---

## 2. MODULE DEPENDENCY GRAPH (from module poms; compile scope unless noted)

```
annotation  -> (nothing; annotations only)
core        -> (jackson-annotations, jsr305, commons-logging)
client      -> core            (+ jackson-databind, httpclient5, jaxb-api; test: testcontainers)
export      -> core            (test: client)
import      -> client          (=> transitively core)
autolayout  -> export          (test: client)
component   -> core            (+ bcel, javaparser; test: annotation)
inspection  -> core            (test: dsl)
dsl         -> client, import, export, component   (test: groovy-jsr223, kotlin-scripting-jsr223, jruby-core)
application -> dsl:6.2.4, inspection:6.2.4         (pinned literal versions; excludes commons-logging)
```
- All library deps use `${project.version}`; the reactor version is 6.2.4 everywhere (annotation etc. inherit parent version).
- **structurizr-application is odd**: parent = `org.springframework.boot:spring-boot-starter-parent:4.1.2` (NOT the aggregator), version `${app.revision}` (default `1.0.0`), packaging `war`; depends on the *library artifacts* `com.structurizr:structurizr-dsl:6.2.4` and `structurizr-inspection:6.2.4` (matches reactor version so it resolves in-reactor).

## 3. MODULE-BY-MODULE

### structurizr-annotation  (com.structurizr:structurizr-annotation:6.2.4)
- ~70 LOC, no deps. Base pkg `com.structurizr.annotation`.
- 5 java annotations: `@Component`, `@Tags`, `@Tag(name)`, `@Properties`, `@Property(name,value)` — markers for the component finder.

### structurizr-core  (structurizr-core:6.2.4) — 108 main files / 14,349 LOC; 61 test files / 11,516 LOC; test resources: only image.svg + image.png
- Pkgs: `com.structurizr` (Workspace, AbstractWorkspace, PerspectivesHolder, PropertyHolder, WorkspaceValidationException), `.model`, `.view`, `.configuration`, `.documentation`, `.validation`, `.util`.
- Key classes:
  - `AbstractWorkspace`: id(long), name, description, **version(String)**, lastModifiedDate/User/Agent, thumbnail, `properties` map, `configuration`; `Workspace extends AbstractWorkspace`: model + viewSet + documentation; `trim()`, `hydrateModel/hydrateViewSet` (relink after JSON load), `validate()`.
  - `model/Model` (people/softwareSystems/deploymentNodes + relationships lists, `addPerson`, `getElements()`, canonical names via `CanonicalNameGenerator`, `IdGenerator` strategy, `ImpliedRelationshipsStrategy` — Default/CreateImpliedRelationshipsUnlessSameRelationshipExists/CreateImpliedRelationshipsUnlessAnyRelationshipExists), `Element` hierarchy: Person, SoftwareSystem, Container, Component (StaticStructureElement...), CustomElement, GroupableElement; deployment: DeploymentNode, InfrastructureNode, SoftwareSystemInstance, ContainerInstance (StaticStructureElementInstance/DeploymentElement); `Relationship` (id, description, technology, interactionStyle Synchronous/Asynchronous, linkedRelationshipId, hasExplicitOrdering?).
  - **`model/Location` enum: `Internal, External, Unspecified`** (Person/SoftwareSystem `location`; enterprise concept removed in v5/6).
  - **`model/Tags` constants**: `Element`, `Relationship`, `Person`, `Software System`, `Container`, `Component`, `Deployment Node`, `Infrastructure Node`, `Software System Instance`, `Container Instance`, `Synchronous`, `Asynchronous`. Implicit tag stacks are built in each class's constructor/addTags (Element adds "Element", etc.).
  - **`model/SequentialIntegerIdGeneratorStrategy` (default `IdGenerator`): single counter `ID` shared by elements AND relationships (`++ID` for both); `found(id)` bumps counter past numeric ids, ignores non-numeric.**
  - `view/ViewSet`: all view collections + styles/configuration; `createDefaultViews()` (called by server/local/playground when a workspace has no views — generates system context per software system etc.); view classes: SystemContextView, SystemLandscapeView, ContainerView, ComponentView, DynamicView, DeploymentView, ImageView, CustomView, FilteredView (+ FilterMode Include/Exclude); `ModelView`/`StaticView`/`View` base, `ElementView`/`RelationshipView`, `Styles`/`ElementStyle`/`RelationshipStyle` (color, stroke, shape, icon, metadata, iconPosition, `light`/`dark` variants), `AutomaticLayout` (Graphviz removed v6 — rank direction/ranksep/nodesep only), Animation, `Configuration` (branding removed; Terminology, ColorScheme).
  - `configuration/WorkspaceConfiguration` (users/visibility; `WorkspaceScope` enum used by `validation/WorkspaceScopeValidatorFactory`), `documentation/{Documentation,Section,Decision,Image,Format,Documentable}`.
  - `util/SystemLandscapeGenerator` (for `generate system-landscape`), `Features` (`structurizr.feature.*` names).
- Jackson **annotations live in core** (e.g. `@JsonIgnore` on computed getters) — JSON shape = Jackson bean serialization of core objects (done in client).
- Tests: per-class behavior tests (ModelTests, RelationshipTests, ViewSetTests incl. `createDefaultViews`, WorkspaceTests); NO JSON golden files in core.

### structurizr-client  (structurizr-client:6.2.4) — 31 main / 2,286 LOC; 19 test / 1,569 LOC
- Base pkgs: `com.structurizr.io[.json]`, `.api`, `.http`, `.encryption`, `.util`, `.view`.
- **THE JSON (DE)SERIALIZER** (no "JsonMapper" class anymore): `io/json/JsonReader` + `JsonWriter` (thin wrappers over Jackson `ObjectMapper`), `AbstractJsonWriter.createObjectMapper`: `SORT_PROPERTIES_ALPHABETICALLY`, `NON_NULL` **and** `NON_EMPTY` inclusion, date format `yyyy-MM-dd'T'HH:mm:ss'Z'` in UTC, pretty print with 2-space indent + `\n` line endings. `EncryptedJsonReader/Writer` for encrypted workspaces.
- `io/WorkspaceReader/Writer` interfaces; `util/WorkspaceUtils.loadWorkspaceFromJson/saveWorkspaceToJson`.
- `api/WorkspaceApiClient(url, apiKey)`: GET/PUT `/api/workspace/{id}` (+ branch variants), `putImage()`; `AdminApiClient`; auth now **`X-Authorization: <apiKey>` header** (`api/HttpHeaders.X_AUTHORIZATION`, User-Agent) — old HMAC scheme gone (v6 "API key replaces HMAC"); `AbstractApiClient` uses httpclient5.
- `encryption/AesEncryptionStrategy`, `EncryptedWorkspace`, `EncryptionLocation`.
- `view/ThemeUtils` (theme loading incl. embedded/base64, `InstalledThemes` support in core), `http/HttpClient` (also used by DSL for `!include` URLs).
- Tests: `io/json/JsonTests`+`JsonWriterTests`+Encrypted*; test resources: `structurizr-36141-workspace.json`, `backwardsCompatibility/structurizr-31-workspace.json`, `backwardsCompatibility/structurizr-36141-workspace.json`, `backwardsCompatibility/structurizr-39459-workspace.json`, `backwardsCompatibility/views-without-order.json` (JSON back-compat fixtures!), `workspaceValidation/*.json` (16 validation-error fixtures), theme.json, logo. API integration tests use testcontainers (tagged `IntegrationTest`).

### structurizr-dsl  (structurizr-dsl:6.2.4) — 168 main / 11,524 LOC; 83 test / 13,649 LOC; 183 test-resource files
- Deps: client, import, export, component (compile); test-only script engines groovy-jsr223, kotlin-scripting-jsr223, jruby-core (for `!script`). **No ANTLR — hand-rolled tokenizer + keyword dispatch.**
- Pkg `com.structurizr.dsl` (~168 flat classes). Key classes:
  - **`StructurizrDslParser`** ("Main DSL parser class – forms the API"): `parse(File)/parse(String)`, `getWorkspace()`; preprocesses lines (strips `//`/`#` comments, `/* */` multi-line handled per-line, `\` line continuation, `"""` text-blocks → hidden constants, UTF-8 BOM); `${NAME}` string substitution via `substituteStrings()` — constants/vars first, then **environment variables** when `Features.ENVIRONMENT` enabled (there is **no `env()` function** in current DSL; `${VAR}` is the mechanism; Feature name `structurizr.feature.dsl.environment`). Tracks `dslPortable` and stores DSL source in workspace properties (`structurizr.dsl.source`, `structurizr.dsl.identifier`) only if portable. `IdentifierScope Flat|Hierarchical` (`!identifiers hierarchical`). `extend`/`!extend` (deprecated tokens) support extending JSON workspaces; constants/archetypes inherited on extension.
  - **`Tokenizer`** (quoted-string aware splitter, respects `\"\"\"`), **`StructurizrDslTokens`** (all keyword constants), `Tokens`, `DslLine`, `DslParserContext`.
  - Context/parser dispatch: `DslContext` base + per-scope contexts (`WorkspaceDslContext`, `ModelDslContext`, `PersonDslContext`, `SoftwareSystemDslContext`, `ContainerDslContext`, `ComponentDslContext`, `DeploymentNodeDslContext`, `InfrastructureNodeDslContext`, `ContainerInstanceDslContext`, `SoftwareSystemInstanceDslContext`, `RelationshipDslContext`, `ViewsDslContext`, `ViewDslContext` subcontexts per view type, `StylesDslContext`, `ConfigurationDslContext`, `GroupableDslContext`, `DeploymentElementParser` …) — one `XxxParser` class per keyword (registered via reflection in `DslParserContext`).
  - Keywords (`StructurizrDslTokens`, exact strings): `workspace model views configuration styles default person softwareSystem container component custom relationship -> -- -/> description technology url group scope enterprise(deprecated) people systems deployment deploymentNode infrastructureNode instanceOf softwareSystemInstance containerInstance deploymentEnvironment deploymentGroup instances dynamic systemContext systemLandscape filtered filter include exclude * animation animationStep position parallel autolayout routing metadata metadataSymbols? title theme themes style element relationship tag tags color/colour background stroke strokeWidth opacity shape icon iconPosition border width height fontSize shape ... terminology visibility users strategy matcher forEach supportingTypes healthCheck image plantuml mermaid kroki light dark perspective perspectives !include !docs !adrs !decisions !const !var !constant(removed→error) !identifiers !impliedRelationships !ref(deprecated) !extend(deprecated) !element !elements !relationship !relationships !plugin !script !components this name value extends` etc.
  - `!include` handled by **`IncludeParser`** (file/dir/URL; `.dsl` fragments, whole workspaces, JSON workspaces; docs dirs via `!docs`→`DocsParser`/import module; `!adrs`/`!decisions`→`DecisionsParser` + madr/log4brains/adrtools importers); `!const`/`!var` via `NameValueParser`/`NameValuePair`; `!element(s)`/`!relationship(s)` via FindElement(s)/FindRelationshipsParser + `ExpressionParser`/`StructurizrDslExpressions` (expressions like `element.name==`, `element.group!=`, `->container->`, `element.technology==Java`); archetypes (`Archetype`, `ArchetypeParser`, per-type archetype contexts); `!plugin` (`StructurizrDslPlugin` + `StructurizrDslPluginContext`), `!script` (`ScriptParser` JSR-223 + `StructurizrDslScriptContext` — exposes parser/workspace/parameters, no env function), `!components` (`ComponentFinderDslContext` + strategy parsers wrapping structurizr-component), `ImpliedRelationshipsParser` (class name or strategy name), `InstanceOfParser`.
  - Errors: **`StructurizrDslParserException`** — message "... at line N of /abs/path: <trimmed line>" (file+line, **no column**; `getLineNumber()`).
- **Test layout / fixtures (future compat goldens)**:
  - `src/test/java/com/structurizr/dsl/*Tests.java` — 83 test classes; per-parser unit tests (tokens → in-memory workspace assertions via `AbstractTests.context()`), plus `DslTests.java` (full-file parse tests, e.g. `src/test/resources/dsl/getting-started.dsl` → counts/people/views/keys like `SystemContext-001`), `TokenizerTests`, `TokensTests`, `IdentifierScopeParserTests`, `IdentifierRegisterTests`, `CookbookTests` (walks `docs/cookbook/**/example-*.dsl` — the dir is NOT in the repo, so it's a no-op unless docs are placed there).
  - `src/test/resources/dsl/` — **107 `.dsl` fixtures total** (84 top-level + 23 nested). Naming pattern: kebab-case feature names: archetypes*.dsl, big-bank-plc.dsl, bulk-operations.dsl, constant.dsl, deployment-{environment-empty,groups-flat,groups-hierarchical,view-animation}.dsl, dynamic*.dsl, enterprise.dsl, exclude-*.dsl, filteredviews.dsl, find-*.dsl, getting-started[-short].dsl, group*.dsl, hierarchical-identifiers*.dsl, identifiers.dsl, image-view.dsl, include-{directory,file,url,implied-relationship}.dsl, iso-8859.dsl, multi-line[-with-error].dsl, multiple-{model,view,workspace}-tokens.dsl, no-relationship.dsl, parallel[123].dsl, plugin-*.dsl, relationship-*.dsl, script-{inline,external,external-with-parameters,in-dynamic-view}.dsl, shapes.dsl, source-{child,parent,not-retained}.dsl, static-view-animation.dsl, test.dsl, text-block.dsl, this.dsl, unexpected-tokens-*.dsl, utf8.dsl, views-without-keys.dsl, workspace-{properties,with-bom,with-bom-model}.dsl, amazon-web-services[-local].dsl, financial-risk-system.dsl, color-schemes.dsl …
  - Nested fixture dirs: `dsl/big-bank-plc/` and `dsl/spring-petclinic/` (multi-file `!include` workspaces w/ model+views dirs, docs, `.structurizr/` index+images incl. `workspace.json`), `dsl/include/` (model/software-system fragments, docs/section.md), `dsl/docs/` (workspace/softwaresystem/container/component markdown), `dsl/decisions/{madr,log4brains,adrtools}`, `dsl/extend/` (+workspace.json), `dsl/image-views/` (diagram.puml, image.svg/png), `dsl/plugins/structurizr-dsl-plugin-1.0.0.jar` (binary plugin for `!plugin` tests), scripts `test.groovy|js|kts|rb|test-with-parameters.groovy`; `src/test/resources/themes/theme.json`.
  - **Behavior verification = in-code assertions, NOT golden JSON** (only 3 .json test files). Golden JSON comparisons live in export tests (`amazon-web-services.dsl/json`, `big-bank-plc.json`, `groups.dsl/json`) and client back-compat JSONs. `@Tag("IntegrationTest")` on ~10 DSL tests (Kroki/kroki.io, image rendering).

### structurizr-export  (structurizr-export:6.2.4) — 32 main / 5,054 LOC; 14 test / 9,288 LOC
- Deps: core (compile), client (test). Pkgs `com.structurizr.export{,.plantuml,.mermaid,.dot,.websequencediagrams,.ilograph}`.
- Key: `AbstractDiagramExporter`/`AbstractExporter`/`AbstractWorkspaceExporter`, `Diagram` (name/puml/plantuml? content + format), exporters: `StructurizrPlantUMLExporter`, `C4PlantUMLExporter` (+ `AbstractPlantUMLExporter`, style classes PlantUML{Root,Element,Relationship,Group,Boundary,DeploymentNode,Legend}Style), `MermaidDiagramExporter`, `DOTExporter`, `WebSequenceDiagramsExporter`, `IlographExporter` (+ `IlographWorkspaceExport`), `Legend`, `IndentingWriter`/`IndentType`.
- Tests: big string-content assertions (e.g. C4PlantUMLDiagramExporterTests) + IntegrationTest-tagged rendering; resources: `amazon-web-services.dsl/json`, `big-bank-plc.json`, `groups.dsl/json` (parse→export→optional render).

### structurizr-import  (structurizr-import:6.2.4) — 18 main / 1,628 LOC; 14 test / 1,555; 63 test-resource files
- Dep: client. Pkgs `com.structurizr.importer.{documentation,diagrams[.plantuml|.mermaid|.kroki|.image]}`.
- Key: `DefaultDocumentationImporter`, `RecursiveDefaultDocumentationImporter`, `FormatFinder` (markdown/asciidoc), decision importers `MadrDecisionImporter`, `Log4brainsDecisionImporter`, `AdrToolsDecisionImporter` (+`AbstractDecisionImporter`), `DefaultImageImporter` (with `reluctant` property), diagram importers PlantUML/Mermaid/Kroki (encode→URL via kroki.io for PNG/SVG), `ImageImporter` base.
- Test resources mirror docs/decisions/diagrams fixture trees (madr, log4brains, adrtools).

### structurizr-inspection  (structurizr-inspection:6.2.4) — 52 main / 1,747 LOC; 32 test files
- Dep: core (test: dsl). Pkgs `com.structurizr.inspection{,.model,.view,.workspace,.documentation}`.
- Key: `Inspector`/`Inspection`/`Violation`/`Severity` (Error/Warning/Info/Ignore), `DefaultInspector`, severity strategies (`FixedSeverityStrategy`, `PropertyBasedSeverityStrategy` — `structurizr.inspection` property), ~30 concrete inspections (ElementDescriptionInspection, RelationshipTechnologyInspection, EmptyViewInspection, GeneratedKeyInspection, ManualLayoutInspection, DisconnectedElementInspection, WorkspaceScopeInspection, …). Used by `validate`/`inspect` commands.

### structurizr-autolayout  (structurizr-autolayout:6.2.4) — 7 main / 751 LOC; 3 test files
- Deps: export (compile), client (test). Pkg `com.structurizr.autolayout.graphviz`.
- Key: `GraphvizAutomaticLayout` — **shells out to `dot` via ProcessBuilder/temp files** (graphviz binary, not a Java binding); `DOTDiagram`, `DOTExporter` (duplicate-ish of export's dot pkg), `RankDirection`, `SVGReader` (reads back x/y from graphviz SVG), `StyleUtils`, `Constants`. Note: v6.0.0 removed Graphviz as an auto-layout option *in core* (view `AutomaticLayout` still exists; layout now done in browser via Dagre for server/local; this module remains published & used by e.g. playground export/old flows).

### structurizr-component  (structurizr-component:6.2.4) — 47 main / 2,111 LOC; 52 test files
- Deps: core, annotation (test). "Component finder" — discovers components in a Java codebase. Deps: `org.apache.bcel:bcel:6.12.0` (bytecode) + `com.github.javaparser:javaparser-symbol-solver-core:3.28.2` (source).
- Key: `ComponentFinder(Builder)`, `ComponentFinderStrategy(Builder)`, `TypeProvider`s (ClassDirectory/ClassJarFile/SourceDirectory), `TypeMatcher`s (Annotation/Extends/Implements/NameSuffix/Regex), `SupportingTypesStrategy`s (Default, ImplementationWithSuffix/Prefix, AllTypesIn/UnderPackage, …), `TypeFilter`s, naming/description/url strategies, `TypeFinder`/`TypeRepository`/`TypeDependencyFinder`, `DiscoveredComponent`, `ComponentVisitor`.

### structurizr-application  (com.structurizr:structurizr:**${app.revision}**=1.0.0, war) — 177 main / 11,805 LOC; 63 test / 9,813; 2201 main-resource files (2079 svg icons, 35 js, fonts, css)
- **Spring Boot 4.1.2** (starter-webmvc + security[ldap, saml2] + devtools, spring-session-redis), embedded **Tomcat** (jasper JSP + JSTL — server-rendered JSP views), **Spring AI `spring-ai-starter-mcp-server-webmvc:2.0.2`**.
- Other third-party: commons-cli (command parsing), groovy/kotlin-jsr223/jruby (`!script` in server), Apache **Lucene 10.5.1** (documentation search), jedis + redisson + ehcache + jcache (metadata cache), AWS SDK s3/sts, Azure storage-blob (closed-ish extensions/cloud), elasticsearch-rest-client 7.17 (search adapter alt), bouncycastle, guava, **Playwright 1.62.0** (PNG/SVG export via headless browser; profile `include-playwright` default-on, `exclude-playwright` omits `PlaywrightExporterImpl`), httpclient5/httpcore5, testcontainers (elasticsearch, redis) for IntegrationTests. Extra repo: **Shibboleth** (SAML2). `extensions/src/{main,test}/java` is add-source'ed if present (closed extensions dropped in, not in open repo).
- Entry point: `com.structurizr.Application` (@SpringBootApplication, plain `main`) dispatching commons-cli-style subcommands from a LinkedHashMap: `playground local server mcp push pull lock unlock branches create delete regenerate-apikey export generate merge validate inspect list version help`.
- Commands (`com.structurizr.command.*`): ExportCommand (static-site + plantuml/c4plantuml/mermaid/dot/websequencediagrams/ilograph + `-format png|svg` via `PlaywrightExporterImpl` reflection; `DslWorkspaceExport`, `JsonWorkspaceExport`, `JsonWorkspaceThemeExporter`, `StaticSiteExporter`), GenerateCommand (+GenerateSystemLandscapeSubcommand), MergeCommand (merge DSL/JSON workspaces), ValidateCommand/InspectCommand (structurizr-inspection), Push/Pull/Lock/Unlock/Branches/Create/Delete/RegenerateApiKey/List (WorkspaceApiClient/AdminApiClient), LocalCommand/ServerCommand/PlaygroundCommand/McpCommand (`com.structurizr.server.Local/Server`, `com.structurizr.playground.Server`, `com.structurizr.mcp.Server` each with own main).
- **MCP server** (`com.structurizr.mcp.Server` + `McpCommand`, unreleased section): Spring AI MCP server, protocol `STATELESS` (application.properties `spring.ai.mcp.server.protocol=STATELESS`); tool groups enabled by flags: `-dsl` (validate/parse/inspect), `-server-create/-read/-update/-delete`, `-plantuml`, `-mermaid`; `STRUCTURIZR_THEMES` env var; classes `DslTools`, `PlantUMLTools`, `MermaidTools`, `Server{Create,Read,Update,Delete}Tools`, `AbstractExportTools`.
- Server internals: `server/component/workspace/*WorkspaceAdapter` (LocalFileSystemSingle/Multiple, ServerFileSystem), `server/component/search/*` (Lucene/NoOp adapters), `server/web/**` controllers (diagram viewer/editor, DSL editor, docs/decisions, images, api JSON endpoints `server/web/api/{Local,Server}WorkspaceApiController`), `server/domain/*` (User/Role/Permission/Image/WorkspaceMetadata), `configuration/StructurizrProperties` (`structurizr.xxx` properties / `STRUCTURIZR_XXX` env).
- Static UI: `src/main/resources/static/static/js` — vendored **structurizr-*.js** bundles (structurizr-ui.js, -diagram.js, -editor pieces) + ace, dagre, jointjs, d3, markdown-it, asciidoctor, katex, highlight, crypto-js, bootstrap-icons SVG set.

## 4. BUILD & RUN

- Whole repo (Java 21, Maven wrapper 3.9.11): `./mvnw -DexcludedGroups=IntegrationTest package` (what CI does); skip tests: `./mvnw -Dmaven.test.skip=true package`. IntegrationTests need Docker (testcontainers) / network (kroki.io).
- Run app: `java -jar structurizr-application/target/structurizr-1.0.0.war <command>` (or `./structurizr.sh <command>`); e.g. `local`, `server`, `export -format plantuml …`, `mcp -dsl`.
- Docker: `docker build . -f structurizr-application/Dockerfiles/eclipse-temurin-alpine -t structurizr` (also -noble, -resolute, playwright-java-noble variants; docs: `structurizr/structurizr` image, tags `latest|yyyy.mm.dd`, `-noble`, `-playwright`, `preview`).
- Maven Central: library modules (annotation, autolayout, client, core, component, dsl, export, import, inspection) publish via root central-publishing plugin + GPG. Application = distributable **war/binaries** at download.structurizr.com (`structurizr-2026.09.19.war`, `-playwright` variant), not a Central Java library.
- Versioning: **libraries** semver in root pom `6.2.4` (single version for all modules); **application** = `app.revision` (default 1.0.0), binaries **calver `yyyy.mm.dd`** (`./mvnw package -Dapp.revision=2026.02.01`). Docs: docs.structurizr.com/building.
- Library coordinate change to watch: groupId `com.structurizr`, artifactIds unchanged from old structurizr/java — old v5.x jars and v6.x jars share coordinates (compatibility continuity on Central).

## 5. CODESTATS (find+wc; main vs test java files/LOC)

| module | main java/LOC | test java/LOC | main res | test res |
|---|---|---|---|---|
| annotation | 5 / 70 | 0 | 0 | 0 |
| autolayout | 7 / 751 | 3 / 791 | 0 | 3 (graphviz/, workspace json) |
| client | 31 / 2,286 | 19 / 1,569 | 0 | 23 |
| component | 47 / 2,111 | 52 / 1,503 | 0 | 0 |
| core | 108 / 14,349 | 61 / 11,516 | 0 | 2 |
| dsl | 168 / 11,524 | 83 / 13,649 | 0 | 183 (incl. 107 .dsl) |
| export | 32 / 5,054 | 14 / 9,288 | 0 | 5 |
| import | 18 / 1,628 | 14 / 1,555 | 0 | 63 |
| inspection | 52 / 1,747 | 32 / 1,762 | 0 | 1 |
| application | 177 / 11,805 | 63 / 9,813 | 2,201 | 0 |

Total: 986 java files, ~102.8k LOC. Stack: Java 21, Maven; app = Spring Boot 4.1.2/Tomcat/JSP; no ANTLR anywhere (hand-written DSL tokenizer); graphviz = external `dot` binary via autolayout module; PlantUML/Mermaid = *text generation only* in export (rendering via kroki.io in import/image-views or Playwright/browser in app).

## 6. LICENSING
- Monorepo: **Apache-2.0** (root LICENSE; pom `<licenses>`; no per-module `<licenses>` overrides in any module pom).
- structurizr-themes: theme.json Apache-2.0; **PNG icons remain copyright their owners** (AWS/Azure/GCP/OCI assets) — relevant if vendoring icons.
- Old repos: java Apache-2.0, cli Apache-2.0, lite **MIT**, ui **MIT**, onpremises MIT (from GitHub org page). Docs repo (structurizr-docs) has own LICENSE.
- COMPAT.md caveat: Maven Central artifacts under `com.structurizr` — current monorepo artifacts are Apache-2.0; very old (pre-2.x) artifacts historically GPL per COMPAT.md note.

## 7. Misc facts relevant to oxidizr compat
- DSL parse → in-memory `Workspace`; JSON write path = Jackson bean serialization of core model with alphabetical property sort + NON_EMPTY (empty arrays/objects/strings omitted!) — critical byte-level compat detail.
- Element/relationship ids: one shared sequential counter starting at 1 (`SequentialIntegerIdGeneratorStrategy`); JSON `id` values are strings in model, workspace `id` is number/long, `version` string; view keys auto-generated when absent (view `order` numbers assigned by ViewSet; back-compat loader fills missing `order`).
- DSL source retention: stored as workspace property only when DSL is "portable" (no file/plugin/script references); original-case identifiers via `structurizr.dsl.identifier` property (no longer lower-cased, since v5).
- Deprecated/removed keyword timeline (from old + new changelogs): `!constant` removed in 3.0.0 (Sep 2024; now throws "use !const or !var"), `!ref`/`!extend` deprecated (still tokens, "deprecated"), `enterprise` keyword removed (2.0.0), `branding` removed (6.0.0), `theme default` removed (6.0.0), Graphviz auto-layout removed from core (6.0.0).
- `structurizr-json/structurizr.yaml` (OpenAPI, schema version 6.1.0) = machine-readable JSON schema source of truth.
