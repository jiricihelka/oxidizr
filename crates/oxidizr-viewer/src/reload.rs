//! Live reload preserving layout: watch the opened file
//! plus its `!include` closure, coalesce the raw `notify` storm, and
//! report "the sources changed" as a single edge the shell polls.
//!
//! A plain write, a same-content touch and OUR temp+rename save arrive as
//! EVENT-IDENTICAL notifications — only a content-hash compare tells
//! a real change from noise, so hashes are the authority and events
//! merely schedule a check. FILE watches die on delete/rename swaps,
//! so watches sit on the PARENT DIRECTORIES (`NonRecursive`) with an
//! exact-path filter (the `.tmp` siblings our saves churn can never
//! match). Events are debounced per path-SET: the reload edge fires
//! once [`DEBOUNCE`] has settled after the LAST event, then exactly
//! once per content change — five rapid writes are 13 raw events
//! and must still be one reload.
//!
//! The driver is headless (no eframe): the shell polls
//! [`ReloadDriver::poll`] from its own frame loop and hands its repaint
//! signal to [`ReloadDriver::watch`] so a change wakes a sleeping UI.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};

/// Wake handle the notify thread calls on every raw event that hits
/// the watched set: a cheap "something moved, repaint and poll". The
/// shell supplies `move || ctx.request_repaint` over its cloned
/// paint context — there is no repaint-signal type, that context
/// method IS the cross-thread wake; keeping it a plain
/// closure is what lets this module stay GUI-free (boundary guard).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Settle window after the LAST raw event before the hash check runs:
/// 250 ms. External bursts arrive ~30 ms apart, so this coalesces them
/// into one check.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// The last content hash of a watched file; `None` = unreadable
/// (deleted, mid-rename, zero permissions). An ABSENT file is only
/// worth a reload once the settle window confirms it STAYED gone —
/// delete+recreate inside the window must not storm.
type Hashed = Option<u64>;

/// `SipHash` of a file's full bytes — the dedup authority. Process-local
/// by contract: hashes only ever compare against their own previous
/// reads, never across runs or machines (no byte contract touched).
#[must_use]
pub fn content_hash(path: &Path) -> Hashed {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

/// Lock the shared state, poison-tolerant by policy: a panic on one
/// side leaves valid invariants (plain data, no cross-field contract),
/// and a reload watcher must never take the app down with it.
fn lock<T>(cell: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    cell.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Watched paths + the debounce state, shared between the notify
/// thread (writes) and [`ReloadDriver::poll`] (reads).
#[derive(Debug)]
struct Inner {
    /// Exact watched paths (display file, sidecar, includes) — raw
    /// parent-dir events outside this set are dropped.
    paths: BTreeSet<PathBuf>,
    /// Content hash per path as of the LAST emitted change.
    baseline: Vec<(PathBuf, Hashed)>,
    /// Instant of the last raw filtered event (debounce anchor); the
    /// settle check is `elapsed >= DEBOUNCE`, so no extra flag.
    last_event: Option<Instant>,
    /// A raw event hit the set and no hash check ran yet.
    pending: bool,
}

/// Watches the source set of one opened workspace and reports content
/// changes as debounced edges (see module docs).
#[derive(Debug)]
pub struct ReloadDriver {
    inner: Arc<Mutex<Inner>>,
    /// Kept alive with the driver: dropping it stops the watcher.
    _watcher: notify::RecommendedWatcher,
}

impl ReloadDriver {
    /// Watch `paths` (display file, sidecar if any, `Opened::includes`)
    /// from their CURRENT content. Parent directories get `NonRecursive`
    /// watches (they survive delete/rename); unreadable paths are
    /// watched anyway — their appearance IS a change. `wake` wakes the
    /// UI so a change is polled promptly (see [`Wake`]).
    ///
    /// # Errors
    ///
    /// `notify::Error` when the OS watcher cannot be created or a
    /// directory cannot be watched. A MISSING parent directory is not
    /// an error — nothing can change there yet.
    pub fn watch(paths: &[PathBuf], wake: Wake) -> Result<Self, notify::Error> {
        let set: BTreeSet<PathBuf> = paths.iter().map(|path| canonical_or_self(path)).collect();
        let baseline: Vec<(PathBuf, Hashed)> = set
            .iter()
            .map(|path| (path.clone(), content_hash(path)))
            .collect();
        let inner = Arc::new(Mutex::new(Inner {
            paths: set.clone(),
            baseline,
            last_event: None,
            pending: false,
        }));
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx)?;
        let dirs: BTreeSet<PathBuf> = set
            .iter()
            .filter_map(|path| path.parent().map(Path::to_path_buf))
            .collect();
        for dir in dirs {
            if dir.is_dir() {
                watcher.watch(&dir, RecursiveMode::NonRecursive)?;
            }
        }
        std::thread::spawn({
            let inner = Arc::clone(&inner);
            move || pump(rx, inner, wake)
        });
        Ok(Self {
            inner,
            _watcher: watcher,
        })
    }

    /// Replace the watched set (a reload changed the include closure or
    /// adopted a sidecar). Re-baselines: the new set's current content
    /// is truth, so no self-triggered edge results.
    pub fn reset(&self, paths: &[PathBuf]) {
        let mut inner = lock(&self.inner);
        inner.paths = paths.iter().map(|path| canonical_or_self(path)).collect();
        inner.baseline = inner
            .paths
            .iter()
            .map(|path| (path.clone(), content_hash(path)))
            .collect();
        inner.last_event = None;
        inner.pending = false;
    }

    /// We just wrote `path` OURSELF with `bytes` (our save): pin
    /// its baseline hash to exactly that content, so the rename events
    /// our save fired hash-match and no reload edge can fire for it —
    /// without waiting for the re-baseline `adopt` performs
    /// (saving must not self-trigger).
    pub fn note_written(&self, path: &Path, bytes: &[u8]) {
        let mut inner = lock(&self.inner);
        if let Some(slot) = inner
            .baseline
            .iter_mut()
            .find(|(known, _)| known == &canonical_or_self(path))
        {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            slot.1 = Some(hasher.finish());
        }
    }

    /// Poll from the frame loop: `true` EXACTLY ONCE per real content
    /// change, and only after the sources settled for [`DEBOUNCE`].
    /// The hashes are committed here, so a later revert-to-baseline
    /// (an external editor saving twice) reports no spurious second
    /// edge, and OUR OWN save (hash already current) never fires.
    #[must_use]
    pub fn poll(&self) -> bool {
        let mut inner = lock(&self.inner);
        if !inner.pending || inner.last_event.is_none_or(|at| at.elapsed() < DEBOUNCE) {
            return false;
        }
        let current: Vec<(PathBuf, Hashed)> = inner
            .paths
            .iter()
            .map(|path| (path.clone(), content_hash(path)))
            .collect();
        inner.pending = false;
        if current == inner.baseline {
            return false;
        }
        inner.baseline = current;
        true
    }
}

/// Lexical canonical stand-in: `canonicalize` when the path exists
/// (matches `Opened::display`'s spelling), else keep it as-is (a
/// sidecar created later lives in the same dir either way).
fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The notify thread: filter raw events to the watched set, stamp the
/// debounce anchor, set `pending`, wake the UI. A closed stream
/// (both-ends-dropped test harness) ends the thread quietly.
// Thread entry point: move-in is the idiomatic hand-off even though
// the loop only borrows the three.
#[allow(clippy::needless_pass_by_value)]
fn pump(rx: mpsc::Receiver<notify::Result<notify::Event>>, inner: Arc<Mutex<Inner>>, wake: Wake) {
    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if !interesting(event.kind) {
                    continue;
                }
                let hit = lock(&inner).paths.iter().any(|known| {
                    event
                        .paths
                        .iter()
                        .any(|path| &canonical_or_self(path) == known)
                });
                if hit {
                    let mut state = lock(&inner);
                    state.last_event = Some(Instant::now());
                    state.pending = true;
                }
                wake();
            }
            Ok(Err(_)) => {}
            // Stream closed (harness dropped the sender): stop.
            Err(_) => break,
        }
    }
}

/// Content-relevant kinds: data writes, create/remove and the
/// rename both-ends; access/other kinds are noise (`Access(Open)` fires
/// on every plain read, ours included).
///
/// The path filter + settle + content-hash diff in
/// [`ReloadDriver::poll`] is the edge contract; this is only a
/// per-backend noise pre-filter. The Windows backend maps EVERY
/// `FILE_ACTION_MODIFIED` — content write or mtime touch alike — to
/// `Modify(Any)` and never to `Modify(Data)`, so the inotify/fsevent
/// data classes alone drop all plain writes there. Accepting `Any` on
/// Windows cannot fire a false edge — a metadata-only write still
/// hash-matches in `poll` and stays silent — while Linux/macOS keep the
/// data-only classes verbatim.
fn interesting(kind: EventKind) -> bool {
    #[cfg(not(target_os = "windows"))]
    if matches!(
        kind,
        EventKind::Modify(notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Name(_))
    ) {
        return true;
    }
    // Windows: `Modify(Any)` IS the data-write class (comment above).
    #[cfg(target_os = "windows")]
    if matches!(
        kind,
        EventKind::Modify(
            notify::event::ModifyKind::Any
                | notify::event::ModifyKind::Data(_)
                | notify::event::ModifyKind::Name(_),
        )
    ) {
        return true;
    }
    matches!(kind, EventKind::Create(_) | EventKind::Remove(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(topic: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oxidizr-m41-{topic}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn wake() -> Wake {
        Arc::new(|| {})
    }

    /// Poll until an edge or the budget runs out (events are async;
    /// the budget is generous, a green run exits early).
    fn wait_for_edge(driver: &ReloadDriver) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if driver.poll() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn interesting_maps_the_per_backend_event_classes() {
        use notify::event::{
            AccessKind, AccessMode, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind,
            RenameMode,
        };
        // Cross-platform classes: data writes, create/remove,
        // rename both-ends are relevant; access/other/metadata are not.
        assert!(interesting(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(interesting(EventKind::Modify(ModifyKind::Name(
            RenameMode::To
        ))));
        assert!(interesting(EventKind::Create(CreateKind::File)));
        assert!(interesting(EventKind::Remove(RemoveKind::File)));
        assert!(!interesting(EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))));
        assert!(!interesting(EventKind::Other));
        assert!(!interesting(EventKind::Any));
        assert!(!interesting(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::WriteTime
        ))));
        assert!(!interesting(EventKind::Modify(ModifyKind::Other)));
        // The Windows seam: that backend reports every plain write as
        // `Modify(Any)`, so only that OS accepts it here
        // (the hash layer in `poll` is the cross-platform authority).
        assert_eq!(
            interesting(EventKind::Modify(ModifyKind::Any)),
            cfg!(target_os = "windows")
        );
    }

    #[test]
    fn change_emits_exactly_one_edge_and_identical_rewrites_are_silent() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let dir = temp_dir("edge");
        let file = dir.join("demo.dsl");
        std::fs::write(&file, "v1").unwrap();
        let woken = Arc::new(AtomicBool::new(false));
        let wake: Wake = {
            let woken = Arc::clone(&woken);
            Arc::new(move || woken.store(true, Ordering::Relaxed))
        };
        let driver = ReloadDriver::watch(std::slice::from_ref(&file), wake).unwrap();

        std::fs::write(&file, "v2").unwrap();
        assert!(wait_for_edge(&driver), "external change must fire");
        assert!(woken.load(Ordering::Relaxed), "the thread must wake the UI");
        // Edge consumed: poll stays false until the NEXT change.
        std::thread::sleep(DEBOUNCE + Duration::from_millis(100));
        assert!(!driver.poll(), "exactly once per change");

        // OUR save posture (`write_workspace`): temp write +
        // rename of bytes IDENTICAL to the current content — this is
        // event-indistinguishable from an external atomic
        // save, so only the hash filter can keep it silent. It must.
        let save = |bytes: &[u8]| {
            std::fs::write(dir.join("demo.dsl.123.tmp"), bytes).unwrap();
            std::fs::rename(dir.join("demo.dsl.123.tmp"), &file).unwrap();
        };
        save(b"v2");
        std::thread::sleep(DEBOUNCE + Duration::from_millis(300));
        assert!(!driver.poll(), "our own save (same bytes) is hash-silent");

        // Within one settle window a write-then-revert nets to the
        // baseline ⇒ no edge either (the hash check runs only after
        // DEBOUNCE of quiet).
        std::fs::write(&file, "gone temporarily").unwrap();
        std::fs::write(&file, "v2").unwrap();
        std::thread::sleep(DEBOUNCE + Duration::from_millis(300));
        assert!(!driver.poll(), "a net-zero burst is one silent check");
    }

    #[test]
    fn rapid_burst_debounce_coalesces_into_one_edge() {
        let dir = temp_dir("burst");
        let file = dir.join("burst.dsl");
        std::fs::write(&file, "0").unwrap();
        let driver = ReloadDriver::watch(std::slice::from_ref(&file), wake()).unwrap();
        // 5 writes @30 ms — 13 raw events, one reload.
        for i in 1..=5 {
            std::fs::write(&file, format!("{i}")).unwrap();
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(wait_for_edge(&driver));
        std::thread::sleep(DEBOUNCE + Duration::from_millis(200));
        assert!(!driver.poll(), "the burst was ONE edge");
    }

    #[test]
    fn unwatched_sibling_temp_churn_stays_silent() {
        let dir = temp_dir("sibling");
        let file = dir.join("watched.dsl");
        std::fs::write(&file, "keep").unwrap();
        let driver = ReloadDriver::watch(std::slice::from_ref(&file), wake()).unwrap();
        for i in 0..3 {
            std::fs::write(dir.join(format!("watched.dsl.{i}.tmp")), "junk").unwrap();
            std::fs::write(dir.join("other.txt"), "junk").unwrap();
        }
        std::thread::sleep(DEBOUNCE + Duration::from_millis(300));
        assert!(!driver.poll(), "the exact-path filter drops siblings");
    }

    #[test]
    fn reset_rebaselines_so_a_reload_triggers_nothing() {
        let dir = temp_dir("reset");
        let main = dir.join("main.dsl");
        let included = dir.join("child.dsl");
        std::fs::write(&main, "!include child.dsl").unwrap();
        std::fs::write(&included, "a").unwrap();
        let driver = ReloadDriver::watch(&[main.clone(), included.clone()], wake()).unwrap();
        // A reload swaps the closure: child out, new sidecar in.
        let sidecar = dir.join("main.json");
        std::fs::write(&sidecar, "{}").unwrap();
        driver.reset(&[main, sidecar.clone()]);
        // The child (now unwatched) changing must not fire…
        std::fs::write(&included, "changed and unwatched").unwrap();
        // …and neither must the new set's mere existence (re-baselined).
        std::thread::sleep(DEBOUNCE + Duration::from_millis(300));
        assert!(!driver.poll());
        // The new set IS watched: sidecar edits fire.
        std::fs::write(&sidecar, "{\"id\": 1}").unwrap();
        assert!(wait_for_edge(&driver));
    }

    #[test]
    fn note_written_pins_the_save_so_our_rename_never_reloads() {
        let dir = temp_dir("note");
        let file = dir.join("saved.dsl");
        std::fs::write(&file, "base").unwrap();
        let driver = ReloadDriver::watch(std::slice::from_ref(&file), wake()).unwrap();
        // Our save: write temp, pin, rename (the save order).
        let bytes = b"saved geometry".to_vec();
        std::fs::write(dir.join("saved.dsl.7.tmp"), &bytes).unwrap();
        driver.note_written(&file, &bytes);
        std::fs::rename(dir.join("saved.dsl.7.tmp"), &file).unwrap();
        std::thread::sleep(DEBOUNCE + Duration::from_millis(300));
        assert!(!driver.poll(), "our save (pinned) must not self-trigger");
        // An external change still fires.
        std::fs::write(&file, "external edit").unwrap();
        assert!(wait_for_edge(&driver));
    }
}
