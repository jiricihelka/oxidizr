# Editor setup notes (`oxidizr-lsp`)

Smoke notes for driving `oxidizr-lsp` from a real editor. The
server speaks plain LSP over stdio with no client requirements; the
snippets below are the shapes we smoke-tested by hand, not a support
matrix. Range semantics are UTF-16 code units (0-based), as the spec
defines — the same mapper the tests pin.

Run the binary: `oxidizr-lsp` (stdio), `oxidizr-lsp --version` / `-V`
prints and exits. Nothing else on argv; unknown argv is ignored.

## What the server offers

| Capability | Behavior |
|---|---|
| Sync | `textDocumentSync = Full` — every `didChange` carries the whole buffer (files are small; incremental stays deferred). |
| Diagnostics | Pushed on `didOpen`/`didChange`, cleared on `didClose` (EMPTY publish). The exact CLI pipeline (`oxidizr validate` squiggle-for-squiggle); `.json` buffers validate through the JSON read leg. |
| Completion | Scope-accurate keywords + directives + contextual identifiers + snippets; trigger character `!`. |
| Hover | Model cards (kind, canonical name, technology, tags, relationship counts) from the LAST SUCCESSFUL parse — mid-typing hovers survive a broken buffer; plus keyword/directive cards. |
| Formatting | `formatting` prints the canonical style (4-space indent, conservative unquoting) as ONE whole-buffer edit; `rangeFormatting` widens to whole top-level statements (the canonical print is global — a mid-statement edit would disagree with the next full format). `.json` buffers are NEVER formatted — the byte contract. |
| Shutdown | `shutdown` then `exit`; clean exit ⇒ status 0. |

Not implemented (answering `MethodNotFound`):
semantic tokens, rename, code actions, incremental sync, watching.

## `initializationOptions`

All fields optional; a malformed blob (or none at all) answers the
defaults — configuration never fails `initialize`.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `strict` | bool | `false` | `--strict` for every pipeline run (warnings block). |
| `themesDir` | string | unset | `--themes-dir`: highest-priority installed-theme source. |
| `allowNet` | bool | `false` | `--allow-net`: lifts the pipeline's theme-URL gate. The server NEVER introduces network access of its own. |
| `maxNumberOfProblems` | number | `100` | Cap for each `publishDiagnostics` (`0` is honored literally). |

```json
{
  "initializationOptions": {
    "strict": false,
    "themesDir": "~/.structurizr/themes",
    "allowNet": false,
    "maxNumberOfProblems": 50
  }
}
```

## Helix

`~/.config/helix/languages.toml`:

```toml
[language-server.oxidizr]
command = "oxidizr-lsp"

[[language]]
name = "json"   # .json workspaces: diagnostics only, formatting refused
```

Helix needs a grammar for `.dsl` files to label the language; point
`file-types` at `["dsl"]` with any (even empty) grammar — the server
keys the format route off the URI extension, not the client's
`languageId`.

## Neovim

```lua
vim.lsp.config.oxidizr = {
  cmd = { "oxidizr-lsp" },
  filetypes = { "dsl", "json" },
  init_options = { maxNumberOfProblems = 50 },
}
vim.lsp.enable("oxidizr")
```

(and `vim.filetype.add({ extension = { dsl = "dsl" } })` so `.dsl`
buffers carry a filetype).

## VS Code

The lightest ride is the `vscode-languageserver-client` quickstart (no
extension scaffold needed for a smoke): point a tiny `package.json`
contribution at the executable and set `initializationOptions` in
settings — or use any existing "generic LSP client" extension with the
same JSON block. The server needs nothing beyond stdio.

## Smoke checklist (any editor)

1. Open a `.dsl` with a broken statement — a squiggle appears with code
   `OXZ-nnnn`; hover it (hover shows the registry link via the code
   description).
2. Type `model {` and request completion after `!` — the directive menu
   appears.
3. Format (whole document) — indent snaps to 4-space canon; format again
   — no edit (idempotent).
4. Break the buffer mid-file — hover over an element name still shows
   its model card (the last good parse).
5. Open a `.json` workspace — diagnostics work, formatting changes
   nothing (empty edit array).
