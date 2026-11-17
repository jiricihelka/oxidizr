//! Sidecar regeneration: `cargo xtask e2e bless [FILTER]` (P1T-D3). The
//! git diff of a bless run IS the behavior-change review artifact — an
//! intentional deviation in a later phase re-blesses, and the reviewed diff
//! records it (the tests never rot as obstacles).

use crate::corpus::{self, Entry};
use crate::{display, manifest, runner};

/// Regenerate `workspace.e2e.json` for every entry whose fully-qualified
/// name contains `filter` (all when `None`). Returns one report line per
/// blessed entry (new vs unchanged included in the text).
///
/// # Errors
///
/// Corpus discovery failures, sidecar write failures, or no entries
/// matching the filter.
pub fn bless(filter: Option<&str>) -> Result<Vec<String>, String> {
    let entries = corpus::discover()?;
    bless_entries(&entries, filter)
}

fn bless_entries(entries: &[Entry], filter: Option<&str>) -> Result<Vec<String>, String> {
    let mut reports = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for entry in entries {
        let name = entry.full_name();
        if filter.is_some_and(|f| !name.contains(f)) {
            continue;
        }
        let observed = runner::observe(entry);
        let sidecar = entry.dir.join(manifest::FILE_NAME);
        let state = match manifest::Manifest::read(&sidecar) {
            Ok(existing) if existing == observed => "unchanged",
            Ok(_) => "updated",
            Err(_) => "created",
        };
        if let Err(e) = observed.write(&sidecar) {
            errors.push(e);
        } else {
            reports.push(format!("{name}: {state}"));
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    if reports.is_empty() {
        return Err(format!(
            "no corpus entries matched (root: {})",
            display(&crate::corpus_root())
        ));
    }
    Ok(reports)
}
