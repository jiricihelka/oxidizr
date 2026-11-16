# parallelism — spend LLM turns, hoard harness cycles

Harness machine: powerful, mostly single-tenant. LLM server: **one agent at a
time**. So wall-clock = LLM turn count, and harness jobs are free background
fuel. (Adopted 2026-11-15; permanent rule: `agents/plan/WORKFLOW.md`
§Execution model.)

## Do

- Start every long independent harness job as a background shell immediately
  (nix build/dev-shell eval, `oracle verify`, bench/cov, ast-grep sweeps,
  find+jq analyses); do other useful work meanwhile; never poll its output file.
- Batch independent checks into ONE command (`fmt && clippy && test && refs`)
  — one LLM turn instead of four.
- Prefer tool answers over reading: `cargo xtask refs` / `lint` / ast-grep
  patterns instead of eyeballing files (each tool run costs 0 turns to think).
- Multi-file extraction/verification (golden diffs, manifest counts) → one
  python3/jq script, report only its conclusion.

## Don't

- Subagents — serialise on the single LLM slot, pure overhead (2026-11-15).
- Two concurrent `cargo` invocations — same target-dir lock, they just queue;
  chain them in one command instead.
- Sleeping/polling on background shells (harness notifies on completion).

Last useful: never
