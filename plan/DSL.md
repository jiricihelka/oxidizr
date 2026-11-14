# DSL grammar reference (quick reference)

Sources: `structurizr-dsl/.../StructurizrDslTokens.java` (full token list, verbatim
below), per-construct `XxxParser`/`XxxDslContext` classes, docs clone `dsl/71-language.md`
(authoritative grammar page) + `05-identifiers`, `06-archetypes`, `07-implied-relationships`,
`08-expressions`, `31-includes`, `32-workspace-extension`, `51-docs`, `52-adrs`, `61-scripts`.
Pinned v6.2.4 @ `ad34036`.

## 1. Lexing / preprocessing

- Line-based. Tokenizer is quote-aware (`"…"` strings, `\"` escapes); unquoted runs are
  single tokens. No ANTLR — keyword dispatch per scope context.
- Preprocess pass: strip `//` and `#` comments (full-line) and `/* … */` blocks; join `\`
  continuations; `"""…"""` text blocks become hidden constants; UTF-8 BOM stripped
  (ISO-8859/Latin-1 files also supported); `${NAME}` substitution over ALL strings:
  `!const`/`!var` map first, then **env vars** (feature `structurizr.feature.dsl.environment`).
- `${NAME}` name chars `[a-zA-Z0-9-_.]`. No `env()`/`$()` function (legacy, gone).
- **M1-verified corrections** (from `preProcessLines` @ `ad34036`, implemented + tested in
  `oxidizr-dsl/src/preprocess.rs`; plan/PROGRESS.md M1 has the details): middle lines of a
  `\`-continuation keep their indentation (only the final completing line is `stripLeading`ed);
  a joined logical line is reported under its LAST physical line's number; a text block's
  value = text after the FIRST `"""` (incl. opener-line tail) + dedented middle lines each
  +`\n`, truncated at the first interior `"""`, last char dropped; text-block lines are
  classified BEFORE blank/comment checks; the whole-line comment pattern is
  `^\s*(//|#).*$`; the block-comment machine's check order is load-bearing (whole-line
  `/* … */` neither opens nor closes a region; first-token `/*` opens — after the
  `x =`-assignment strip; trimmed-line `*/` closes).
- Parse aborts on FIRST error: `StructurizrDslParserException` =
  `"<msg> at line N of <abs-path>: <trimmed source line>"` — no column.
- Per-scope context classes each define a permitted-token set; unknown statement there →
  `"<token> is not valid here"`.

## 2. Statement forms

- Identifier via assignment (optional): `id = keyword <positional args> { children }`.
  Identifier regex: starts word char, then `[a-zA-Z0-9_-]*`; must be unique.
  `!identifiers flat` (default) | `!identifiers hierarchical` (ids = dotted parent path).
- Positional args are quoted or unquoted strings, order-significant (see table).
- `this` = element in current scope (relationship source position).
- Relationships: `[id = ]a -> b ["description" ["technology"] ["tags"]] { tags|url|properties|perspectives }`;
  implicit source `-> b …` inside element scope; archetype arrow `a --name-> b …`;
  `-/-> b` removes a replicated model relationship inside deployment scope.
- Element args (order matters! `person` has NO technology):

| Statement | Positional args |
| --- | --- |
| `person` | name [description] [tags] |
| `softwareSystem` | name [description] [tags] |
| `container` | name [description] [technology] [tags] |
| `component` | name [description] [technology] [tags] |
| `element` (custom) | name [metadata] [description] [tags] (tags default `Element`) |
| `deploymentNode` | name [description] [technology] [tags] [instances] |
| `infrastructureNode` | name [description] [technology] [tags] |
| `softwareSystemInstance` / `containerInstance` / `instanceOf` | <element identifier> [deploymentGroups] [tags] |
| `healthCheck` | name url [interval(s)=60] [timeout(ms)=0] |
| `deploymentEnvironment` | [name] [description] (id = UUID internally) |
| `group` | `"Name"` then nested definitions |

- Common children of element/relationship blocks: `name`, `description`, `technology`,
  `metadata`/`tags`/`tag` (all add tags; `metadata` is an alias), `url`,
  `properties { k v }`, `perspective "n" [desc] [value] [url]`, `group "Name"` (sets group),
  nested kind statements, relationship statements. `instances N` child on deploymentNode.
- View children: `title`, `description`, `properties`, `autolayout
  [rankDirection [rankSep [nodeSep [edgeSep [vertices]]]]]`, `include …`/`exclude …`,
  `animation { animationStep N { elements … relationships … } }`, `styles` only in
  `views` scope. `default { … }` in views scope = defaults for generated views? (verify).

## 3. Scopes and top level

- Bare top level ok (implicit workspace) or `workspace "name" ["desc"] { … }`;
  `workspace "…" extends "file.dsl|file.json|url" { … }` (overlay; see docs 32).
- Scopes: `model { }` (also allowed at top level: person/softwareSystem/
  deploymentEnvironment/group/relationships/properties/!include directly),
  `views { }`, `configuration { }`.
- `scope shared { }` — relaxes one-parent/scope validation (also `-scope shared` via
  `configuration.visibility`? verify token mapping: `visibility` token exists).
- Model scope children: `person`, `softwareSystem`, `deploymentEnvironment`, `group`,
  `archetypes`, `properties`, relationship statements, plus all `!directives`.
- SoftwareSystem scope: `container`, `group`, `archetypes`? + children; Container scope:
  `component`, `group`; Component scope: children only.
- `deploymentEnvironment` scope: nested `deploymentNode` trees; node scope: `deploymentNode`,
  `infrastructureNode`, `softwareSystemInstance`, `containerInstance`, `instanceOf`,
  `group`, `healthCheck`, `deploymentGroup "N"`, relationships, description/technology/
  instances/tags/url/properties/perspectives.
- Views scope: view-creation statements (below) + `styles` + `themes` + `default`.
- Configuration scope: `properties`, `terminology`, `visibility`, `users` (deprecated),
  `metadata` (metadata symbols), development mode? (verify: no token seen).

## 4. View creation statements

| Statement | Args | Notes |
| --- | --- | --- |
| `systemLandscape` | [key] ["title"] ["desc"] | key optional (`views-without-keys.dsl` proves auto keys) |
| `systemContext` | <system> [key] [title] [desc] | |
| `container` | <system> [key] [title] [desc] | |
| `component` | <container> [key] [title] [desc] | |
| `dynamic` | <element> [key] [title] [desc] | body: ordered `a -> b "msg" [tech]`, `parallel { }`, include/exclude |
| `deployment` | ["env"] [key] [title] [desc] | scoped by environment name |
| `filtered` | <baseKey> <key> "title" ["desc"] | body: `include "<filter>"` / `exclude "<filter>"` |
| `custom` | vendor tooling? … (verify exact arg list) | CustomView |
| `image` | — `{ plantuml/mermaid/kroki/image … }` | content blocks: `plantuml { …raw… }`, `mermaid { … }`, `kroki "type" { … }`, `image "path|url"`; light/dark variants (`light`/`dark` tokens) |

- `include *` adds everything in scope; include/exclude take identifiers, name paths,
  finders (`!element`/`!elements`), filter/expression strings. Singular forms only — the
  `includes`/`excludes` of pre-v6 docs are rejected at the pinned revision (verified
  against the 6.2.4 parser, 2026-11-14).

## 5. Styles & themes (in `views`)

- `styles { element "Tag" { … } relationship "Tag" { … } }`.
- Element style children: `shape` (Box RoundedBox Circle Ellipse Hexagon Diamond Cylinder
  Bucket Pipe Person Robot Folder WebBrowser Window Terminal Shell
  MobileDevicePortrait MobileDeviceLandscape Component), `background`, `stroke`,
  `strokeWidth`, `colour`/`color` (both tokens valid; JSON key is `color`), `icon`
  (URL/file/data-URI; file breaks portability), `iconPosition`, `opacity`,
  `border` (Solid Dashed Dotted), `fontSize`, `width`, `height`,
  `metadata { display? fontSize }`?, `description true|false`.
- Relationship style children: `thickness`, `colour`/`color`, `dashed true|false`,
  `line style`/`style` Solid|Dashed|Dotted, `routing` (verify enum: Orthogonal/Polyline/Curved),
  `jump`, `position` (0-100 label), `opacity`, `fontSize`, `width`, `metadata`,
  `description`.
- `themes { url "…" }` lines; `theme "name"` (deprecated; →
  `https://static.structurizr.com/themes/<name>/theme.json`); default theme URL const
  `ThemeParser.DEFAULT_THEME_URL`; installed themes dir `$STRUCTURIZR_THEMES` or
  `./themes`.

## 6. Directives (verbatim tokens)

| Directive | Syntax / semantics |
| --- | --- |
| `!include` | `!include <file\|dir\|url>` — `.dsl` fragment (person/rels/… no workspace wrapper) or full workspace; JSON files ok; relative to including file; nestable |
| `!docs` | `!docs <dir> [exclude <glob>…]` → markdown/asciidoc doc sections (docs 51) |
| `!adrs` / `!decisions` | `<dir> [exclude…]` — ADR import; format auto-detected: structurizr / adr-tools / MADR / log4brains (docs 52) |
| `!const` | `!const "NAME" "value"` — immutable, redefinition = error |
| `!var` | `!var "NAME" "value"` — redefinable |
| `!constant` | removed (3.0) — token kept only to raise "use !const or !var" |
| `!identifiers` | `flat` (default) \| `hierarchical` |
| `!impliedRelationships` | `[true\|false\|<fqcn\|strategy-name>]`; default ON, strategy unless-any-relationship-exists; fqcn breaks portability |
| `!ref` / `!extend` | deprecated legacy (pre-`extends`) — expect clean deprecation error |
| `!element` / `!elements` | finders by name path / filter expression; usable as relationship endpoints and in view include/exclude |
| `!relationship` / `!relationships` | same for relationships |
| `!components` | `!components "<system>" "<container>" { classes <source-dir\|jar> [filter …] strategy { matcher { technology… } supportingTypes { … } name/description/url strategies forEach … } }` — Java-only discovery (low priority for oxidizr) |
| `!plugin` | `!plugin <fqcn>` — SPI `StructurizrDslPlugin` from classpath/plugins dir |
| `!script` | inline `!script { … }` or `!script file.ext` — JSR-223 (js/groovy/kts/rb in tests); exposes parser/workspace/parameters |

## 7. Archetypes (docs 06)

- `archetypes { }` in model scope; `springBootApplication = application { description
  "…"; technology "Java Spring Boot"; tag "Spring Boot"; … }`; bases: person,
  softwareSystem, container, component; archetypes may extend archetypes.
- Use as keyword: `api = springBootApplication "API Application" …`.
- Relationship archetypes: `sync = -> { technology "HTTPS" }`; chain `https = --sync->
  { … }`; use as arrow: `a --https-> b "desc"`.
- Custom element archetypes supported (`archetypes-for-custom-elements.dsl`).

## 8. Expressions / filters (docs 08; core `filter/`)

- Used by `!elements`, view `include`/`exclude`, filtered views.
- Shorthands: tag string (`"Mobile"`), type (`"Software System"`? verify), `element.name==X`,
  `element.group!=X`, `element.technology==Java`, direction `->container->` …
- v6.2.1 added `element.group==name`.

## 9. Portability & identifier retention (features §5 in FEATURES.md)

- Portable ⇒ writer adds workspace `properties["structurizr.dsl"]` (base64 flattened DSL)
  and per-element `properties["structurizr.dsl.identifier"]` (rels only when explicitly
  named). Non-portable triggers: extends/!include/!script/!plugin/!docs/!adrs/!components/
  fqcn implied/ file icons&themes&logo. `structurizr.dsl.source=false` → discard.

## 10. Full token inventory (verbatim, `StructurizrDslTokens`)

```text
= element person softwareSystem -> -/> container component group name metadata description
technology instances tags tag url properties perspectives perspective [value url desc]
workspace extends scope model archetypes views enterprise(->error) deploymentEnvironment
deploymentGroup deploymentNode infrastructureNode instanceOf softwareSystemInstance
containerInstance healthCheck custom systemLandscape systemContext container component
dynamic deployment filtered image include exclude animation animationStep autolayout
default title [description] plantuml mermaid kroki image styles light dark element shape
background stroke strokeWidth colour color icon iconPosition opacity border fontSize width
height metadata description relationship thickness dashed routing jump style position
theme themes configuration visibility terminology metadata(symbols) users this
!include !docs(exclude) !adrs !decisions(exclude) !constant !const !var !identifiers
!impliedRelationships !ref(deprecated) !extend(deprecated) !element !elements
!relationship !relationships !plugin !script !components [+ classes source filter
strategy technology matcher filter supportingTypes name description url forEach]
archetype arrow pattern: --<name>->
```
