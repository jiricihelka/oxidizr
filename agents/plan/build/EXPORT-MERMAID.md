# EXPORT-MERMAID.md — Mermaid exporter catalog (M4)

THE contract for the Mermaid port (mirror this file, not memory). Sources at
the pin (agents/plan/COMPAT.md, `ad34036`):
`structurizr-export/src/main/java/com/structurizr/export/mermaid/MermaidDiagramExporter.java`
(402 lines, `MMD` below), `mermaid/MermaidDiagram.java` (extension `mmd`),
and the SHARED base `export/AbstractDiagramExporter.java` (`ADE`) +
`AbstractExporter.java` (`AE`). The M2-b engine (`plantuml/engine.rs`) already
ports the base loops (§10) — reuse, don't move.

## §1 Property register (`getViewOrViewSetProperty` cascade: view → views.configuration → default)

| property | site | default | match | effect |
|---|---|---|---|---|
| `mermaid.title` | `MMD:392` | `true` | case-SENSITIVE `equals("true")` | header subgraph title (§2) |
| `mermaid.sequenceDiagram` | `MMD:396` | `false` | `equalsIgnoreCase("true")` | dynamic views render as `sequenceDiagram` (§9) |
| `mermaid.icons` | `MMD:292` | `false` | case-SENSITIVE `equals("true")` | element icon `<img>` (§6) |

No direction/skinparams/includes/animation/boundaries properties exist for
mermaid — direction comes from `automaticLayout` only (§2), and the base
`isAnimationSupported` is `false` (`ADE` default, NOT overridden) → mermaid
NEVER emits animation frames. `model.groupSeparator` is read directly from
MODEL properties (§4), not the view cascade.

## §2 Header / footer (all non-sequence views; `MMD:34-79`)

1. `groupId = 0` reset (per-diagram, incl. frames).
2. direction from `view.automaticLayout.rankDirection`: TopBottom→`TB`,
   BottomTop→`BT`, LeftRight→`LR`, RightLeft→`RL`; no automaticLayout → `TB`.
3. `graph <D>`; indent; `linkStyle default fill:#ffffff`; blank line.
4. title: `" "` if `mermaid.title` ≠ true, else `view.title`, blank →
   `view.name` (`getName()` = view key/name fallback per core model).
5. `subgraph diagram ["<title>"]`; indent; `style diagram
   fill:#ffffff,stroke:#ffffff`; blank line. (TWO open indents.)
6. footer: outdent, `end`, outdent — ONE `end` closes both opens; no blank.

File naming: `structurizr-<workspaceId>-<key>.mmd` via the shared ExportCommand
loop; extension `mmd` (`MermaidDiagram.getFileExtension`); `createLegend` =
null (`ADE:760`) → NO legend files; frames never (§1).

## §3 Element write dispatch (`ADE:679-685`)

`write(view, element)`: DeploymentView + DeploymentNode → DN boundary recurse
(`ADE:687+`, children/infraNodes/SSI/CI each sorted by NAME, stable — engine
`write_deployment_node`); else `writeElement` hook.

## §4 Group boundaries (`MMD:82-117`)

- counter `group<N>` (N from 1); line `subgraph group<N> ["<name>"]`; indent;
  `style group<N> fill:#ffffff,stroke:<c>,color:<c>,stroke-dasharray:5`; blank.
- name: after the LAST `model.groupSeparator` occurrence (substring
  `lastIndexOf(sep)+sep.length()`); no/empty separator property → full group.
- color `<c>`: `ElementStyle("Group:"+group).color` → else `ElementStyle("Group").color`
  → else `#cccccc` (each candidate needs non-empty `color`).
- close: outdent, `end`, blank line.
- nested-group walk = base `writeElements` (`ADE:543+`, `split(Pattern.quote(sep))`
  depth walk) — engine `write_elements` (already byte-pinned via M2/M3).

## §5 Element boundaries

- SoftwareSystem (`MMD:120-128`): `subgraph <id> ["<name>"]` (id = element id,
  hierarchical identifiers apply); `style <id> fill:#ffffff,stroke:<s>,color:<s>`
  with `<s>` = resolved style STROKE for both.
- Container (`MMD:138-146`): identical shape of output, style of the container.
- DeploymentNode (`MMD:156-163`): same lines but `stroke:<style.stroke>,color:<style.color>`.
- closes: outdent, `end`, blank.

## §6 Element (`MMD:254-316`, non-sequence)

Order of facts:
1. style = `findElementStyle(element)` — resolved on the ORIGINAL element
   (instances keep their own tags for style), while name/description/type
   unwrap from the instance (`MMD:262-267`).
2. node symbols by RESOLVED shape: `RoundedBox`→`( … )`, `Cylinder`→`[( … )]`,
   anything else (incl. Person/Pipe!) → `[ … ]`. No D12 icon-scale here.
3. label = `"<div style='font-weight: bold'>{name}</div>{type}{desc}{icon}"`
   — name RAW (no wrapping).
   - desc (gated: non-empty AND `style.description != false`):
     `<div style='font-size: 80%; margin-top:10px'>{lines(desc)}</div>`
   - type (gated: `style.metadata != false`, else empty): `typeOf(view, element,
     true)` wrapped into `<div style='font-size: 70%; margin-top: 0px'>{type}</div>`
     — `typeOf` = AE:56-… terminology (+`: technology` for Container/Component/
     DN/IN; CustomElement → metadata string), empty → empty (NO brackets);
     metadata symbols per `views.metadataSymbols` (null → SquareBrackets,
     `[X]`, `RoundBrackets (X)`, `CurlyBrackets {X}`, `AngleBrackets <X>`,
     `DoubleAngleBrackets <<X>>`, `None X`).
   - icon (gated: `mermaid.icons` equals-true AND style.icon non-empty AND
     starts with `http`): `<div><img src='{icon}' style='max-height: 50px;
     margin: auto; margin-top:10px'/></div>`
4. line: `{id}{open}"{label}"{close}`.
5. `click {id} {url} "{url}"` iff element url non-empty.
6. `style {id} fill:{bg},stroke:{stroke},color:{color}` — both Java branches
   (instance / not) are IDENTICAL (`MMD:310-315`, dead quirk).

`lines(text)` (`MMD:360-384`): trim, split on whitespace runs, greedy lines
while `len(line)+1+len(word) < 30`, else flush + `<br />` separator; empty/
null → empty string.

## §7 Relationship (`MMD:319-358`, non-sequence)

1. SKIP entirely if source OR destination `instanceof DeploymentNode`
   (stronger than the class README: any DN endpoint kills the edge).
2. response (`relationshipView.isResponse == true`) swaps source/destination.
3. solid ⇔ `style.style == Solid || style.dashed == false`; arrow parts:
   solid `A-- "…" -->B`, dotted `A-. "…" .->B`.
4. description = RV description, fallback relationship description; RV order
   non-empty → prefix `{order}. ` BEFORE wrapping.
5. label = `"<div>{lines(desc)}</div><div style='font-size: 70%'>{[tech]}</div>"`
   (`[tech]` only when technology non-empty; tech NOT `lines()`-wrapped).

## §8 Dynamic (non-sequence) = base body leg (`ADE:380+`) — engine `body`
`View::Dynamic` leg already ports it byte-faithfully (scoped legs, boundary
stacks via §5 hooks, flat rootless elements, trailing blank iff elementsWritten).

## §9 Dynamic sequence form (`MMD:173-246`, `mermaid.sequenceDiagram`=true)

NOT header/footer, NOT `writeRelationship` — a standalone document:
1. `sequenceDiagram`; blank; indent (never outdented again).
2. participants: LinkedHashSet over relationshipViews (source then destination
   per RV, first-seen wins — engine `view_relationships` order); per element:
   `actor` iff RESOLVED shape == Person else `participant`;
   `type = typeOf(view, element, true)`; empty type OR `style.metadata == false`
   → type part empty, else prefix `<br />`;
   line `{participant|actor} {id} as {name}{type}`.
3. blank line (ALWAYS written after participants, even when none).
4. messages per RV: ids swap when response; arrow `->>` else `-->>` for
   response; `technology = "<br />[" + tech + "]"` iff non-empty;
   line `{src}{arrow}{dst}: {esc(desc)}{esc(tech)}` where `esc` replaces every
   `;` with `#59;`. desc = RV description else relationship description;
   null/null → Java `escapeSemicolon(null)` = NPE (ReferenceAbort parity leg
   in oxidizr — mirror the engine's existing abort pattern, do NOT invent text).
5. No footer, no frames, no legend, no `graph`/subgraph wrapper.

## §10 Engine reuse map (delta work for the port)

`Dialect` + `Engine` reuse: view iteration order (custom→landscape→context→
container→component→dynamic→deployment, `ADE:39-90`), `body` legs,
`write_elements`, `write_deployment_node`, `find_groups` (elements sorted by
id, Java string order), `get_relationships_in_view` (sorted by relationship id
string EXCEPT dynamic views), CustomView export (engine
`exports_custom_views() → true`), `create_legend → None`, frames never
(`is_animation_supported` stays the trait default `false`).

Deltas to implement (M4-b):
- NEW trait route for the sequence form: the engine's
  `export_dynamic_sequence` is the ABSTRACT-PLANTUML walk (`plantuml.
  boundaries` + box stack) — Mermaid's §9 is a different Java method. Add
  `Dialect::sequence_document(&mut self, e, view, dv) -> Result<Diagram>`
  consulted when `render_as_sequence_diagram` returns true and the dialect
  owns its sequence syntax (default impl = the current engine walk stays).
- `.mmd` file extension per diagram (naming: the ExportCommand write loop uses
  `Diagram.getFileExtension`; check `format.rs::workspace_export_files`
  threads it — M2/M3 hardcode `puml`).
- `export_property` reads per §1 (case sensitivity differs per property!).
- style resolution already exists (M2 engine helpers `resolve_element_style`
  /terminology); stroke/background defaults `#444444`/`#ffffff` come from the
  resolved default theme constants, exactly as in M2/M3 witnesses.

## §11 Witness sweep (oracle 2026.10.26, `--format mermaid`)

Sweep: `/tmp/nix-shell.uni100/opencode/m4/sweep.sh` → witness dirs
`/tmp/nix-shell.uni100/opencode/m2d/sweep/<fixture>/ref-mermaid/` +
`ref-mermaid.status` (2026-11-17). 44-fixture corpus: 33 produced ≥1 `.mmd`;
10 failed oracle LOAD (the same set as the C4 sweep); `text-block` loaded but
wrote ZERO files (oracle exit 0 — mermaid-vacuous, ungoldenable).

Golden corpus (planned M4-d): the 33 witnesses MINUS the D2 loader rejects
(`script-inline`, `script-in-dynamic-view`) — `bulk-operations` is IN until a
byte DIFF says otherwise (Mermaid never iterates properties, so the D13
identity-hash order may not bite: `findGroups` sorts elements by id).
The `custom-view-animation` witness (2 files: `Custom-001`, `Custom-002` =
TWO views, not frames) proves §1 "never emits frames".
