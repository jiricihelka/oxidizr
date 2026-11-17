# EXPORT-PLANTUML.md — Structurizr-PlantUML micro-grammar catalog

The P2-V2 deferred half (dialect = `export/plantuml/StructurizrPlantUMLExporter`),
cataloged line-by-line from the pinned monorepo (`ad34036`) 2026-11-17 for the
M2 port. Same contract style as `EXPORT-PROPERTIES.md`: every emitted line
format, constant, gate and quirk with its Java evidence — the port mirrors
this file, not memory. The C4-PlantUML dialect (M3) gets its deltas added
here. Engine iteration details are appended as a new section with the M2-b
port (per-view export loop, `writeElements` grouping, `getRelationshipsInView`).

Sources (line counts at the pin): `plantuml/StructurizrPlantUMLExporter.java`
757, `plantuml/AbstractPlantUMLExporter.java` 408, `export/
AbstractDiagramExporter.java` 848, `export/AbstractExporter.java` 118, `export/
IndentingWriter.java` 68, the 8 `plantuml/PlantUML*Style.java` classes.

## §1 Writer mechanics (`export/IndentingWriter`, already ported M0)

* Default: `Spaces`, quantity **2** (not 4); `indent()`/`outdent()` per level.
* `writeLine(content)` = `padding + content.replace("\n", "\\n") + "\n"` —
  REAL newlines inside a formatted string become literal `\n` in the output
  (that is how every `\n` inside labels appears; the format strings write the
  two-char sequence `"\\n"` which is already literal — both routes converge).
* `writeLine()` (no args) = a bare `"\n"` — NO padding on blank lines.
* `replace(before, after)` = first `indexOf` only (unguarded: `before` absent
  → Java throws; the `<style></style>` placeholder is always present).
* `toString()` drops exactly one trailing `"\n"`.

## §2 Entry points and lifecycle (Structurizr dialect)

`writeHeader` (292-381) resets `plantUMLStyles = new HashSet<>()` at the top —
the style set is PER EXPORT call (per diagram, and per animation frame, since
each frame is its own export). A style is added by every boundary/group/
element/relationship writer with the RESOLVED style values; the `HashSet`
dedups by `equals` = same concrete class + same `getClassSelector()`
(`PlantUMLStyle.hashCode` = classSelector hash; `PlantUMLRootStyle.equals` =
same class only). Selectors: `Element-`/`Relationship-`/`DeploymentNode-`/
`Boundary-`/`Group-` + `Base64.getEncoder().encodeToString(name.getBytes())`
(standard base64 WITH padding, UTF-8 bytes; `PlantUMLLegendStyle` selector is
the literal `Element-Transparent`). Names fed to styles:

| style class            | `name` =                       | selector prefix        |
|------------------------|--------------------------------|------------------------|
| Root                   | `.root` (fixed)                | `root` (NO dot in block) |
| Element                | resolved style `tag`           | `Element-`             |
| Relationship           | resolved style `tag`           | `Relationship-`        |
| DeploymentNode         | resolved style `tag`           | `DeploymentNode-`      |
| Boundary               | boundary ELEMENT/box name      | `Boundary-`            |
| Group                  | FULL group (pre-trim)          | `Group-`               |
| Legend                 | `Element-Transparent` (fixed)  | literal                |

Style name = TAG means default-themed elements of the same tag share one
style block — and `equals` dedups them regardless of per-diagram resolution.

## §3 Header grammar (`writeHeader`, exact output order)

1. `@startuml`
2. title leg — gated `includeTitle`: `"true".equals(plantuml.title)`
   (case-SENSITIVE, `AbstractPlantUMLExporter` 177); title text =
   `view.getTitle()` falling back to `view.getName()` when null/empty. With a
   view DESCRIPTION present:
   `title <size:%s>%s</size>\n<size:%s>%s</size>` (ONE line; the literal `\n`
   separates; sizes = fontSize of resolved styles `Diagram:Title` /
   `Diagram:Description`, else constant `DEFAULT_FONT_SIZE` = 24 — a style
   present with NULL fontSize formats as the literal string `null`).
   Without description: `title <size:%s>%s</size>`.
3. blank line, then `set separator none` (base header tail, 223-225).
4. direction leg (Structurizr override, 42-65):
   * sequence-render views (§8): if `"true".equalsIgnoreCase(plantuml.
     boundaries)` (default `true`) → `!pragma teoz true` INSTEAD of direction.
   * `view.automaticLayout != null`: `LeftRight` → `left to right direction`;
     ANY other rank direction → `top to bottom direction`; then
     `skinparam ranksep <rankSeparation / 5>` and
     `skinparam nodesep <nodeSeparation / 10>` — Java INTEGER division
     (`(300/60)`=5, `(300/30)`=10, truncated toward zero).
   * no autolayout → `top to bottom direction` only.
5. `hide stereotype`
6. skinparams (`writeSkinParams`, 227-251): the exporter's own `skinParams`
   map is a `LinkedHashMap` (insertion order; EMPTY when driven by the CLI —
   only the programmatic `addSkinParam` fills it). Block emitted when the map
   OR the comma-separated `plantuml.skinparams` property is non-empty:
   blank line, `skinparam {`, indent, per map entry `<name> <value>`, then
   property entries: `split(",")`, skip null/empty, trim, require contains
   `"="`, `split("=")` → `parts[0] parts[1]` (a third+ segment is DROPPED),
   `}`, outdent. ALWAYS a trailing blank line after the leg.
7. `<style></style>` (the placeholder, replaced in the footer) + blank line.
8. root style added to the set (NOT written here): Light → background
   `#ffffff`/color `#444444`; Dark → `#111111`/`#cccccc`; fontName always
   `null` → `FontName:` line suppressed (StructurizrPlantUMLExporter 69-80).
9. includes (`writeIncludes`, 253-266): `plantuml.includes` non-empty →
   `split(",")`, skip null/empty, trim → `!include <entry>` each, + ONE
   trailing blank line (only when the leg emitted).

Property constants (AbstractPlantUMLExporter 22-30): `plantuml.title`,
`plantuml.includes`, `plantuml.skinparams`, `plantuml.animation`,
`plantuml.sequenceDiagram`, `plantuml.boundaries`; style-level (dialect):
`plantuml.shadow` (StructurizrPlantUMLExporter 15). All read via the 2-level
cascade (§ EXPORT-PROPERTIES §2; here = `export_property`).

## §4 Element grammar, non-sequence (`writeElement`, 310-420)

Resolved style via `findElementStyle(view, element)` (=
`Styles.findElementStyle(element, colorScheme)` — M1-b port); `width`
defaults 450, `height` 300 applied to the RESOLVED style before the
`PlantUMLElementStyle` is created (height never used by this dialect).
`PlantUMLElementStyle` name = style TAG; strokeWidth forced to 2
(`DEFAULT_STROKE_WIDTH`) on sequence renders, else style value else 2;
shadow = style property `plantuml.shadow` equalsIgnoreCase `"true"` (style
properties = the 3rd cascade level: style-level `properties` map).

Line: `<shape> "==<name><type><icon><description>" <<Element-…>> as <id><url>`

* `==` prefix is unconditional (413-414 format `"==%s%s%s%s"`).
* name/description/type/shape/url come from the REFERENCED element for
  `StaticStructureElementInstance`s (instance url falls back to the
  referenced element's url when null/empty); plain elements from themselves.
* `<type>` = `\n<size:M>typeOf(view, element, true)</size>` when type non-empty
  AND `elementStyle.getMetadata() != false`; `M = floor(fontSize × 0.7)`
  (`METADATA_FONT_SIZE_RATIO`, `(int) Math.floor`).
* `<icon>` = `\n\n<img:<iconUrl>{scale=<s>}>` only when `isSupportedIcon`
  (= non-empty && `startsWith("http")` — NOT `https`-only, any `http` prefix);
  scale is the D12 pin `0.5` (rendered by Java `Double.toString` → `0.5`).
* `<description>` = `\n\n<description>` when non-empty AND
  `elementStyle.getDescription() != false`.
* `<url>` = ` [[<url>]]` appended (outside the quotes) when non-empty.
* then `hide <id>` IF `!isVisible(view, element)` — visibility is purely
  ANIMATION-FRAME state (`AbstractDiagramExporter` 712-747): with `frame !=
  null` and step > 0, visible = element added by animation steps of order ≤
  frame (StaticView/DeploymentView) or a relationship endpoint at that frame
  (DynamicView); `frame == null` (every non-animation export) → ALWAYS
  visible — the JSON `ElementView.visible` flag is NOT consulted.

Shape map (`plantUMLShapeOf`, 55-75): Person/Robot → `person`; Component →
`component`; Cylinder → `database`; Folder → `folder`; Ellipse/Circle →
`storage`; Hexagon → `hexagon`; Pipe → `queue`; default `rectangle`.

## §5 Relationship grammar (`writeRelationship`, 422-485)

Resolved via `findRelationshipStyle(view, relationship)`; style name = TAG;
thickness forced 2 on sequence renders. metadata font = floor(×0.7) of the
RELATIONSHIP style fontSize. Description = `<rv.order>: ` prefix when
`relationshipView.getOrder()` has text, + rv.description, else
relationship.description, else `""` (Java `hasValue` = trim-non-empty).
Technology suffix = `\n<size:M>[<technology>]</size>` from the RAW
`relationship.getTechnology()` (NOT the relationshipView override) when
non-empty.

* non-sequence: `<srcId> --> <dstId> <<Relationship-…>> : "<description><tech>"`
  with `arrow = "<--"` when `relationshipView.isResponse() == true` (the
  wrapper is null-checked: null → forward).
* sequence (§8): `<src> -<dst> <<sel>> : <description><tech>` — response
  swaps to `<-` + `-` with NO spaces around; label UNQUOTED.

## §6 Boundary / group grammar (all gated `!renderAsSequenceDiagram` unless noted)

* Software-system boundary (161-189): `rectangle "<name>\n<size:M><typeOf(
  view, system, true)></size>" <<Boundary-base64(name)>> {` + indent; close =
  outdent, `}`, blank. Style name = element NAME; resolved by
  `findBoundaryStyle` (§7); strokeWidth fallback 2; shadow property same as
  elements.
* Container boundary: identical shape, scoped container (200-231).
* Deployment-node boundary (234-280) — NOT sequence-gated:
  `rectangle "<name>[ (x<instances>)]\n<size:M><typeOf(…,true)></size><icon>"
  <<DeploymentNode-base64(style TAG)>> as <idOf>[ [[url]]] {` + indent;
  instances suffix when `instances != "1"`; icon leg from the node's OWN
  resolved style (`isSupportedIcon`); then `hide <idOf(node)>` when frame
  hides it (§4). Close: outdent, `}`, blank.
* Group boundary (113-149): style name = FULL group; the DISPLAY name is
  trimmed to the segment AFTER the LAST `structurizr.groupSeparator` (model
  property — raw `model.properties.get`, no cascade) when set. Line:
  `rectangle "<groupName><icon?>" <<Group-base64(FULL group)>> as group<N> {`
  where `N` = exporter-instance counter `++groupId` starting at 1 (never
  reset across views/frames within one exporter instance — oxidizr must
  mirror the per-exporter scope). Icon leg = `\n\n<img:…{scale=0.5}>` from
  the GROUP style (ungated by `isSupportedIcon` — non-empty icon ONLY here,
  133-135 — contrast §4). Close: outdent, `}`, blank (sequence-gated).
* DynamicView boundary box (sequence path, §8, `startBoundary` 290-303):
  `box "<name>\n<size:M><typeOf></size>" <<Boundary-base64(name)>>` + indent
  (NO `{`); close `endBoundary`: outdent + `end box` (no blank line).

## §7 Boundary/group style resolution (`findBoundaryStyle` 576-654,
`findGroupStyle` 660-723)

Field-wise cascade per field: `Boundary:<SoftwareSystem|Container>` →
`Boundary` → the boundary's ELEMENT style → the per-field default
(background `#ffffff`/`#111111`, stroke+color `#444444`/`#cccccc`,
strokeWidth 2, border `Dotted`, icon `""`, fontSize 24) — string fields take
the first NON-EMPTY, object fields (strokeWidth/border/fontSize) the first
NON-NULL, icon the first `isSupportedIcon`. Groups: `Group:<group>` →
`Group` → same defaults (NO element level). MIRROR the upstream bug at 711:
the group-icon else-if assigns `elementStyleForAllGroups.getColor()` to the
ICON field when the plain-`Group` style's icon is unsupported — a color value
can land in `icon`; byte-parity requires reproducing it (legend icon leg then
emits `<img:#444444{scale=0.5}>`).

`toLineStyle` (729-739, and the identical switch in every PlantUMLStyle ctor):
border `Dotted` → `<sw>-<sw>`, `Dashed` → `<sw*5>-<sw*5>`, else `0`. Border
NULL reaches the switch → Java NPE (unguarded); in practice resolved styles
always carry a border by then (§7 defaults / M1-b theme resolution).

## §8 Sequence-diagram path (`export(DynamicView)`, AbstractPlantUMLExporter 273-360)

Gate: view IS DynamicView && `"true".equalsIgnoreCase(plantuml.
sequenceDiagram)` (2-level cascade). Sequence:
1. header (§3, teoz leg).
2. elements = `LinkedHashSet` of source-then-destination per
   `view.getRelationships()` in relationship order.
3. `plantuml.boundaries` (default true, equalsIgnoreCase): else plain
   `writeElement` per set order. With boundaries:
   * `elementsInWriteOrder`: rootless elements first-seen; else the parent
     chain (root-last reversed) appended, then the element inserted AFTER its
     sibling group (`canonicalName.contains(parentCanonical.substring(
     indexOf("://")))` — the canonical-name form is `<kind>://<name path>`),
     after `parent+1` when no siblings.
   * a stack drives: element → `writeElement` (pop+`endBoundary` while top ≠
     parent; `startBoundary` for non-member parents).
4. blank line when any elements written.
5. `writeRelationships` (all `view.getRelationships()`, §5 sequence leg).
6. footer: `@enduml`, then `writeStyles` replace (§9).
7. `diagram.setLegend(createLegend(view))` — legend after the body (styles
   populated).

Sequence element grammar (`writeElement` sequence leg, 373-381):
`<seqShape> "<name>\n<size:M><typeOf></size>" as <idOf> <<sel>>`. seqShape =
`plantumlSequenceType` (83-100): Box → `participant`, Person → `actor`,
Cylinder → `database`, Folder → `collections`, Ellipse/Circle → `entity`,
default `participant`. For `actor`/`database` the resolved style COLOR is
overwritten with its STROKE before the PlantUMLElementStyle is created (label
sits outside the shape). Sequence-boundary sort comparator (741-756) applies
where the engine sorts elements (§10, M2-b).

## §9 Footer + styles block (`writeFooter` 88-108, style `toString`s)

Footer = `@enduml` then `writeStyles`: all `plantUMLStyles` sorted ONCE by
`getName()` (UTF-16 code units — `Comparator.comparing`), then FILTERED into
seven passes in class order Root → Element → Relationship → DeploymentNode →
Boundary → Group → Legend (each pass re-filters the SAME sorted list),
rendered and `replace`d into the placeholder as `<style>\n` + blocks +
`</style>`. Blocks are `toString()`s built on FRESH `IndentingWriter`s (base
padding = 2 spaces, fields 4):

```
  // <name>                              ← absent for Root and Legend
  .<selector> {                         ← Root: "root {" (no dot)
    <field lines, exact per-class order below>
  }
```

* Root: `BackgroundColor: b; FontColor: c; [FontName: f;]`
* Element: `BackgroundColor; LineColor; LineStyle; LineThickness;
  [RoundCorner: 20; when shape == RoundedBox] FontColor; FontSize;
  HorizontalAlignment: center; Shadowing: <10|0>; MaximumWidth: <width>;`
* Relationship: `LineThickness; LineStyle; LineColor; FontColor; FontSize;
  MaximumWidth;` (Line/Font color = the single style color)
* DeploymentNode: `BackgroundColor; LineColor; LineStyle; LineThickness;
  FontColor; FontSize; HorizontalAlignment: center; Shadowing;
  [MaximumWidth — only when set, legend only]`
* Boundary/Group: same as DeploymentNode MINUS MaximumWidth/icon.
* Legend: comment `// transparent element for relationships in legend`,
  `BackgroundColor: transparent; LineColor: transparent; FontColor:
  transparent;`
* `SHADOW_DISTANCE` = 10 → `Shadowing: 10;` when `plantuml.shadow` true.

## §10 Legend diagram (`createLegend`, 487-574) — P2-V6 Structurizr half

Emitted for EVERY view of this dialect (base `createLegend` returns null; the
override always returns a Legend; `Diagram.setLegend` in every per-view
export + the sequence path). Legend content (fresh writer, its OWN
`<style></style>` + own `writeStyles` with the same style set):

1. `@startuml`, blank, `set separator none`, `hide stereotype`, blank.
2. `<style></style>`, blank.
3. DeploymentNode styles (set sorted by name again): `setWidth(200)`;
   description = name with a leading `"Element,"` stripped, then
   `replaceAll(",", ", ")`; icon leg via `isSupportedIcon`; line
   `rectangle "==<description><icon?>" <<DeploymentNode-…>>` + blank.
4. Element styles (sorted): same description/icon legs, `setWidth(200)`,
   line `<<shape keyword(style.shape)>> "==<description><icon?>" <<sel>>`
   + blank (shape-only overload `plantUMLShapeOf(style.getShape())`).
5. Relationship styles (sorted, `id` counter restarts at 1): description =
   name minus leading `"Relationship,"`, commas `", "`; TWO lines per style:
   `rectangle "." <<.Element-Transparent>> as <id>` then
   `<id> --> <id> <<Relationship-…>> : "<description>"` (self-loop carries
   the style; label QUOTED) + blank.
6. `@enduml`.
7. `plantUMLStyles.add(new PlantUMLLegendStyle())` AFTER writing the body,
   then `writeStyles` — so the legend's style block includes the transparent
   style (and the diagram it came from, if re-rendered, would too — but the
   diagram was already serialized).

## §11 idOf / filter (AbstractPlantUMLExporter 89-170)

`idOf(modelItem)`: parent-chain `filter(name)` joined by `"."`; top-level
`DeploymentNode` prefixed `filter(environment) + "."`; non-element items →
raw id; instances append `"_" + instanceId`. `filter` = Java
`replaceAll("(?U)\\W", "")` (Unicode word-char removal — oxidizr: `char::
is_alphanumeric` + `_` under `--` no; mirror `[\p{L}\p{N}_]` keep-set,
matching `(?U)\W`). Collisions (two systems "A B" / "AB") are NOT resolved —
mirror.

## §12 P2-V5 (Structurizr dialect color constants) — VERIFIED 2026-11-17

`Styles.DEFAULT_BACKGROUND_LIGHT = "#ffffff"`, `DEFAULT_COLOR_LIGHT =
"#444444"`, `DEFAULT_BACKGROUND_DARK = "#111111"`, `DEFAULT_COLOR_DARK =
"#cccccc"` (`structurizr-core/view/Styles` 11-15). Root style + the
boundary/group defaults use these per `colorScheme` (constructor default
Light; the CLI passes light/dark per the format suffix — registry already
ports the suffix in `format.rs`). Mermaid/C4 legs land with M3/M4.

## §13 Java formatting quirks to mirror verbatim

* `%s` of a NULL `Integer` (fontSize etc. reaching a format string) prints
  `null` — only reachable when a theme style sets a field to null where the
  exporter assumes non-null (title/description styles: title LEGAL; boundary/
  group styles resolve to defaults first — not reachable there).
* `Double.toString(scale)` = shortest round-trip (`0.5` here, D12 pin).
* Integer division for ranksep/nodesep (§3.4) TRUNCATES (Java `/`),
  e.g. rankSeparation 22 → `4`.
* Base64 = RFC 4648 standard alphabet WITH `=` padding, UTF-8 bytes.
* `(x<instances>)`: string compare `!"1".equals(instances)` — `"01"` would
  suffix too.

## §14 Engine iteration (`export/AbstractDiagramExporter` 848 +
`AbstractExporter` 118) — appended with the M2-b port, 2026-11-17

**Order contracts** (every iteration leg in the engine):

* `view.getElements()` = Java `TreeSet<ElementView>`: **numeric** element-id
  ascending (`Integer.parseInt` subtraction), string fallback when either id
  is non-numeric (`ElementView` 104-111). Our `ViewContent.elements` is
  insertion order → the engine must sort on read.
* `view.getRelationships()` = `TreeSet<RelationshipView>`: `id + "/" + order`
  string compare — a null `order` joins as the literal `"null"`
  (`RelationshipView` 312-316).
* `getXxx` lists (custom/people/systems/boundary-containers/containers) and
  `getBoundarySoftwareSystems`: `Comparator.comparing(Element::getId)` =
  **id STRING** order (NOT numeric) after `distinct()` (first occurrence kept);
  `findGroups` sorts the element list **in place** by id string (the ungrouped
  pass below sees the sorted list).
* Deployment-node children / infra / instances: sorted by NAME (UTF-16) per
  kind, concatenated children → infra → systemInstances → containerInstances,
  each filtered `view.isElementInView`.
* Group display order: `Collections.sort` natural String (UTF-16).

**Per-view bodies** (`export(view)` = `export(view, null)`; then animation
frames if `isAnimationSupported` and — static/deployment/custom — animations
non-empty, one frame export per animation in list order; DynamicView instead
iterates `LinkedHashSet` of relationship-view ORDERS (TreeSet traversal,
deduped); `diagram.setLegend(createLegend(view))` LAST — for the Structurizr
dialect the legend's style set is whatever the LAST body export (last frame,
or the main diagram) left — mirror).

* custom/landscape/context: header → `writeElements(getGroupableElements
  (view, null))` → blank line IF elements list non-empty → relationships →
  footer.
* container: (1) non-container view elements flat in view order + blank iff
  written; (2) per boundary software system (owner of each container in view,
  id-string sorted): `startSoftwareSystemBoundary` → writeElements(the view's
  containers with parent == system) → `end…`; (3) relationships; footer. No
  blank line between legs 1/2 beyond the legs' own.
* component: custom-, people-, systems-legs each + blank iff non-empty; then
  per boundary software system (LinkedHashSet over the id-sorted boundary
  containers): `startSS` → per boundary container of that system
  `startContainerBoundary` → writeElements(its components) → `endContainer` →
  then ALL view containers of that system written flat → `endSS`;
  relationships; footer.
* dynamic (scope from `element_id`): no scope → writeElements(all elements);
  system scope → boundary legs (as container-view leg 2) then the
  parent-less elements flat; container scope → the full component-view
  sequence then the elements whose parent is NOT a container flat. The
  "blank line" leg is governed by the LAST leg's `elementsWritten` (no-scope:
  the elements list).
* deployment: header → writeElements(TOP-LEVEL deployment nodes in view) →
  relationships → footer — NO blank-line leg between (506-510).
* `write(deploymentNode)` recursion: boundary → children/infra/instances
  sorted-by-name → `writeElements` (which sorts THAT combined list in place
  by id string; groups honored per node) → end boundary.

**`writeElements`** (`findGroups` first): grouped pass — flat mode =
per group `startGroupBoundary`/members/`endGroupBoundary`; **nested mode**
(model property `structurizr.groupSeparator` non-empty): per sorted group
compare `group.split(sep).length` vs `context.split(sep).length` (Java split:
trailing empties dropped; `"".split` → length 1): deeper → `indent()`; equal
→ `endGroupBoundary` **only when `context.length() > 1`** (a ONE-CHARACTER
context never closes — upstream quirk, mirror byte-exactly); shallower →
`contextCount-groupCount` × (end + outdent) **plus one MORE end** without
outdent; every pass starts its own group; tail closes `contextCount` groups
outdenting all but the last. Ungrouped pass afterwards in the (id-sorted)
list order.

**`getRelationshipsInView`**: DynamicView → view order (TreeSet); else sorted
by relationship-id STRING. `writeRelationships` writes ALL of them (the
`isVisible(view, rv)` helper is never consulted here — frames show every
relationship) + trailing blank iff non-empty.

**`getGroupableElements(view, parent)`** matrix: custom→all (cast, abort on
non-custom); landscape/context→all (`GroupableElement` — ClassCastException =
reference abort on deployment-y elements); container→containers only;
component→components only; dynamic→ by scope: none→all, system→containers,
container→components; deployment→deployment nodes with `parent == null`;
`parent != null` filters `parent ==` reference-equality (Id equality in Rust).

**Animation `isVisible(view, element)`** (frame state): static/deployment
with step > 0 → element in union of animation steps ≤ step; dynamic → frame
order equals an rv order (endpoint of it); `frame == null` or step ≤ 0 →
true. (`ElementView.visible` JSON flag: never consulted.)

**Terminology** (`AbstractExporter.typeOf` 48-105): per-kind
`Terminology.findTerminology` (instances resolve to their element; defaults
`Person`/`Software System`/`Container`/`Component`/`Deployment Node`/
`Infrastructure Node`; custom → `metadata` else `"Element"`; trim-check), +
`: <technology>` suffix (trim-check `hasValue`) for container/component/
deploymentNode/infrastructureNode ONLY; custom-element type = `metadata`
directly (never via terminology); empty → returned bare (no symbols);
else wrapped per `configuration.metadataSymbols` (NULL → mutated to
SquareBrackets): `[…]`/`(…)`/`{…}`/`<…>`/`<<…>>`/none. `breakText`:
`maxCharacters = (int)(maxWidth/(fontSize×0.6))`, `s.length() < maxCharacters`
→ unchanged; word-wrap with `"<br />"` joins; empty/null → `""`.
`StringUtils.isNullOrEmpty` = null || TRIM empty (every gate in the engine).
