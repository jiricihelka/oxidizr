//! The `<stem>.oxidizr.json` sidecar: oxidizr-only per-file state the
//! Structurizr JSON model cannot carry.
//!
//! This is a PURE CACHE:
//! * it is OUR OWN JSON — a versioned envelope, never fed to or read from
//!   the contract reader/writer (`oxidizr_json` is not referenced here);
//! * it is NEVER an open or reload source — the layout sidecar for
//!   `big.dsl` is `big.json` (a different exact filename), and nothing in
//!   `reload`/`open` ever lists this file, so the newest-wins reload
//!   machinery can never adopt it;
//! * deleting it always loses nothing. Every read is best-effort: a
//!   missing, unreadable, unparseable, wrong-version or non-object file
//!   all decode to the same empty state, and writes never clobber a newer
//!   file on a version mismatch.
//!
//! The only user so far is per-view viewport memory: the zoom + world
//! centre a view was last left at, restored when the file is reopened.
//!
//! Writes are atomic (temp file + rename in the same directory) so a
//! crash mid-write can never leave a half-written sidecar beside the
//! user's workspace.

use std::collections::BTreeMap;
use std::io;

/// Versioned-envelope schema tag. A future format bump increments this
/// and the loader starts ignoring files it does not understand.
const FORMAT: &str = "oxidizr-viewer-state";
const VERSION: u64 = 1;
/// The sidecar suffix appended to the source path — `big.dsl` and
/// `big.json` BOTH get `….oxidizr.json`, distinct from the contract
/// layout sidecar `big.json`.
const SUFFIX: &str = ".oxidizr.json";

/// The remembered camera for one view: the viewport scale plus the WORLD
/// point at the canvas centre. Storing world coords (not the screen
/// offset) keeps the memory resolution-independent — on reopen the offset
/// is recomputed for whatever canvas size the first frame reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewMemory {
    /// Zoom scale (the viewport's, already clamped by the shell).
    pub zoom: f64,
    /// The world point the canvas was centred on.
    pub centre: (f64, f64),
}

/// The per-file state: view key → remembered camera. A `BTreeMap` so the
/// JSON key order is deterministic (the write stays byte-stable).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewerState {
    /// One entry per view the user has panned/zoomed this session.
    pub views: BTreeMap<String, ViewMemory>,
}

impl ViewerState {
    /// Nothing remembered (the decode fallback for any unusable file).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }
}

/// The sidecar path for a source `display`: the source's DIRECTORY plus
/// its STEM with `.oxidizr.json` appended — `work/big.dsl` →
/// `work/big.oxidizr.json`. The `big.dsl`/`big.json` companion PAIR
/// therefore shares ONE state file, which is exactly right: they are the
/// same workspace, and the display flips between them the moment a save
/// creates the sidecar — the viewport memory survives that transition.
/// A different stem is a different workspace; the contract layout sidecar
/// `big.json` is a different exact filename and can never collide. Empty
/// and stdin (`-`) have no sidecar — there is nothing beside them.
#[must_use]
pub fn sidecar_path(display: &str) -> Option<std::path::PathBuf> {
    if display.is_empty() || display == "-" {
        return None;
    }
    let given = std::path::Path::new(display);
    let stem = given.file_stem()?.to_str()?;
    let name = format!("{stem}{SUFFIX}");
    Some(match given.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => std::path::PathBuf::from(name),
    })
}

/// Read the sidecar best-effort. ANY failure — absent file, unreadable,
/// not valid JSON, wrong format/version, a non-object — yields the empty
/// state (this is a cache, never an error path).
#[must_use]
pub fn load(path: &std::path::Path) -> ViewerState {
    let Ok(text) = std::fs::read_to_string(path) else {
        return ViewerState::default();
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&text) else {
        return ViewerState::default();
    };
    let Some(obj) = root.as_object() else {
        return ViewerState::default();
    };
    if obj.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT)
        || obj.get("version").and_then(serde_json::Value::as_u64) != Some(VERSION)
    {
        return ViewerState::default();
    }
    let mut state = ViewerState::default();
    // A missing/!object `views` is accepted (a state file with no users
    // yet is legal); a per-view entry that is malformed is skipped.
    if let Some(views) = obj.get("views").and_then(serde_json::Value::as_object) {
        for (key, value) in views {
            let Some(number) = value.get("zoom").and_then(serde_json::Value::as_f64) else {
                continue;
            };
            let (Some(x), Some(y)) = (
                value.get("x").and_then(serde_json::Value::as_f64),
                value.get("y").and_then(serde_json::Value::as_f64),
            ) else {
                continue;
            };
            if !number.is_finite() || !x.is_finite() || !y.is_finite() {
                continue;
            }
            state.views.insert(
                key.clone(),
                ViewMemory {
                    zoom: number,
                    centre: (x, y),
                },
            );
        }
    }
    state
}

/// Write the sidecar atomically (temp + rename in the target's directory).
///
/// Refuses to OVERWRITE a file that already parses as a NEWER or
/// different-format envelope (someone/something else owns it) — a pure
/// cache never destroys unknown data. A file that is unparseable garbage
/// is replaced (by definition it is ours and unusable).
///
/// # Errors
///
/// A human-readable description of: a refusal to clobber a newer
/// envelope, a serialization failure, a path without a file name, or the
/// temp-write/rename `io::Error`. The caller treats every error as
/// "cache not saved" — never as a session failure.
pub fn store(path: &std::path::Path, state: &ViewerState) -> Result<(), String> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && let Ok(root) = serde_json::from_str::<serde_json::Value>(&existing)
    {
        let version = root
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let format = root.get("format").and_then(serde_json::Value::as_str);
        // A recognized envelope at a version we do not write: leave it.
        if format == Some(FORMAT) && version > VERSION {
            return Err(format!(
                "left newer {FORMAT} v{version} (this build writes v{VERSION})"
            ));
        }
    }
    let json = serde_json::to_vec_pretty(&envelope(state))
        .map_err(|e| format!("serialize sidecar: {e}"))?;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "sidecar path has no file name".to_owned())?;
    // A pid-uniquified temp name in the SAME directory (rename must not
    // cross a filesystem boundary); distinct pids never collide.
    let tmp_name = format!("{file}.{}.tmp", std::process::id());
    let tmp = match dir {
        Some(d) => d.join(tmp_name),
        None => std::path::PathBuf::from(tmp_name),
    };
    let write = (|| -> Result<(), io::Error> {
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if write.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    write.map_err(|e| format!("write {}: {e}", path.display()))
}

/// The JSON envelope: `{ "format":…, "version":…, "views": { key:
/// {zoom,x,y}, … } }`. Hand-built (no `Serialize` derive on our types)
/// so the schema stays exactly this literal shape.
fn envelope(state: &ViewerState) -> serde_json::Value {
    let mut views = serde_json::Map::new();
    for (key, memory) in &state.views {
        views.insert(
            key.clone(),
            serde_json::json!({
                "zoom": memory.zoom,
                "x": memory.centre.0,
                "y": memory.centre.1,
            }),
        );
    }
    serde_json::json!({
        "format": FORMAT,
        "version": VERSION,
        "views": serde_json::Value::Object(views),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(topic: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("oxidizr-m1112-{topic}-{}", std::process::id()))
    }

    fn fresh(topic: &str) -> std::path::PathBuf {
        let d = dir(topic);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn memory(zoom: f64, x: f64, y: f64) -> ViewMemory {
        ViewMemory {
            zoom,
            centre: (x, y),
        }
    }

    #[test]
    fn sidecar_path_uses_the_stem_and_skips_stdin() {
        // Exact filename `<stem>.oxidizr.json`: the big.dsl / big.json
        // companion PAIR shares one state file and neither can collide
        // with the contract layout sidecar `big.json`.
        assert_eq!(
            sidecar_path("work/big.dsl").unwrap(),
            std::path::PathBuf::from("work/big.oxidizr.json")
        );
        assert_eq!(
            sidecar_path("work/big.json").unwrap(),
            std::path::PathBuf::from("work/big.oxidizr.json")
        );
        assert_eq!(
            sidecar_path("big.dsl").unwrap(),
            std::path::PathBuf::from("big.oxidizr.json")
        );
        assert_ne!(
            sidecar_path("big.dsl").unwrap(),
            std::path::PathBuf::from("big.json")
        );
        assert_eq!(sidecar_path("-"), None);
        assert_eq!(sidecar_path(""), None);
    }

    #[test]
    fn round_trip_is_exact_and_deterministic() {
        let d = fresh("roundtrip");
        let path = d.join("ws.dsl.oxidizr.json");
        let mut state = ViewerState::default();
        state
            .views
            .insert("b".to_owned(), memory(1.5, -20.0, 40.25));
        state
            .views
            .insert("a".to_owned(), memory(0.4, 100.0, 200.0));
        store(&path, &state).unwrap();
        let back = load(&path);
        assert_eq!(back, state);
        // Byte-stable: keys sorted, same bytes on a rewrite.
        let first = std::fs::read(&path).unwrap();
        store(&path, &state).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        // The envelope is recognizable and NOT a Structurizr workspace.
        let text = String::from_utf8(first).unwrap();
        assert!(
            text.contains("\"format\": \"oxidizr-viewer-state\""),
            "{text}"
        );
        assert!(
            !text.contains("configuration"),
            "must never look like contract JSON"
        );
    }

    #[test]
    fn empty_state_writes_and_reads_clean() {
        let d = fresh("empty");
        let path = d.join("e.json.oxidizr.json");
        store(&path, &ViewerState::default()).unwrap();
        let back = load(&path);
        assert!(back.is_empty());
    }

    #[test]
    fn unreadable_inputs_decode_to_empty() {
        let d = fresh("unreadable");
        // Absent file.
        assert!(load(&d.join("nope.json.oxidizr.json")).is_empty());
        // Garbage (unparseable).
        let garbage = d.join("g.oxidizr.json");
        std::fs::write(&garbage, b"not json at all {{{").unwrap();
        assert!(load(&garbage).is_empty());
        // Valid JSON but not our envelope.
        let other = d.join("o.oxidizr.json");
        std::fs::write(&other, br#"{"configuration":{}}"#).unwrap();
        assert!(load(&other).is_empty());
        // Our format but a future version → ignored (never crash).
        let future = d.join("f.oxidizr.json");
        std::fs::write(
            &future,
            br#"{"format":"oxidizr-viewer-state","version":99,"views":{"a":{"zoom":1,"x":0,"y":0}}}"#,
        )
        .unwrap();
        assert!(load(&future).is_empty());
        // Wrong per-view shape is skipped, a good sibling survives.
        let mixed = d.join("m.oxidizr.json");
        std::fs::write(
            &mixed,
            br#"{"format":"oxidizr-viewer-state","version":1,"views":{"bad":{"zoom":"x"},"good":{"zoom":2,"x":1,"y":3}}}"#,
        )
        .unwrap();
        let back = load(&mixed);
        assert_eq!(back.views.keys().collect::<Vec<_>>(), vec!["good"]);
        assert_eq!(back.views["good"], memory(2.0, 1.0, 3.0));
    }

    #[test]
    fn store_refuses_to_overwrite_a_newer_envelope() {
        let d = fresh("newer");
        let path = d.join("n.oxidizr.json");
        std::fs::write(
            &path,
            br#"{"format":"oxidizr-viewer-state","version":2,"views":{"keep":{"zoom":9,"x":9,"y":9}}}"#,
        )
        .unwrap();
        let mut state = ViewerState::default();
        state.views.insert("mine".to_owned(), memory(1.0, 0.0, 0.0));
        let err = store(&path, &state).unwrap_err();
        assert!(err.contains("newer"), "{err}");
        // Untouched — and note `load` deliberately ignores the v2 file,
        // so the check is on the raw bytes.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("keep"), "{text}");
    }

    #[test]
    fn store_replaces_unparseable_garbage_and_leaves_no_temp() {
        let d = fresh("garbage-write");
        let path = d.join("g.oxidizr.json");
        std::fs::write(&path, b"total garbage \x00\x01").unwrap();
        let mut state = ViewerState::default();
        state.views.insert("v".to_owned(), memory(1.0, 5.0, 6.0));
        store(&path, &state).unwrap();
        assert_eq!(load(&path).views.keys().collect::<Vec<_>>(), vec!["v"]);
        // No `.tmp` litter left behind in the directory.
        let litter: Vec<String> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp"))
            .collect();
        assert_eq!(litter, Vec::<String>::new());
    }
}
