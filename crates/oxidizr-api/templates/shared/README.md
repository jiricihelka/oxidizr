# My Workspace

A models-as-code architecture workspace, created by `oxidizr init`.

- `workspace.dsl` — the model (people, systems, containers) and the views.

## Commands

```sh
oxidizr validate workspace.dsl            # parse + core validation
oxidizr export workspace.dsl -f plantuml  # emit PlantUML for every view
oxidizr query workspace.dsl --filter 'element.tag==Software System'
oxidizr view workspace.dsl                # interactive viewer (gui builds)
```

## Language server

`oxidizr-lsp` speaks LSP over stdio — diagnostics, completion, hover and
formatting for `*.dsl`. Point your editor's LSP client at the binary (the
`.vscode/` settings carry a note).
