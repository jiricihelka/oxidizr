//! Corpus discovery: `e2e/corpus/{valid,invalid,mutated}/<nn-name>/`.
//! An entry is a directory with a `workspace.dsl` and/or a `workspace.json`
//! (its children — include / extends files, docs dirs — live beside it; the
//! sidecar `workspace.e2e.json` records the assertions).

use std::fs;
use std::path::{Path, PathBuf};

/// Corpus kind = the top-level subdirectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `corpus/valid` — authored realistic workspaces.
    Valid,
    /// `corpus/invalid` — one targeted failure class per entry.
    Invalid,
    /// `corpus/mutated` — generated mutations (T3; may be absent until then).
    Mutated,
}

impl Kind {
    /// Subdirectory name.
    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            Kind::Valid => "valid",
            Kind::Invalid => "invalid",
            Kind::Mutated => "mutated",
        }
    }
}

/// One corpus entry. Inputs: `workspace.dsl` (DSL primary input) and/or
/// `workspace.json` (JSON primary input for `json2json` / `validate` — the
/// way a raw-JSON failure class like OXZ-4001 enters the corpus). At least
/// one is required; child fixtures (extends/`!include` targets) carry other
/// names and are not entry inputs.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Corpus kind.
    pub kind: Kind,
    /// Directory name (`<nn-feature>-<name>`).
    pub name: String,
    /// Entry directory.
    pub dir: PathBuf,
    /// The entry DSL (`workspace.dsl`), when the entry has one.
    pub dsl: Option<PathBuf>,
    /// The entry JSON workspace fixture (`workspace.json`), when present:
    /// the explicit feed for `json2json` (else the `dsl2json` output is
    /// re-read) and the `validate` target when there is no DSL.
    pub json: Option<PathBuf>,
}

impl Entry {
    /// Fully-qualified name for reports and filters (`valid/01-basic`).
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.kind.dir_name(), self.name)
    }
}

/// Discover all entries under `root` (sorted, deterministic). A kind
/// directory that does not exist yields nothing (`mutated` arrives at T3);
/// an entry directory without an input (`workspace.dsl` or `workspace.json`)
/// is a corpus error.
///
/// # Errors
///
/// Directory read failures, or an entry directory without any input file.
pub(crate) fn discover_in(root: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for kind in [Kind::Valid, Kind::Invalid, Kind::Mutated] {
        let dir = root.join(kind.dir_name());
        if !dir.exists() {
            continue;
        }
        let mut dirs: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for entry_dir in dirs {
            let dsl = entry_dir.join("workspace.dsl");
            let dsl = dsl.is_file().then_some(dsl);
            let json = entry_dir.join("workspace.json");
            let json = json.is_file().then_some(json);
            if dsl.is_none() && json.is_none() {
                return Err(format!(
                    "{}: entry directory without workspace.dsl or workspace.json",
                    entry_dir.display()
                ));
            }
            let name = entry_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            out.push(Entry {
                kind,
                name,
                dir: entry_dir,
                dsl,
                json,
            });
        }
    }
    Ok(out)
}

/// Discover all entries under the repo corpus root.
///
/// # Errors
///
/// See `discover_in`.
pub fn discover() -> Result<Vec<Entry>, String> {
    discover_in(&crate::corpus_root())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_corpus_discovers() {
        let entries = discover().expect("corpus discovers");
        assert!(
            !entries.is_empty(),
            "e2e/corpus is empty — seed entries are missing"
        );
        // Deterministic within each kind (kinds are emitted in
        // valid → invalid → mutated order, sorted by directory name within).
        for kind in [Kind::Valid, Kind::Invalid, Kind::Mutated] {
            let mut names: Vec<String> = entries
                .iter()
                .filter(|e| e.kind == kind)
                .map(|e| e.name.clone())
                .collect();
            let original = names.clone();
            names.sort();
            assert_eq!(names, original, "{kind:?} discovery order must be sorted");
        }
        // Every entry resolves at least one input file.
        assert!(
            entries.iter().all(|e| e.dsl.is_some() || e.json.is_some()),
            "every entry has an input"
        );
    }
}
