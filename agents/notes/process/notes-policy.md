# notes policy (write more permanent notes)

Why: long phases (e.g. M4) hit many context compressions; a fresh session that
must re-read big files or re-derive tool behavior wastes budget and risks
re-litigating settled decisions. Short notes here are the recall layer.

Write a note when a fix/behavior was NON-OBVIOUS: cost debugging time, surprised
you, or would fail plausible-but-wrong assumptions. Also when the user drops an OS
report and the root cause is environmental.

Format: ≤ 25 lines; facts + the command that verifies them; link don't duplicate —
decisions belong in DEVIATIONS.md/ADDITIONAL_FEATURES.md, phase state in
PROGRESS.md, this tree only holds operational memory.

Placement: pick the subdir (env/pitfalls/testing/process), add the file, add one
row to that subdir's README table (and to notes/README.md's copy of it). New topic
family → new subdir with its own README, cross-linked from notes/README.md.
Prune/merge when a note goes stale — stale notes are worse than none.
