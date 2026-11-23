# process — how work is done

| note | one-liner |
|---|---|
| [commit-discipline.md](commit-discipline.md) | gate quartet; commit EVERY green sub-piece; never push |
| [refactor-patterns.md](refactor-patterns.md) | P2R zero-behavior-change playbook: phantom-Result cascade, push-order, rng-draw dedupe, bless-as-net |
| [resume-ritual.md](resume-ritual.md) | rewrite `RESUME.md` at each green commit; resume from it |
| [faithful-port.md](faithful-port.md) | engine = port of pinned Java, lint allows need rationale |
| [notes-policy.md](notes-policy.md) | when and how to write notes |
| [parallelism.md](parallelism.md) | one-LLM box: background jobs, no subagents (9 concurrent = server memory thrash; cap 3) |
| [api-churn-v0.4.0.md](api-churn-v0.4.0.md) | P4R staged API-churn list → folded into CHANGELOG.md at E-5 |
| [p4d-strip-rules.md](p4d-strip-rules.md) | P4D comment-strip contract: comments only, facts restated plainly, per-crate verify recipe |
| [p4d-d6-bless-prediction.md](p4d-d6-bless-prediction.md) | what may/must not move when D-6 rewrites human text (e2e bless stays 307×) |

Current session state: [`../RESUME.md`](../RESUME.md) (mutable checkpoint, not a note).
