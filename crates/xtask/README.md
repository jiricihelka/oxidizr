# xtask

Dev-only tooling, run as `cargo xtask <job>`: the commit-gate quartet
(`ci`), golden generation/verification against the reference oracle
(`oracle`), the e2e sidecar bless/mutate jobs (`e2e`), benchmarks and
baseline comparisons (`bench`), fuzz and coverage mirrors, fixture
vendoring (`vendor`), and the doc-reference integrity check (`refs`).

The job table and rules: `agents/plan/WORKFLOW.md`.
