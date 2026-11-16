# `git mv` on a directory containing untracked files

Moving `plan/` → `agents/plan/` with `git mv plan agents/plan` + `rmdir plan`
lost untracked `plan/reports/windows_test_error.txt` — it surfaced in
`~/.local/share/Trash/files/reports/` (this desktop environment trashes rather
than deletes; `cmp` confirmed the copy was byte-identical, restore was trivial).

Rules:
- Before moving/renaming a directory: `git status --short --untracked-files=all
  <dir>` and stash or copy out anything untracked.
- If a file vanishes this way: check `~/.local/share/Trash/files/` FIRST (fast),
  then `git fsck --unreachable` for blobs, then the opencode session DB
  (`session_message.data` holds tool outputs — searchable via
  `nix run nixpkgs#sqlite -- ~/.local/share/opencode/opencode.db`).
- `agents/plan/reports/` stays untracked (user-dropped CI reports, scratch).

Last useful: never
