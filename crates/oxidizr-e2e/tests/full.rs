//! Full-tier invariants (P1T-D6): the tool checked against ITSELF per valid
//! entry — a later intentional deviation moves both sides together, so this
//! net survives compat changes. Gated by env `OXIDZR_E2E_FULL=1` (NOT
//! `#[ignore]`: V-T4 inventoried the suite — the repo currently has zero
//! ignored tests, and an env gate never drags future expect-test gaps into
//! the nightly run the way `--include-ignored` would). Nightly `e2e.yml`
//! exports the env; locally: `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`.
#![allow(clippy::unwrap_used)]

use std::env;

use oxidizr_api::{ExitClass, FmtMode, Input, LogItem, Request, execute};
use oxidizr_e2e::corpus::{self, Entry, Kind};

/// Skip-with-note unless the full tier is requested.
fn full_tier() -> bool {
    if env::var("OXIDZR_E2E_FULL").is_ok() {
        true
    } else {
        eprintln!("skipped: set OXIDZR_E2E_FULL=1 to run the full tier (P1T-D6)");
        false
    }
}

/// Valid entries that carry a DSL — the invariant subject set.
fn dsl_entries() -> Vec<Entry> {
    corpus::discover()
        .expect("corpus discovers")
        .into_iter()
        .filter(|e| e.kind == Kind::Valid && e.dsl.is_some())
        .collect()
}

/// Fan `f` out over `items` (std-only), collecting `(label, Err(msg))`s.
fn fan_out<T: Send + Clone, F>(items: &[T], f: F) -> Vec<String>
where
    F: Fn(&T) -> Result<(), String> + Send + Sync,
{
    let f = &f;
    let mut results: Vec<Option<String>> = vec![None; items.len()];
    std::thread::scope(|scope| {
        let slots: Vec<(&T, &mut Option<String>)> = items.iter().zip(results.iter_mut()).collect();
        for (item, slot) in slots {
            let item = item.clone();
            scope.spawn(move || {
                if let Err(e) = f(&item) {
                    *slot = Some(e);
                }
            });
        }
    });
    results.into_iter().flatten().collect()
}

fn codes_of(out: &oxidizr_api::Outcome) -> Vec<u16> {
    let mut codes: Vec<u16> = out
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

fn dsl2json(entry: &Entry) -> oxidizr_api::Outcome {
    execute(
        Request::Dsl2Json {
            input: Input::File(entry.dsl.clone().unwrap()),
            output: None,
        },
        &oxidizr_e2e::hermetic_options(),
    )
}

/// Mask the two volatile JSON surfaces (M6-b technique): the `structurizr.dsl`
/// portability line (base64 of the source — differs across `x`/`fmt(x)`) and
/// the per-parse UUIDs (`structurizr.dsl.identifier` values).
fn mask_json(json: &[u8]) -> Vec<u8> {
    const KEY: &[u8] = b"\"structurizr.dsl\"";
    let hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    let seg = |w: &[u8], a: usize, b: usize| w[a..b].iter().all(|&c| hex(c));
    let mut kept: Vec<u8> = Vec::with_capacity(json.len());
    let mut started = false;
    for l in json.split(|&b| b == b'\n') {
        if l.windows(KEY.len()).any(|w| w == KEY) {
            continue;
        }
        if started {
            kept.push(b'\n');
        }
        started = true;
        kept.extend_from_slice(l);
    }
    let mut out = Vec::with_capacity(kept.len());
    let mut i = 0;
    while i < kept.len() {
        let hit = kept.len() >= i + 36 && {
            let w = &kept[i..i + 36];
            w[8] == b'-'
                && w[13] == b'-'
                && w[18] == b'-'
                && w[23] == b'-'
                && seg(w, 0, 8)
                && seg(w, 9, 13)
                && seg(w, 14, 18)
                && seg(w, 19, 23)
                && seg(w, 24, 36)
        };
        if hit {
            out.extend_from_slice(b"<uuid>");
            i += 36;
        } else {
            out.push(kept[i]);
            i += 1;
        }
    }
    out
}

fn fmt_print(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let out = execute(
        Request::Fmt {
            inputs: vec![Input::Bytes(bytes)],
            mode: FmtMode::Print,
        },
        &oxidizr_e2e::hermetic_options(),
    );
    out.bytes
}

/// Invariant 1 — parse determinism: two `dsl2json` runs over one source
/// produce identical masked bytes and identical code sets.
#[test]
fn parse_is_deterministic() {
    if !full_tier() {
        return;
    }
    let entries = dsl_entries();
    let problems = fan_out(&entries, |e| {
        let a = dsl2json(e);
        let b = dsl2json(e);
        let name = e.full_name();
        if codes_of(&a) != codes_of(&b) {
            return Err(format!("{name}: code sets differ between runs"));
        }
        match (a.bytes, b.bytes) {
            (Some(x), Some(y)) if mask_json(&x) == mask_json(&y) => Ok(()),
            (Some(_), Some(_)) => Err(format!("{name}: masked JSON differs between runs")),
            _ => Ok(()), // failing entries carry no bytes — codes already compared
        }
    });
    assert!(
        problems.is_empty(),
        "parse determinism:\n{}",
        problems.join("\n")
    );
}

/// Invariant 2 — writer stability: `json2json` reaches a byte fixpoint in ONE
/// step (`out₃ == out₂`); internal consistency, not byte-parity with the
/// reference. `out₂ == out₁` is NOT the bar: the reference JSON reader
/// rebuilds Java `TreeSet`s, re-sorting model element arrays numeric-id
/// ascending, while the DSL path stores some collections in reverse-insert
/// order (WRITE-CATALOG §5, witness-proven in P2 commit `cc243a0` — a DSL
/// group A..F exports F..A, the re-read sorts it A..F). The rebuild is
/// idempotent, so bytes must stabilize after the first pass — and if they
/// didn't, `out₃ != out₂` would catch the drift byte-exactly.
#[test]
fn json_round_trip_is_byte_stable() {
    if !full_tier() {
        return;
    }
    let entries = dsl_entries();
    let problems = fan_out(&entries, |e| {
        let name = e.full_name();
        let first = dsl2json(e);
        let Some(one) = first.bytes else {
            return Ok(());
        };
        let pass = |bytes: Vec<u8>| {
            execute(
                Request::Json2Json {
                    input: Input::Bytes(bytes),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            )
        };
        let second = pass(one);
        let Some(two) = second.bytes else {
            return Err(format!("{name}: json2json failed over own dsl2json output"));
        };
        let third = pass(two.clone());
        match third.bytes {
            Some(tref) if tref == two => Ok(()),
            Some(_) => Err(format!(
                "{name}: json2json is not a byte fixpoint (out3 != out2)"
            )),
            None => Err(format!(
                "{name}: json2json failed over own output (second pass)"
            )),
        }
    });
    assert!(
        problems.is_empty(),
        "json2json fixpoint:\n{}",
        problems.join("\n")
    );
}

/// Invariant 3 — `fmt`: idempotent, and `parse(fmt(x)) ≡ parse(x)` (masked).
/// The parse half replays `fmt(x)` from a scratch copy of the entry directory
/// (child fixtures included): `!include`/`extends` resolve relative to the
/// input file, so `Input::Bytes` would break multi-file entries by design,
/// not by an engine bug.
#[test]
fn fmt_is_fixpoint_and_semantic() {
    if !full_tier() {
        return;
    }
    let entries = dsl_entries();
    let problems = fan_out(&entries, |e| {
        let name = e.full_name();
        let source = std::fs::read(e.dsl.as_ref().unwrap()).unwrap();
        let Some(once) = fmt_print(source) else {
            return Err(format!("{name}: fmt produced no output"));
        };
        let twice = fmt_print(once.clone());
        if twice.as_deref() != Some(once.as_slice()) {
            return Err(format!("{name}: fmt is not idempotent"));
        }
        let before = dsl2json(e);
        let Some(before_bytes) = before.bytes else {
            return Ok(()); // invalid entries: fmt-only checks above
        };
        let after = parse_in_scratch(e, &name, &once);
        match after {
            Some(a) if mask_json(&a) == mask_json(&before_bytes) => Ok(()),
            Some(_) => Err(format!("{name}: parse(fmt(x)) != parse(x)")),
            None => Err(format!("{name}: dsl2json failed over fmt(x)")),
        }
    });
    assert!(
        problems.is_empty(),
        "fmt invariants:\n{}",
        problems.join("\n")
    );
}

/// `dsl2json` over `text` as `workspace.dsl` inside a scratch copy of the
/// entry directory (children copied, sidecar excluded), so relative
/// `!include`/`extends` targets resolve like they do for the original.
fn parse_in_scratch(entry: &Entry, name: &str, text: &[u8]) -> Option<Vec<u8>> {
    let scratch = std::env::temp_dir().join(format!(
        "oxidizr-e2e-fmt-{}-{}",
        std::process::id(),
        name.replace('/', "-")
    ));
    let result = (|| -> Option<Vec<u8>> {
        std::fs::create_dir_all(&scratch).ok()?;
        copy_children(&entry.dir, &scratch).ok()?;
        let dsl = scratch.join("workspace.dsl");
        std::fs::write(&dsl, text).ok()?;
        let out = execute(
            Request::Dsl2Json {
                input: Input::File(dsl),
                output: None,
            },
            &oxidizr_e2e::hermetic_options(),
        );
        out.bytes
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Copy every child of `src` into `dst` except the entry DSL and the sidecar
/// (the caller writes the DSL; sidecars are expectations, never inputs).
fn copy_children(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    for item in std::fs::read_dir(src)? {
        let from = item?.path();
        let Some(file_name) = from.file_name() else {
            continue;
        };
        if file_name == oxidizr_e2e::manifest::FILE_NAME || file_name == "workspace.dsl" {
            continue;
        }
        let to = dst.join(file_name);
        if from.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_children(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Invariant 4 — `generate system-landscape` merge (§P4T T-3 gap 4): every
/// consecutive PAIR of loading valid entries merges into a landscape that
/// (a) carries NO views (the witnessed reference behavior — P4-D9), (b) has
/// unique element ids across the merged set, and (c) keeps at most ONE
/// relationship per source→destination pair (the `First` duplicate-edge
/// policy). Pair inputs are the entries' own `dsl2json` exports written as
/// `a/workspace.json` / `b/workspace.json` — what the default include
/// pattern selects.
#[test]
fn generate_pair_merge_is_view_free_and_id_unique() {
    if !full_tier() {
        return;
    }
    let mut entries = dsl_entries();
    entries.sort_by_key(Entry::full_name);
    let pairs: Vec<(Entry, Entry)> = entries
        .chunks(2)
        .filter_map(|chunk| Some((chunk.first()?.clone(), chunk.get(1)?.clone())))
        .collect();
    let problems = fan_out(&pairs, |(a, b)| {
        let name = format!("{} + {}", a.full_name(), b.full_name());
        let Some(a_json) = dsl2json(a).bytes else {
            return Ok(()); // non-loading entries: the quick-tier axes cover them
        };
        let Some(b_json) = dsl2json(b).bytes else {
            return Ok(());
        };
        let scratch = oxidizr_e2e::runner::scratch_dir(&format!("gen-{}", a.name));
        let result = (|| -> Result<(), String> {
            for (side, bytes) in [("a", a_json), ("b", b_json)] {
                let dir = scratch.join(side);
                std::fs::create_dir_all(&dir).map_err(|e| format!("{name}: scratch: {e}"))?;
                std::fs::write(dir.join("workspace.json"), bytes)
                    .map_err(|e| format!("{name}: scratch write: {e}"))?;
            }
            let out = execute(
                Request::Generate {
                    inputs: vec![scratch.clone()],
                    include: None,
                    exclude: None,
                    strategy: oxidizr_api::RelationshipsStrategy::default(),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            );
            if out.exit != ExitClass::Ok {
                return Err(format!("{name}: generate exited {:?}", out.exit));
            }
            let Some(bytes) = out.bytes else {
                return Err(format!("{name}: generate produced no output"));
            };
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("{name}: landscape is not JSON: {e}"))?;
            // (a) no views: absent, or every present section empty.
            if let Some(views) = value.get("views") {
                let non_empty = views.as_object().is_some_and(|m| {
                    m.values()
                        .any(|v| v.as_array().is_some_and(|a| !a.is_empty()))
                });
                if non_empty || views.is_array() {
                    return Err(format!("{name}: landscape carries views"));
                }
            }
            let Some(model) = value.get("model").and_then(|m| m.as_object()) else {
                return Err(format!("{name}: model is not an object"));
            };
            // (b) unique ids across the merged people + systems.
            let mut ids: Vec<&str> = Vec::new();
            for key in ["people", "softwareSystems"] {
                if let Some(items) = model.get(key).and_then(|v| v.as_array()) {
                    for item in items {
                        let id = item.get("id").and_then(|v| v.as_str());
                        match id {
                            Some(id) => ids.push(id),
                            None => return Err(format!("{name}: merged item without id")),
                        }
                    }
                }
            }
            let unique = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
            if unique != ids.len() {
                return Err(format!(
                    "{name}: merged ids not unique ({unique} of {})",
                    ids.len()
                ));
            }
            // (c) `First` policy: one edge per source→destination pair.
            let mut pairs_seen: std::collections::BTreeSet<(&str, &str)> =
                std::collections::BTreeSet::new();
            if let Some(rels) = model.get("relationships").and_then(|v| v.as_array()) {
                for rel in rels {
                    let src = rel.get("source").and_then(|v| v.as_str());
                    let dst = rel.get("destination").and_then(|v| v.as_str());
                    if let (Some(s), Some(d)) = (src, dst)
                        && !pairs_seen.insert((s, d))
                    {
                        return Err(format!("{name}: duplicate edge {s} -> {d} under First"));
                    }
                }
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&scratch);
        result
    });
    assert!(
        problems.is_empty(),
        "generate pair-merge invariants:\n{}",
        problems.join("\n")
    );
}

/// Invariant 5 — self-`reconcile`: a workspace reconciled against its own
/// JSON export is the identity — exit ok, zero ambiguity notes, zero graft
/// (output ≡ layout, masked).
#[test]
fn self_reconcile_is_identity() {
    if !full_tier() {
        return;
    }
    let entries = dsl_entries();
    let problems = fan_out(&entries, |e| {
        let name = e.full_name();
        let first = dsl2json(e);
        let Some(layout) = first.bytes else {
            return Ok(());
        };
        let out = execute(
            Request::Reconcile {
                dsl: Input::File(e.dsl.clone().unwrap()),
                layout: Input::Bytes(layout.clone()),
                output: None,
            },
            &oxidizr_e2e::hermetic_options(),
        );
        if out.exit != ExitClass::Ok {
            return Err(format!("{name}: self-reconcile exited {:?}", out.exit));
        }
        let codes = codes_of(&out);
        if !codes.is_empty() {
            return Err(format!("{name}: self-reconcile emitted {codes:?}"));
        }
        match out.bytes {
            Some(b) if mask_json(&b) == mask_json(&layout) => Ok(()),
            Some(_) => Err(format!("{name}: self-reconcile changed the workspace")),
            None => Err(format!("{name}: self-reconcile produced no output")),
        }
    });
    assert!(
        problems.is_empty(),
        "self-reconcile identity:\n{}",
        problems.join("\n")
    );
}
