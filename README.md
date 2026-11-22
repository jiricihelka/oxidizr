# oxidizr

A Rust toolchain for [Structurizr](https://structurizr.com) workspaces — the
"models-as-code" architecture tool for the [C4 model](https://c4model.com).

The core promise: your workspaces stay Structurizr-compatible. oxidizr reads
and writes both the DSL (`.dsl`) and JSON (`.json`) formats, byte-comparable
where the format defines it, so files round-trip between oxidizr and the
official tooling unchanged.

## Install

**Prebuilt binaries** — download the latest release for your platform from
the [releases page](https://github.com/jiricihelka/oxidizr/releases) and put
`oxidizr` on your `PATH`.

**From source** (Rust; the pinned nightly in `rust-toolchain.toml` is picked
up automatically by rustup):

```sh
cargo build --release -p oxidizr-cli                 # → target/release/oxidizr
cargo build --release -p oxidizr-cli --features gui  # ...with the native viewer window
```

A Nix dev shell (`nix develop`) additionally provides the oracle binary and
cross-compilation toolchains used by the test suite.

## Quickstart

```sh
oxidizr validate workspace.dsl                # check it (format detected by extension)
oxidizr dsl2json workspace.dsl -o workspace.json
oxidizr export -w workspace.dsl -f plantuml/c4plantuml -o out/
oxidizr diff before.dsl after.dsl             # exit 1 when they differ
```

## Commands

Run `oxidizr --help` (or `oxidizr <command> --help`) for the full grammar.

| Command | What it does |
|---|---|
| `dsl2json` | Parse a DSL workspace and write standard JSON (`-` = stdin/stdout) |
| `json2json` | Re-serialize JSON (normalization; `--keep-unknown` keeps non-schema keys) |
| `validate` | Validate workspace file(s); format auto-detected by extension |
| `fmt` | Reformat DSL source to canonical style (`--write`, or `--check` as a CI gate) |
| `reconcile` | Re-attach layout-JSON geometry onto an edited DSL (id drift survives) |
| `export` | Export the workspace: `theme`/`json` plus diagram formats `plantuml`/`c4plantuml`/`mermaid`/`websequencediagrams` |
| `diff` | Compare two workspaces (JSON twins of one model diff empty); exit 1 when they differ |
| `inspect` | Run advisory model/view inspections; exit 1 when anything is reported |
| `import` | Import a C4-PlantUML, Mermaid or image diagram into a fresh workspace |
| `query` | Query a workspace with the DSL filter grammar (`element.tag==Web`, `user->system`, …) |
| `init` | Scaffold a starter workspace project (`minimal` or `c4` template) |
| `generate system-landscape` | Derive the system landscape of one or more workspaces |
| `completions` | Print a shell-completion script (`bash`, `zsh`, `fish`, …) |
| `oracle` | Generate/verify reference JSON goldens for fixture authors (spawns the official `structurizr` CLI) |
| `view` | Open the workspace in the native viewer window (needs the `gui` build feature) |

Global flags: `--strict` (warnings become errors), `--quiet`, `--no-color`
(also via `NO_COLOR`), `--allow-net` (permit theme-URL fetches during
`export`; offline by default), `--themes-dir <dir>` (installed-theme root).

## Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | error diagnostics (any warning under `--strict`), or differences found (`diff` / `inspect`) |
| `2` | usage error (unknown command/argument) |
| `3` | I/O failure |

Every diagnostic carries an `OXZ-…` code — [docs/diagnostics.md](docs/diagnostics.md).

## Documentation

- [docs/](docs/README.md) — machine-readable report schemas, the LSP setup, diagnostics
- `oxidizr view` and the standalone viewer binary: [crates/oxidizr-viewer/README.md](crates/oxidizr-viewer/README.md)
- Working on oxidizr? [AGENTS.md](AGENTS.md) and the plan corpus under `agents/plan/`

## Compatibility notes

JSON is the contract: field names, shapes and types follow the Structurizr
format wherever it defines them. Workspaces written by oxidizr open in the
official tooling, and official `.dsl`/`.json` files load here — the test
corpus under `testdata/` (see its README) is vendored byte-exact from the
official fixtures.

## License

Dual-licensed under either of Apache-2.0 (`LICENSE-APACHE`) or MIT
(`LICENSE-MIT`) at your option. The vendored fixtures in `testdata/` carry
their own attribution (see `testdata/README.md`).
