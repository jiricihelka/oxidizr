//! Observation: run every applicable command over one entry through
//! `oxidizr_api::execute` and reduce the [`Outcome`]s to manifest form
//! (exit class + sorted code set + counts / idempotence). The quick-tier
//! test compares that against the blessed sidecar; `cargo xtask e2e bless`
//! writes it.

use oxidizr_api::{ExitClass, FmtMode, Input, LogItem, Outcome, Request, execute};
use oxidizr_core::validate::flattened;

use crate::corpus::Entry;
use crate::manifest::{
    AxisReport, CommandReport, Counts, Exit, ExportReport, ExportReports, FmtReport, Manifest,
};

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
        // 2R R-tool-04: the guard already proves `dsl_out` is the SUCCESSFUL
        // dsl2json, so its bytes ARE the feed — one explicit empty-bytes
        // fallback (the value the old double `unwrap_or_default` smuggled for
        // the legs the guard makes unreachable) instead of nested swallowers.
        let feed = dsl_out.and_then(|out| out.bytes).unwrap_or_default();
        let out = execute(
            Request::Json2Json {
                input: Input::Bytes(feed),
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
    let validate = entry
        .dsl
        .as_ref()
        .or(entry.json.as_ref())
        .map(|file| observe_validate(file, &opts));

    // reconcile (T3): applies when the entry carries BOTH inputs.
    let reconcile = observe_reconcile(entry, &opts);

    // export matrix (P2T-D2): diagram formats + compat probe into scratch
    // dirs; applicable when the input parses with ≥ 1 view.
    let export = observe_export(entry, &opts, dsl2json.as_ref(), json2json.as_ref());

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

    // §P4T T-3: the three Phase-4 axes over the entry's own input and the
    // `generate` leg over the entry directory (helpers keep `observe`
    // within the line budget).
    let (inspect, query, diff) = observe_axes(entry, &opts);
    let generate = Some(observe_generate(entry, &opts));

    Manifest {
        dsl2json,
        json2json,
        validate,
        reconcile,
        export,
        fmt,
        inspect,
        query,
        diff,
        generate,
    }
}

/// The v3 axes over the entry's OWN input (validate's DSL-first rule):
/// `inspect` violations at the CLI default threshold (`info`), `query`
/// matches of the fixed smoke filter (every model element carries
/// `Element`, so it is the grammar's identity probe), and `diff` of the
/// file against ITSELF — the identity axis, 0 findings for every input the
/// loader accepts. All three apply whenever the entry has an input at all;
/// load failures surface as exit + codes (§P4T T-3).
fn observe_axes(
    entry: &Entry,
    opts: &oxidizr_api::ApiOptions,
) -> (Option<AxisReport>, Option<AxisReport>, Option<AxisReport>) {
    let axis_input = entry.dsl.as_ref().or(entry.json.as_ref());
    let inspect = axis_input.map(|file| {
        let out = execute(
            Request::Inspect {
                workspace: file.clone(),
                threshold: oxidizr_api::Severity::Info,
                only: Vec::new(),
                except: Vec::new(),
                json: true,
                quiet: false,
            },
            opts,
        );
        AxisReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            count: report_count(out.bytes.as_deref(), &["violations"]),
        }
    });
    let query = axis_input.map(|file| {
        let out = execute(
            Request::Query {
                workspace: file.clone(),
                filter: "element.tag==Element".into(),
                kind: oxidizr_api::QueryKind::All,
                format: oxidizr_api::QueryFormat::Json,
                output: None,
            },
            opts,
        );
        AxisReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            count: report_count(
                out.bytes.as_deref(),
                &["elements", "relationships", "views"],
            ),
        }
    });
    let diff = axis_input.map(|file| {
        let out = execute(
            Request::Diff {
                a: file.clone(),
                b: file.clone(),
                json: true,
                ignore_layout: false,
                quiet: false,
            },
            opts,
        );
        AxisReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            count: report_count(
                out.bytes.as_deref(),
                &["added", "removed", "changed", "idChanged"],
            ),
        }
    });
    (inspect, query, diff)
}

/// `generate system-landscape` (P4T-V1): the command fans over
/// DIRECTORIES, so its observation unit is the ENTRY DIRECTORY itself —
/// `scan_dir` picks `workspace.dsl`/`workspace.json` through the reference
/// default include, the sidecar never matches. Zero loadable workspaces is
/// the OXZ-5010 landscape failure — how the mutated corpus feeds 5010 into
/// the coverage gate; the merged landscape carries NO views (witnessed),
/// which `views: 0` pins in every blessed sidecar (§P4T T-3).
fn observe_generate(entry: &Entry, opts: &oxidizr_api::ApiOptions) -> CommandReport {
    let out = execute(
        Request::Generate {
            inputs: vec![entry.dir.clone()],
            include: None,
            exclude: None,
            strategy: oxidizr_api::RelationshipsStrategy::default(),
            output: None,
        },
        opts,
    );
    CommandReport {
        exit: Exit::from_class(out.exit),
        codes: codes_of(&out),
        counts: out.bytes.as_deref().map(counts_of_json),
    }
}

/// Total length of the named JSON arrays of a schema-1 report (§P4T T-3
/// axes). Bytes that fail to parse or miss a key count 0 — the exit class
/// and code set already carry the failure (`counts_of_json` rule).
fn report_count(bytes: Option<&[u8]>, keys: &[&str]) -> usize {
    let Some(text) = bytes else { return 0 };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(text) else {
        return 0;
    };
    keys.iter()
        .map(|key| {
            value
                .get(*key)
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum()
}

/// `validate` over whichever input the entry offers (DSL first).
fn observe_validate(file: &std::path::Path, opts: &oxidizr_api::ApiOptions) -> CommandReport {
    let out = execute(
        Request::Validate {
            files: vec![file.to_path_buf()],
        },
        opts,
    );
    CommandReport {
        exit: Exit::from_class(out.exit),
        codes: codes_of(&out),
        counts: None,
    }
}

/// `reconcile` (T3): applies when the entry carries BOTH inputs — the DSL is
/// the (edited) model, the JSON its layout export. The contested-claim shape
/// is the way OXZ-3002 enters the corpus.
fn observe_reconcile(entry: &Entry, opts: &oxidizr_api::ApiOptions) -> Option<CommandReport> {
    let (dsl, json) = entry.dsl.as_ref().zip(entry.json.as_ref())?;
    let out = execute(
        Request::Reconcile {
            dsl: Input::File(dsl.clone()),
            layout: Input::File(json.clone()),
            output: None,
        },
        opts,
    );
    Some(CommandReport {
        exit: Exit::from_class(out.exit),
        codes: codes_of(&out),
        counts: None,
    })
}

/// Fresh unique scratch directory under the process temp dir (parallel-safe;
/// P2T-D2: exports never write into the corpus tree). Falls back to the
/// uncreated candidate after 100 collisions — the export then records its
/// own I/O failure, which is honest, better than hanging.
#[must_use]
pub fn scratch_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    static NONCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let nonce = NONCE.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or_else(|_| "t".to_string(), |d| d.as_nanos().to_string())
    });
    let slug: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let root = std::env::temp_dir().join("oxidizr-e2e-export");
    for _ in 0..100 {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let candidate = root.join(format!("{nonce}-{n}-{slug}"));
        if std::fs::create_dir_all(&root).is_ok() && std::fs::create_dir(&candidate).is_ok() {
            return candidate;
        }
    }
    root.join(format!("{nonce}-x-{slug}"))
}

/// Export one workspace file through one format into a fresh scratch dir;
/// returns the outcome plus every emitted file (name → bytes). The scratch
/// dir is removed before returning — the caller owns the bytes (P2T-D5
/// invariants compare them across runs/paths).
#[must_use]
pub fn export_capture(
    workspace: &std::path::Path,
    format: &str,
    opts: &oxidizr_api::ApiOptions,
    tag: &str,
) -> (Outcome, std::collections::BTreeMap<String, Vec<u8>>) {
    let dir = scratch_dir(tag);
    let out = execute(
        Request::Export {
            workspace: workspace.to_path_buf(),
            format: format.to_string(),
            output: Some(dir.clone()),
        },
        opts,
    );
    let mut files = std::collections::BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Ok(bytes) = std::fs::read(&path) {
                    files.insert(name, bytes);
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    (out, files)
}

/// The export matrix (P2T-D2): applies when the entry's export input (DSL
/// first, else the JSON fixture — the `validate` rule) parses successfully
/// and carries ≥ 1 view; the `websequencediagrams` slot landed at
/// P4 §P4-M2 N-5 (its sidecar recorded the OXZ-5007 compat probe until
/// then); theme failures (OXZ-5001/5008) surface per format before dispatch,
/// exactly like the reference ordering.
fn observe_export(
    entry: &Entry,
    opts: &oxidizr_api::ApiOptions,
    dsl2json: Option<&CommandReport>,
    json2json: Option<&CommandReport>,
) -> Option<ExportReports> {
    let input = entry.dsl.as_ref().or(entry.json.as_ref())?;
    let parses_with_views = if entry.dsl.is_some() {
        dsl2json.is_some_and(|r| r.exit == Exit::Ok && r.counts.is_some_and(|c| c.views >= 1))
    } else {
        // JSON-only entry: the explicit `json2json` feed is the parse gate;
        // the view count comes from the fixture itself.
        json2json.is_some_and(|r| r.exit == Exit::Ok)
            && entry
                .json
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| oxidizr_json::from_slice(&b).ok())
                .is_some_and(|ws| view_count(&ws) >= 1)
    };
    if !parses_with_views {
        return None;
    }
    let report = |format: &str| {
        let (out, files) = export_capture(input, format, opts, &format!("{}-{format}", entry.name));
        ExportReport {
            exit: Exit::from_class(out.exit),
            codes: codes_of(&out),
            files: files.len(),
        }
    };
    Some(ExportReports {
        plantuml: report("plantuml"),
        plantuml_c4: report("plantuml/c4plantuml"),
        mermaid: report("mermaid"),
        websequencediagrams: report("websequencediagrams"),
    })
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
