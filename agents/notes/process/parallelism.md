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
  Worse than overhead: 9 concurrent subagent conversations did not fit the LLM
  server's memory and thrashed it — ~90 % of hours of turns were spent
  re-reading subagent contexts, zero crate work done, one agent even died
  confused. If anything must ever run as subagents, hard cap 3 concurrent;
  default is NEVER — do thinking-requiring work in this session, sequential,
  and parallelize only harness cycles (background shells, non-cargo).
- Two concurrent `cargo` invocations — same target-dir lock, they just queue;
  chain them in one command instead.
- Sleeping/polling on background shells (harness notifies on completion).

Last useful: 2026-11-22
