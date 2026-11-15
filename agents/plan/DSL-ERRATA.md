# DSL reference errata (found during M4 implementation)

Corrections to `agents/plan/DSL.md` / `agents/plan/FEATURES.md` found by reading the pinned
reference source (`structurizr` monorepo @ `ad34036`) while implementing P1-M4.
The Java code remains the contract; these notes fix the distilled reference so it
doesn't mislead later milestones. Each entry cites the class that settles it.

## Grammar / dispatch (`StructurizrDslParser.processLine`)

1. **Identifier prefix is `id = <statement>`, not `[id]`** — the parse loop strips
   `tokens[0]` when `tokens[1] == "="` (needs ≥3 tokens; a bare `x =` is NOT
   stripped). `IdentifiersRegister.validateIdentifierName`: pattern
   `\w[a-zA-Z0-9_-]*` (leading `-` special-cased). Wrong in DSL.md §2/§3.
2. **Arrows: only `->` is a relationship token** (`RELATIONSHIP_TOKEN = "->"`), plus
   archetype arrows `--<name>->` (prefix `--`, suffix `->`) and `-/>`
   (`NO_RELATIONSHIP_TOKEN`, only inside `deploymentEnvironment` with `{`).
   `-->`/`..>` are NOT tokens — they lex as unquoted single tokens and fail
   dispatch. Wrong in DSL.md §3.
3. **Error text for invalid statements** is `Unexpected tokens (expected: a, b, …)`
   (comma list from the context's `getPermittedTokens()`) or plain
   `Unexpected tokens` when the set is empty — NOT "X is not valid here" (DSL.md
   §12). Every per-line exception (incl. `RuntimeException`s) is wrapped as
   `StructurizrDslParserException(e.getMessage() | e.getClass().getSimpleName(),
   file, lineNumber, line)`. The "is not valid here" string does not exist.
4. **Statement dispatch is a single ordered if-else chain** on `(firstToken,
   context-class)` — NOT a per-scope keyword table; the permitted-token lists exist
   only for the "Unexpected tokens" message (and they lag the dispatch — e.g.
   `properties` inside `model` is accepted although not listed). The oxidizr parser
   mirrors the chain order.
5. `deploymentEnvironment { }` accepts ONLY `group | deploymentGroup | deploymentNode
   | <id> -> rel | }` (DeploymentEnvironmentDslContext) — no `name/description/
   technology/instances/properties` (DSL.md §10 wrong).
6. **Group scopes are not statement scopes**: inside `group { }` the `tags/
   description/url/properties/perspectives` branches are guarded by
   `!isGroup(context)`, so those statements are NOT valid inside a group scope;
   groups only nest definitions (and `group` on a `component` is the two-token
   `group <name>` property form, nested groups require the
   `structurizr.groupSeparator` model property).
7. `inContext(X)` tests the TOP of the context stack only (subtype match).
8. Relationship statements are arrow-based; `a -> b` dispatches in
   `model|deploymentEnvironment|element` scopes (`tokens[1]` is the arrow),
   `-> b` (implicit source) inside element scopes; inside `!elements { }` scopes
   both forms fan out over the element set. `relationship` is not a statement
   keyword anywhere in the model scope.
9. **`id = person …` registers after the statement executes** (post-parse
   `registerIdentifier`); an unregistered element is still registered under a
   random UUID (no `structurizr.dsl.identifier` property written). Duplicate
   identifiers are NEVER silently overwritten:
   `register()` throws `The identifier "x" is already in use` (case-insensitive,
   across elements AND relationships), or `The element is already registered with
   an identifier of "y"` / `Please assign an identifier to "…" before using it`
   (UUID case). Wrong in DSL.md §3.
10. Element archetype statements use the archetype name IN PLACE OF the keyword:
    `paymentProvider = person { … }` inside `archetypes { }` (name = identifier,
    keyword marks the type; re-using an existing archetype token extends it);
    usage is `paymentProvider "Name" …` — no `person` keyword (DSL.md §4 form
    `person name archetype x` is wrong). Relationship archetypes: `https = -> { }`,
    used as `a --https-> b`.
11. `!impliedRelationships` dispatches in ANY context (no `inContext` guard).
12. `custom <name>` view is `custom <name> [description] [key] {`; `systemLandscape
    [key] [description] {`; `dynamic <*|sysId|containerId> [key] [description] {`;
    `deployment <*|sysId> <environment> [key] [description] {`; `filtered <baseKey>
    <include|exclude> <tags> [key] [description] {` (base must be a
    static/deployment view WITHOUT automatic layout).
13. `person` scope has no `technology` statement (confirmed); `element` (custom)
    declaration is `element <name> [metadata] [description] [tags]` (DSL.md §2
    positional table: add metadata column).
14. `healthCheck <name> <url> [interval] [timeout]`: DSL defaults interval=60,
    timeout=0 (interval must be ≥1). (JSON-read defaults 0/0 stay as recorded in
    WRITE-CATALOG.)
15. `animationStep`/`animation { }` exist in static, deployment AND custom views
    (also `CustomViewAnimationDslContext`); dynamic views have `animation`… NOT
    — dynamic views have no animation; `filtered` views accept only
    `default|title|description|properties`.
16. Dynamic view content accepts an optional `order:` prefix token
    (`1:`, `2.1:`… — anything ending in `:`), an `a -> b [desc] [technology]` form,
    or `<relationshipIdentifier> [description]`; parallel sequences open with a
    bare `{` line and close with `}`; numbering follows
    `SequenceNumber`/`SequenceCounter` (nested counters, `endParallelSequence
    (!hadRelationships)`).
17. Styles `light { }`/`dark { }` sub-scopes (`ColorScheme`-scoped element/
    relationship styles) exist in v6 (`light`/`dark` tokens inside `styles`).
18. `!icon`-style `icon <file|url>` (element style) inlines files as
    `data:image/…` (non-portable) — exporter-adjacent but parser-side.

## Core semantics the DSL side must reproduce (`structurizr-core`)

19. `Model.addPerson/addSoftwareSystem` THROW `A top-level element named 'X'
    already exists.` on duplicate top-level names (find-or-create only when
    `workspace extends`).
20. Duplicate explicit relationships: `Model.addRelationship` returns null when
    `source.has(rel)` (equals = source+dest+description+technology), and the DSL
    layer turns null into `A relationship between "A" and "B" already exists`.
    Parent-child relationships throw `Relationships cannot be added between
    parents and children.` (IllegalArgumentException).
21. **DSL workspaces default to
    `CreateImpliedRelationshipsUnlessAnyRelationshipExistsStrategy`** (set in
    `WorkspaceParser.parse`); implied relationships are created synchronously at
    explicit-relationship time for static↔static pairs, tags = default
    `Relationship` only, `linkedRelationshipId` = base id, never self/ancestor-
    descendant pairs (persons exempt from the ancestor rule).
    `!impliedRelationships false|true|<fqcn>` switches strategies (three built-in
    fqcns recognized).
22. Instance replication (`Model.replicateElementRelationships`): adding a
    software-system/container instance replicates matching static-model
    relationships between all instances in the SAME environment + deployment
    group; replicated relationships have EMPTY tags (`setTags(null)`) and a
    `linkedRelationshipId`.
23. Auto view keys (V-4 ✅): `generateViewKey(prefix)` = `<Prefix>-<counter:000>`
    scanning from 1 until unused; prefixes `SystemLandscape, SystemContext,
    Container, Component, Dynamic, Deployment, Filtered, Image, Custom`.
    Key uniqueness checked against ALL views; error texts
    `A key must be specified.` / `A view with the key X already exists.`
    View `order` = max(existing orders) + 1 across the whole view set.
24. Features (V-3 ✅): the DSL parser CONSTRUCTOR enables everything
    (INCLUDE, FILE_SYSTEM, HTTP(S), ENVIRONMENT, DOCUMENTATION, DECISIONS, PLUGINS,
    SCRIPTS, COMPONENT_FINDER) — env-var substitution is on by default; the CLI
    (`structurizr export`) does not touch features, so oracle behavior = all on.
    oxidizr keeps the D5 `--allow-net` gate for URL inputs (deviation) but enables
    env substitution + filesystem by default.
25. `!include` mechanics: file or DIRECTORY (recursive, `File` sort order =
    lexicographic path compare, hidden/dot entries skipped, included files read as
    UTF-8 regardless of the main file's charset); each included line is padded
    with the leading whitespace of the `!include` line; included lines are parsed
    as a FRAGMENT in the CURRENT context (context survives) and are NOT appended
    to the retained DSL source; file includes force `dslPortable=false`, URL
    includes do not.
26. `Tokens.get(i)` post-processes every token access: first replace all
    `\"`→`"`, then all `\n`→newline (sequential, on the substituted value).
    `${NAME}` substitution (pattern `\$\{[a-zA-Z0-9-_.]+?}`) runs on the RAW
    token list right after tokenizing; text-block values re-enter substitution
    recursively, plain constants/variables do not; unresolved `${…}` stays
    verbatim; env-var fallback applies when enabled.
27. Continuation/text-block logical lines report the LAST physical line number in
    diagnostics (parse loop flushes with the advanced `lineNumber`) — matches
    `preprocess::Logical::number`.
28. Dynamic view `a -> b` semantics: relationship must already exist in the model
    (`source.getEfferentRelationshipWith(dest, description)` falling back to any
    source→dest relationship; with technology: exact technology match); a reverse
    relationship already IN VIEW is added as `response:true`; otherwise
    `A relationship between A and B does not exist in model.` (with
    `… with technology T …` variant). `add(Relationship, description)` overrides
    the view description.
29. Dynamic-view step quirks (pinned from `DynamicView`/`ModelView` @ `ad34036`):
    - Technology lookups (`getEfferentRelationshipsWith` + loop WITHOUT break)
      resolve to the LAST model-order relationship matching the technology,
      while the description-preferring lookup takes the FIRST match.
    - The reverse (response) lookup is `destination.getEfferentRelationshipWith
      (source)` — the first reverse relationship in model order, IGNORING
      description; it is usable only when THAT relationship is already in the
      view, even when another relationship for the same element pair is in
      view (then the step errors).
    - Re-adding a relationship already in the view: `ModelView.addRelationship`
      builds a new `RelationshipView`, `TreeSet.add` discards it, and the
      setters (description/order/response) land on the DISCARDED object — the
      existing entry keeps its first description/order/response.
    - `DynamicView` scoped to a software system: the scope's own CONTAINERS may
      be added to a fresh view; only the scope itself ("X is already the scope
      of this view and cannot be added to it.") and components
      ("Components can't be added to a dynamic view when the scope is a
      software system.") are rejected before the relationship lookup.
    - `ContainerView` gates the in-scope system FIRST: adding the scoped system
      yields "The software system in scope cannot be added to a container
      view." even when its container is already in the view (the child check
      would also fire).
30. Animations (`StaticView`/`CustomView`/`DeploymentView.addAnimation` @
    `ad34036`): accepted elements = in view AND not in a previous step; the
    ACCEPTED elements join the "previous" set BEFORE the relationship scan, so
    a relationship whose BOTH ends are revealed by the SAME step also joins it
    (reference quirk, not a typo). Step order = `animations.size() + 1`.
    `DeploymentView.addAnimationStep` additionally reveals the deployment-node
    ANCESTOR CHAIN of each accepted element (adding those nodes to the step).

## Deferred (M4 stance)

- `image { }` views: statements parse, but content imports (`plantuml`/`mermaid`/
  `kroki`/`image`, which run exporters/HTTP/file inlines at parse time) are
  rejected with the deferred-import error (OXZ-5003 family) — importers are P2/P4
  territory, same posture as `!docs`/`!adrs` (D7).
- `!script`/`!plugin`/`!components`/`!ref`/`!extend`/`enterprise`: clean
  errors (OXZ-5002 / reference removal messages).
- `workspace extends <url>` requires `--allow-net` (D5); file form lands in M5.
