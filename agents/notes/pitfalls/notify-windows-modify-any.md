# notify 8 on Windows: plain writes arrive as `Modify(Any)`, never `Modify(Data)`

Symptom: the four `wait_for_edge` reload tests fail ONLY on the Windows CI
matrix leg (`crates/oxidizr-viewer/src/reload.rs`) — the event-driven ones
time out; the silence-only test passes (a broken pre-filter is also silent).

Reference fact (notify 8.2.1 `src/windows.rs`, ReadDirectoryChangesW loop):
`FILE_ACTION_MODIFIED` → `EventKind::Modify(ModifyKind::Any)`. The backend
never emits `Modify(Data)`; renames map to `Modify(Name(From|To))`, adds to
`Create(Any)`, deletes to `Remove(Any)`. inotify/fsevent DO emit
`Modify(Data)` — filtering on it silently drops every watched-file write on
Windows (and `fs::write` is truncate+write, no rename event to ride).

Fix (`reload.rs interesting()`, §P3-M9): accept `ModifyKind::Any` under
`#[cfg(target_os = "windows")]` only. Safe because the edge contract is the
path filter + settle + content-hash diff in `poll()` — a metadata-only
`Modify(Any)` still hash-matches and stays silent. Unit test
`interesting_maps_the_per_backend_event_classes` pins the platform mapping.

Gotchas: `#[cfg]` attributes are REJECTED inside a `matches!` `pat`
fragment — split into two cfg-gated `if` arms. Variant spellings:
`ModifyKind::Other` (not `Others`), `AccessKind::Open(AccessMode)`,
`DataChange` (not `DataKind`).

Wine caveat (`env/wine-windows-leg.md`): wine synthesizes
ReadDirectoryChangesW from inotify — green under wine is WEAK evidence for
real NTFS; the windows-latest CI leg is the authoritative re-check. (Here
wine at least reproduced the exact 4-test failure pre-fix and went green
post-fix — negative control done 2026-11-20.)

Last useful: 2026-11-20 (fixing the Windows reload CI failures)
