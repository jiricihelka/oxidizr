//! Observation: run every applicable command over one entry through
//! `oxidizr_api::execute` and reduce the [`Outcome`]s to manifest form
//! (exit class + sorted code set + counts / idempotence). The quick-tier
//! test compares that against the blessed sidecar; `cargo xtask e2e bless`
//! writes it.

use oxidizr_api::{ExitClass, FmtMode, Input, LogItem, Outcome, Request, execute};
use oxidizr_core::validate::flattened;

use crate::corpus::Entry;
use crate::manifest::{CommandReport, Counts, Exit, FmtReport, Manifest};

/// Run the entry's commands; returns the observed manifest form. Applicable
/// commands follow the entry's inputs: DSL commands (`dsl2json`, `fmt`) need
/// `workspace.dsl`; `json2json` reads the explicit `workspace.json` feed
/// when present, else re-exports the `dsl2json` output (when that
/// succeeded); `validate` takes whichever input exists (DSL first).
#[must_use]
pub fn observe(entry: &Entry) -> Manifest {
    let opts = crate::hermetic_options();

    // dsl2json (the structural anchor: counts and the json2json feed come
    // from its export).
    let dsl_out = entry.dsl.as_ref().map(|dsl| {
        execute(
            Request::Dsl2Json {
                input: Input::File(dsl.clone()),
                output: None,
            },
            &opts,
        )
    });
    let counts = match &dsl_out {
        Some(out) if out.exit == ExitClass::Ok => out.bytes.as_deref().map(counts_of_json),
        _ => None,
    };
    let dsl2json = dsl_out.as_ref().map(|out| CommandReport {
        exit: Exit::from_class(out.exit),
        codes: codes_of(out),
        counts,
    });

    // json2json: the explicit `workspace.json` feed when present; else the
    // dsl2json output re-read (only when that succeeded).
    let json2json = if let Some(json) = &entry.json {
        let out = execute(
            Request::Json2Json {
                input: Input::File(json.clone()),
                output: None,
            },
            &opts,
        );
        Some(CommandReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            counts: None,
        })
    } else if dsl2json.as_ref().is_some_and(|r| r.exit == Exit::Ok) {
        let out = execute(
            Request::Json2Json {
                input: Input::Bytes(dsl_out.unwrap_or_default().bytes.unwrap_or_default()),
                output: None,
            },
            &opts,
        );
        Some(CommandReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            counts: None,
        })
    } else {
        None
    };

    // validate (extension-detected): the DSL entry when it has one, else
    // the JSON fixture.
    let validate_file = entry.dsl.as_ref().or(entry.json.as_ref());
    let validate = validate_file.map(|file| {
        let out = execute(
            Request::Validate {
                files: vec![file.clone()],
            },
            &opts,
        );
        CommandReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            counts: None,
        }
    });

    // fmt: fixed-point property over the entry text (P1T-D3/D6 seed).
    let fmt = entry.dsl.as_ref().map(|dsl| {
        let first = execute(
            Request::Fmt {
                inputs: vec![Input::File(dsl.clone())],
                mode: FmtMode::Print,
            },
            &opts,
        );
        let idempotent = match &first.bytes {
            Some(text1) => {
                let second = execute(
                    Request::Fmt {
                        inputs: vec![Input::Bytes(text1.clone())],
                        mode: FmtMode::Print,
                    },
                    &opts,
                );
                second.bytes.as_deref() == Some(text1.as_slice())
            }
            // No payload (I/O failure): the exit class records it; vacuously
            // true.
            None => true,
        };
        FmtReport {
            exit: Exit::from_class(first.exit),
            idempotent,
        }
    });

    Manifest {
        dsl2json,
        json2json,
        validate,
        fmt,
    }
}

/// Check one entry against its blessed sidecar; empty vec = green.
///
/// # Errors
///
/// Sidecar read/parse failures via [`Manifest::read`] (behavioral drift is
/// the returned `Vec`, not an error).
pub fn check(entry: &Entry) -> Result<Vec<String>, String> {
    let sidecar = entry.dir.join(crate::manifest::FILE_NAME);
    let expected = Manifest::read(&sidecar)?;
    let observed = observe(entry);
    Ok(expected.diff(&observed))
}

/// Sorted, deduplicated OXZ code set of every diagnostic on the outcome.
fn codes_of(outcome: &Outcome) -> Vec<u16> {
    let mut codes: Vec<u16> = outcome
        .log
        .iter()
        .filter_map(|item| match item {
            LogItem::Issue(issue) => Some(issue.code.get()),
            LogItem::OkLine(_) | LogItem::NoteLine(_) => None,
        })
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// Structural counts of an exported JSON workspace. Volatile fields
/// (`structurizr.dsl.identifier` UUIDs, `lastModified*`) never enter. A
/// re-read failure yields zero counts and no panic — the manifest review
/// catches a surprising zero.
fn counts_of_json(bytes: &[u8]) -> Counts {
    match oxidizr_json::from_slice(bytes) {
        Ok(ws) => Counts {
            elements: flattened(&ws.model).len(),
            relationships: ws.model.relationships.len(),
            views: view_count(&ws),
        },
        Err(_) => Counts {
            elements: 0,
            relationships: 0,
            views: 0,
        },
    }
}

/// Views across all eight kinds.
fn view_count(ws: &oxidizr_core::Workspace) -> usize {
    let v = &ws.views;
    v.custom_views.len()
        + v.system_landscape_views.len()
        + v.system_context_views.len()
        + v.container_views.len()
        + v.component_views.len()
        + v.dynamic_views.len()
        + v.filtered_views.len()
        + v.deployment_views.len()
}
