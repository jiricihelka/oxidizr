# RESUME.md checkpoint ritual (adopted 2026-11-15, after M4)

Why: M4 was one ~20 h uncommitted chunk; every context compaction cost re-reads
of plan/charter files and re-derivation of already-settled decisions.

Ritual:
- Session start (and immediately after a compaction): read `agents/notes/RESUME.md`
  FIRST, then only the indexed topics it points at.
- At EVERY green commit: rewrite `agents/notes/RESUME.md` (≤ 20 lines) — HEAD,
  phase/milestone state, the next concrete step, open questions, and
  "don't re-derive" pointers (errata/catalogs to trust). Commit it WITH the work.
- Trust the pointers: facts recorded in PROGRESS.md errata, DSL-ERRATA.md and
  WRITE-CATALOG.md are spot-checked against the pinned reference, never re-derived
  from scratch.
- The file is mutable scratch — overwrite, never append; git keeps the trail.
