//! V-T7 (P2T-D8): no path leakage — exporting the same entry into two
//! different scratch directories must produce byte-identical files (diagram
//! text is built from view keys/names only, never from the output path —
//! the `oxidizr-export::naming` promise checked end-to-end). Rides the
//! quick tier: three entries × two formats is milliseconds.
#![allow(clippy::unwrap_used)]

use oxidizr_e2e::corpus;

#[test]
fn export_bytes_do_not_depend_on_the_scratch_path() {
    let entries = corpus::discover().expect("corpus discovers");
    let opts = oxidizr_e2e::hermetic_options();

    // Deterministic sample: the first three export-applicable entries in
    // discovery order (blessed manifests say applicability).
    let mut sampled = 0;
    for entry in &entries {
        if sampled >= 3 {
            break;
        }
        let sidecar = entry.dir.join(oxidizr_e2e::manifest::FILE_NAME);
        let Ok(manifest) = oxidizr_e2e::manifest::Manifest::read(&sidecar) else {
            continue;
        };
        if manifest.export.is_none() {
            continue;
        }
        sampled += 1;
        let input = entry
            .dsl
            .as_ref()
            .or(entry.json.as_ref())
            .unwrap_or_else(|| panic!("{}: applicable export implies an input", entry.full_name()));
        for format in ["plantuml", "mermaid"] {
            let (out_a, files_a) = oxidizr_e2e::runner::export_capture(
                input,
                format,
                &opts,
                &format!("v7a-{}", entry.name),
            );
            let (out_b, files_b) = oxidizr_e2e::runner::export_capture(
                input,
                format,
                &opts,
                &format!("v7b-{}", entry.name),
            );
            assert_eq!(
                out_a.exit,
                out_b.exit,
                "{} {format}: exit differs across scratch paths",
                entry.full_name()
            );
            assert_eq!(
                files_a.keys().collect::<Vec<_>>(),
                files_b.keys().collect::<Vec<_>>(),
                "{} {format}: file names differ across scratch paths",
                entry.full_name()
            );
            for (name, bytes) in &files_a {
                assert!(
                    !name.contains('/') && !name.contains('\\'),
                    "{} {format}: output name embeds a path: {name}",
                    entry.full_name()
                );
                assert_eq!(
                    bytes,
                    &files_b[name],
                    "{} {format}: bytes of `{name}` differ across scratch paths",
                    entry.full_name()
                );
            }
        }
    }
    assert_eq!(sampled, 3, "expected three export-applicable entries");
}
