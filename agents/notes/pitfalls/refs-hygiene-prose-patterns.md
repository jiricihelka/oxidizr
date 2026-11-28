# refs hygiene: prose patterns that fail the `refs` leg

The `refs` gate's hygiene leg scans shipped-crate text (rustdoc + comments)
for process ids and provenance narration. Three traps found while adding a
keybinding table (P6-M0, 2026-11-28):

1. **Bare `F1`..`F19` in comments/rustdoc** = "feature tag" violation —
   even when you mean the FUNCTION KEY. F1 is a key name, not a registry id,
   but the checker cannot tell. Move it into a string literal (UI text is
   exempt): `keys: "F1 or ?"` passes; `/// F1 opens this` fails.
2. **The phrase "the reference"** (word boundary after) =
   reference-implementation narration — anywhere, including test assert
   messages. Name things differently (`binding table`, `keymap`) and keep
   prose away from that collocation.
3. **`agents/`, `phase_`, `.java`, `ported from`** are narration patterns
   too — fine in `agents/` (excluded), never in shipped prose.

Also: `too_many_lines` fires on big const-literal FUNCTIONS — keep large
data in a `const` ITEM and return it. And `-D warnings` runs differ per
feature set: a plain `cargo clippy --workspace` can pass while the
`ci gui` clippy leg (`--features oxidizr-viewer/gui,...`) is red — the gui
wave's lint gate is `cargo xtask ci gui fmt clippy test refs`, period.
