# DSL grammar gotchas paid for during the e2e corpus (T2)

All empirically decided by the pinned engine (blessed reality) — fixtures must
match these, the engine never bends.

- Trailing `//`/`#` comments on a statement line poison the tokens
  (`"S" // x` = "too many tokens"); comments must occupy their own line.
- Identifiers: `a-zA-Z0-9_-` only (unicode names fine in QUOTED strings).
  `systemLandscape` view key same charset. No `${}` inside identifiers.
- Positional maxes: `softwareSystem <name> [description] [tags]` (3 strings);
  `group <name> {` (no description); `filtered <key> <include|exclude> <tags..>
  [key] [description]` is NOT a block.
- Statements valid only in specific contexts: relationship block =
  tags/url/properties/perspectives (technology/description go positional);
  deploymentNode 5th token = `instances` (int/range — breaks the JSON
  round-trip if misused); element `name`/`metadata` statements don't exist;
  workspace `metadata` doesn't exist.
- `routing` = Direct|Curved|Orthogonal; `iconPosition` = Top|Bottom|Left;
  `jump` true|false; `shape`/`border`/`style` enums per `stmt_style.rs` tables;
  `fill` is not a token. `enterprise` keyword removed (use `group`).
- `!identifiers hierarchical`: container/component/dynamic view scopes and
  cross-context references need the DOTTED id (`core.api`, `strangler.kafka`).
  Nested-group animation steps reference by container(-instance) id.
- Nested groups need `structurizr.groupSeparator` as a MODEL property (not
  workspace). `url` in `group` rejected by the engine (vendored
  `group-url.dsl` disagrees — Phase-1 compat flag, see DEVIATIONS candidates).
- `workspace extends`: child reuses the parent's env via
  `!element <envId> { … }`; re-declaring `deploymentEnvironment` by name fails.
  `deployment <sys> "<env>" [key] [desc]`.
- Style statements in an `!include`d file must be wrapped in `styles { … }`
  when included at views level; inside `styles { }` they go bare.
